package main

import (
	"context"
	"flag"
	"fmt"
	"io"
	"log"

	"github.com/tordynnar/grpcproxy/pb"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

func main() {
	addr := flag.String("addr", "localhost:50052", "proxy address")
	flag.Parse()

	conn, err := grpc.NewClient(*addr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("dial: %v", err)
	}
	defer conn.Close()

	ctx := context.Background()
	echoClient := pb.NewEchoServiceClient(conn)
	mathClient := pb.NewMathServiceClient(conn)

	// Unary Echo (intercepted)
	fmt.Println("=== UnaryEcho (intercepted) ===")
	resp, err := echoClient.UnaryEcho(ctx, &pb.EchoRequest{Message: "hello world"})
	if err != nil {
		log.Fatalf("UnaryEcho: %v", err)
	}
	fmt.Printf("  message=%q source=%q\n\n", resp.Message, resp.Source)

	// Server-streaming Echo (intercepted)
	fmt.Println("=== ServerStreamEcho (intercepted) ===")
	sstream, err := echoClient.ServerStreamEcho(ctx, &pb.EchoRequest{Message: "hello"})
	if err != nil {
		log.Fatalf("ServerStreamEcho: %v", err)
	}
	for {
		r, err := sstream.Recv()
		if err == io.EOF {
			break
		}
		if err != nil {
			log.Fatalf("Recv: %v", err)
		}
		fmt.Printf("  message=%q source=%q\n", r.Message, r.Source)
	}
	fmt.Println()

	// Client-streaming Echo (intercepted)
	fmt.Println("=== ClientStreamEcho (intercepted) ===")
	cstream, err := echoClient.ClientStreamEcho(ctx)
	if err != nil {
		log.Fatalf("ClientStreamEcho: %v", err)
	}
	for _, msg := range []string{"hello", "world", "foo"} {
		cstream.Send(&pb.EchoRequest{Message: msg})
	}
	cresp, err := cstream.CloseAndRecv()
	if err != nil {
		log.Fatalf("CloseAndRecv: %v", err)
	}
	fmt.Printf("  message=%q source=%q\n\n", cresp.Message, cresp.Source)

	// Bidi-streaming Echo (intercepted)
	fmt.Println("=== BidiStreamEcho (intercepted) ===")
	bstream, err := echoClient.BidiStreamEcho(ctx)
	if err != nil {
		log.Fatalf("BidiStreamEcho: %v", err)
	}
	for _, msg := range []string{"alpha", "beta"} {
		bstream.Send(&pb.EchoRequest{Message: msg})
		r, err := bstream.Recv()
		if err != nil {
			log.Fatalf("Recv: %v", err)
		}
		fmt.Printf("  message=%q source=%q\n", r.Message, r.Source)
	}
	bstream.CloseSend()
	fmt.Println()

	// Add (forwarded)
	fmt.Println("=== Add (forwarded) ===")
	addResp, err := mathClient.Add(ctx, &pb.AddRequest{A: 17, B: 25})
	if err != nil {
		log.Fatalf("Add: %v", err)
	}
	fmt.Printf("  result=%d source=%q\n\n", addResp.Result, addResp.Source)

	// Fibonacci (forwarded)
	fmt.Println("=== Fibonacci (forwarded) ===")
	fstream, err := mathClient.Fibonacci(ctx, &pb.FibRequest{Count: 10})
	if err != nil {
		log.Fatalf("Fibonacci: %v", err)
	}
	for {
		r, err := fstream.Recv()
		if err == io.EOF {
			break
		}
		if err != nil {
			log.Fatalf("Recv: %v", err)
		}
		fmt.Printf("  value=%d source=%q\n", r.Value, r.Source)
	}
}
