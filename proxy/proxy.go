package proxy

import (
	"context"
	"fmt"
	"io"
	"net"
	"strings"

	"github.com/tordynnar/grpcproxy/pb"
	grpcproxy "github.com/mwitkow/grpc-proxy/proxy"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
)

// interceptedEchoServer handles EchoService calls locally on the proxy,
// transforming responses (uppercasing messages, setting source="proxy").
type interceptedEchoServer struct {
	pb.UnimplementedEchoServiceServer
}

func (s *interceptedEchoServer) UnaryEcho(_ context.Context, req *pb.EchoRequest) (*pb.EchoResponse, error) {
	return &pb.EchoResponse{
		Message: strings.ToUpper(req.Message),
		Source:  "proxy",
	}, nil
}

func (s *interceptedEchoServer) ServerStreamEcho(req *pb.EchoRequest, stream grpc.ServerStreamingServer[pb.EchoResponse]) error {
	upper := strings.ToUpper(req.Message)
	for i := 0; i < 3; i++ {
		if err := stream.Send(&pb.EchoResponse{
			Message: fmt.Sprintf("[PROXY] %s #%d", upper, i+1),
			Source:  "proxy",
		}); err != nil {
			return err
		}
	}
	return nil
}

func (s *interceptedEchoServer) ClientStreamEcho(stream grpc.ClientStreamingServer[pb.EchoRequest, pb.EchoResponse]) error {
	var messages []string
	for {
		req, err := stream.Recv()
		if err == io.EOF {
			return stream.SendAndClose(&pb.EchoResponse{
				Message: strings.ToUpper(strings.Join(messages, " ")),
				Source:  "proxy",
			})
		}
		if err != nil {
			return err
		}
		messages = append(messages, req.Message)
	}
}

func (s *interceptedEchoServer) BidiStreamEcho(stream grpc.BidiStreamingServer[pb.EchoRequest, pb.EchoResponse]) error {
	for {
		req, err := stream.Recv()
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return err
		}
		if err := stream.Send(&pb.EchoResponse{
			Message: strings.ToUpper(req.Message),
			Source:  "proxy",
		}); err != nil {
			return err
		}
	}
}

// StartProxy starts the proxy server. It registers the intercepted EchoService
// locally and forwards everything else transparently, establishing a new backend
// connection on demand for each proxied request.
func StartProxy(listenAddr, backendAddr string) (*grpc.Server, net.Listener, error) {
	director := func(ctx context.Context, fullMethodName string) (context.Context, grpc.ClientConnInterface, error) {
		md, _ := metadata.FromIncomingContext(ctx)
		outCtx := metadata.NewOutgoingContext(ctx, md.Copy())

		conn, err := grpc.NewClient(backendAddr, grpc.WithTransportCredentials(insecure.NewCredentials()))
		if err != nil {
			return nil, nil, fmt.Errorf("dial backend: %w", err)
		}
		go func() {
			<-ctx.Done()
			conn.Close()
		}()
		return outCtx, conn, nil
	}

	srv := grpc.NewServer(grpc.UnknownServiceHandler(grpcproxy.TransparentHandler(director)))

	// Register EchoService locally — these calls are intercepted.
	// MathService is NOT registered, so it flows through the UnknownServiceHandler to the backend.
	pb.RegisterEchoServiceServer(srv, &interceptedEchoServer{})

	lis, err := net.Listen("tcp", listenAddr)
	if err != nil {
		return nil, nil, fmt.Errorf("listen: %w", err)
	}
	go srv.Serve(lis)
	return srv, lis, nil
}
