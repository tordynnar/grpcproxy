package backend

import (
	"context"
	"fmt"
	"io"
	"net"
	"strings"

	"github.com/tordynnar/grpcproxy/pb"
	"google.golang.org/grpc"
)

type echoServer struct {
	pb.UnimplementedEchoServiceServer
}

func (s *echoServer) UnaryEcho(_ context.Context, req *pb.EchoRequest) (*pb.EchoResponse, error) {
	return &pb.EchoResponse{Message: req.Message, Source: "backend"}, nil
}

func (s *echoServer) ServerStreamEcho(req *pb.EchoRequest, stream grpc.ServerStreamingServer[pb.EchoResponse]) error {
	for i := 0; i < 3; i++ {
		if err := stream.Send(&pb.EchoResponse{
			Message: fmt.Sprintf("%s #%d", req.Message, i+1),
			Source:  "backend",
		}); err != nil {
			return err
		}
	}
	return nil
}

func (s *echoServer) ClientStreamEcho(stream grpc.ClientStreamingServer[pb.EchoRequest, pb.EchoResponse]) error {
	var messages []string
	for {
		req, err := stream.Recv()
		if err == io.EOF {
			return stream.SendAndClose(&pb.EchoResponse{
				Message: strings.Join(messages, " "),
				Source:  "backend",
			})
		}
		if err != nil {
			return err
		}
		messages = append(messages, req.Message)
	}
}

func (s *echoServer) BidiStreamEcho(stream grpc.BidiStreamingServer[pb.EchoRequest, pb.EchoResponse]) error {
	for {
		req, err := stream.Recv()
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return err
		}
		if err := stream.Send(&pb.EchoResponse{
			Message: req.Message,
			Source:  "backend",
		}); err != nil {
			return err
		}
	}
}

type mathServer struct {
	pb.UnimplementedMathServiceServer
}

func (s *mathServer) Add(_ context.Context, req *pb.AddRequest) (*pb.AddResponse, error) {
	return &pb.AddResponse{Result: req.A + req.B, Source: "backend"}, nil
}

func (s *mathServer) Fibonacci(req *pb.FibRequest, stream grpc.ServerStreamingServer[pb.FibResponse]) error {
	a, b := int64(0), int64(1)
	for i := int32(0); i < req.Count; i++ {
		if err := stream.Send(&pb.FibResponse{Value: a, Source: "backend"}); err != nil {
			return err
		}
		a, b = b, a+b
	}
	return nil
}

func StartServer(addr string) (*grpc.Server, net.Listener, error) {
	lis, err := net.Listen("tcp", addr)
	if err != nil {
		return nil, nil, fmt.Errorf("listen: %w", err)
	}
	srv := grpc.NewServer()
	pb.RegisterEchoServiceServer(srv, &echoServer{})
	pb.RegisterMathServiceServer(srv, &mathServer{})
	go srv.Serve(lis)
	return srv, lis, nil
}
