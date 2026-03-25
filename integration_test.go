package grpcproxy_test

import (
	"context"
	"io"
	"testing"

	"github.com/tordynnar/grpcproxy/backend"
	"github.com/tordynnar/grpcproxy/pb"
	"github.com/tordynnar/grpcproxy/proxy"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/status"
)

type testEnv struct {
	echoClient pb.EchoServiceClient
	mathClient pb.MathServiceClient
}

func setup(t *testing.T) *testEnv {
	t.Helper()

	backendSrv, backendLis, err := backend.StartServer("localhost:0")
	if err != nil {
		t.Fatalf("start backend: %v", err)
	}
	t.Cleanup(backendSrv.GracefulStop)

	proxySrv, proxyLis, err := proxy.StartProxy("localhost:0", backendLis.Addr().String())
	if err != nil {
		t.Fatalf("start proxy: %v", err)
	}
	t.Cleanup(proxySrv.GracefulStop)

	conn, err := grpc.NewClient(proxyLis.Addr().String(), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		t.Fatalf("dial proxy: %v", err)
	}
	t.Cleanup(func() { conn.Close() })

	return &testEnv{
		echoClient: pb.NewEchoServiceClient(conn),
		mathClient: pb.NewMathServiceClient(conn),
	}
}

// --- Intercepted: EchoService (handled by proxy, uppercased, source="proxy") ---

func TestUnaryEcho_Intercepted(t *testing.T) {
	env := setup(t)
	resp, err := env.echoClient.UnaryEcho(context.Background(), &pb.EchoRequest{Message: "hello"})
	if err != nil {
		t.Fatalf("UnaryEcho: %v", err)
	}
	if resp.Source != "proxy" {
		t.Errorf("source = %q, want %q", resp.Source, "proxy")
	}
	if resp.Message != "HELLO" {
		t.Errorf("message = %q, want %q", resp.Message, "HELLO")
	}
}

func TestServerStreamEcho_Intercepted(t *testing.T) {
	env := setup(t)
	stream, err := env.echoClient.ServerStreamEcho(context.Background(), &pb.EchoRequest{Message: "hello"})
	if err != nil {
		t.Fatalf("ServerStreamEcho: %v", err)
	}
	var responses []*pb.EchoResponse
	for {
		resp, err := stream.Recv()
		if err == io.EOF {
			break
		}
		if err != nil {
			t.Fatalf("Recv: %v", err)
		}
		responses = append(responses, resp)
	}
	if len(responses) != 3 {
		t.Fatalf("got %d responses, want 3", len(responses))
	}
	for i, resp := range responses {
		if resp.Source != "proxy" {
			t.Errorf("response[%d].source = %q, want %q", i, resp.Source, "proxy")
		}
		if resp.Message != "[PROXY] HELLO #"+string(rune('1'+i)) {
			// More robust check
			want := "[PROXY] HELLO #" + itoa(i+1)
			if resp.Message != want {
				t.Errorf("response[%d].message = %q, want %q", i, resp.Message, want)
			}
		}
	}
}

func TestClientStreamEcho_Intercepted(t *testing.T) {
	env := setup(t)
	stream, err := env.echoClient.ClientStreamEcho(context.Background())
	if err != nil {
		t.Fatalf("ClientStreamEcho: %v", err)
	}
	for _, msg := range []string{"hello", "world"} {
		if err := stream.Send(&pb.EchoRequest{Message: msg}); err != nil {
			t.Fatalf("Send: %v", err)
		}
	}
	resp, err := stream.CloseAndRecv()
	if err != nil {
		t.Fatalf("CloseAndRecv: %v", err)
	}
	if resp.Source != "proxy" {
		t.Errorf("source = %q, want %q", resp.Source, "proxy")
	}
	if resp.Message != "HELLO WORLD" {
		t.Errorf("message = %q, want %q", resp.Message, "HELLO WORLD")
	}
}

func TestBidiStreamEcho_Intercepted(t *testing.T) {
	env := setup(t)
	stream, err := env.echoClient.BidiStreamEcho(context.Background())
	if err != nil {
		t.Fatalf("BidiStreamEcho: %v", err)
	}
	messages := []string{"hello", "world", "foo"}
	for _, msg := range messages {
		if err := stream.Send(&pb.EchoRequest{Message: msg}); err != nil {
			t.Fatalf("Send: %v", err)
		}
		resp, err := stream.Recv()
		if err != nil {
			t.Fatalf("Recv: %v", err)
		}
		if resp.Source != "proxy" {
			t.Errorf("source = %q, want %q", resp.Source, "proxy")
		}
		want := uppercaseASCII(msg)
		if resp.Message != want {
			t.Errorf("message = %q, want %q", resp.Message, want)
		}
	}
	stream.CloseSend()
}

// --- Forwarded: MathService (transparently proxied to backend, source="backend") ---

func TestAdd_Forwarded(t *testing.T) {
	env := setup(t)
	resp, err := env.mathClient.Add(context.Background(), &pb.AddRequest{A: 2, B: 3})
	if err != nil {
		t.Fatalf("Add: %v", err)
	}
	if resp.Source != "backend" {
		t.Errorf("source = %q, want %q", resp.Source, "backend")
	}
	if resp.Result != 5 {
		t.Errorf("result = %d, want %d", resp.Result, 5)
	}
}

func TestFibonacci_Forwarded(t *testing.T) {
	env := setup(t)
	stream, err := env.mathClient.Fibonacci(context.Background(), &pb.FibRequest{Count: 7})
	if err != nil {
		t.Fatalf("Fibonacci: %v", err)
	}
	want := []int64{0, 1, 1, 2, 3, 5, 8}
	var got []int64
	for {
		resp, err := stream.Recv()
		if err == io.EOF {
			break
		}
		if err != nil {
			t.Fatalf("Recv: %v", err)
		}
		if resp.Source != "backend" {
			t.Errorf("source = %q, want %q", resp.Source, "backend")
		}
		got = append(got, resp.Value)
	}
	if len(got) != len(want) {
		t.Fatalf("got %d values, want %d", len(got), len(want))
	}
	for i := range want {
		if got[i] != want[i] {
			t.Errorf("fib[%d] = %d, want %d", i, got[i], want[i])
		}
	}
}

// --- Backend down: intercepted calls still work, forwarded calls fail ---

func setupNoBackend(t *testing.T) *testEnv {
	t.Helper()

	// Point the proxy at a port where nothing is listening.
	proxySrv, proxyLis, err := proxy.StartProxy("localhost:0", "localhost:1")
	if err != nil {
		t.Fatalf("start proxy: %v", err)
	}
	t.Cleanup(proxySrv.GracefulStop)

	conn, err := grpc.NewClient(proxyLis.Addr().String(), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		t.Fatalf("dial proxy: %v", err)
	}
	t.Cleanup(func() { conn.Close() })

	return &testEnv{
		echoClient: pb.NewEchoServiceClient(conn),
		mathClient: pb.NewMathServiceClient(conn),
	}
}

func TestUnaryEcho_BackendDown(t *testing.T) {
	env := setupNoBackend(t)
	resp, err := env.echoClient.UnaryEcho(context.Background(), &pb.EchoRequest{Message: "hello"})
	if err != nil {
		t.Fatalf("UnaryEcho should succeed (intercepted): %v", err)
	}
	if resp.Source != "proxy" {
		t.Errorf("source = %q, want %q", resp.Source, "proxy")
	}
	if resp.Message != "HELLO" {
		t.Errorf("message = %q, want %q", resp.Message, "HELLO")
	}
}

func TestAdd_BackendDown(t *testing.T) {
	env := setupNoBackend(t)
	_, err := env.mathClient.Add(context.Background(), &pb.AddRequest{A: 2, B: 3})
	if err == nil {
		t.Fatal("Add should fail when backend is down")
	}
	if got := status.Code(err); got != codes.Unavailable {
		t.Errorf("code = %v, want %v", got, codes.Unavailable)
	}
}

func TestFibonacci_BackendDown(t *testing.T) {
	env := setupNoBackend(t)
	stream, err := env.mathClient.Fibonacci(context.Background(), &pb.FibRequest{Count: 7})
	if err != nil {
		if got := status.Code(err); got != codes.Unavailable {
			t.Errorf("code = %v, want %v", got, codes.Unavailable)
		}
		return
	}
	_, err = stream.Recv()
	if err == nil || err == io.EOF {
		t.Fatal("Fibonacci Recv should fail when backend is down")
	}
	if got := status.Code(err); got != codes.Unavailable {
		t.Errorf("code = %v, want %v", got, codes.Unavailable)
	}
}

func itoa(n int) string {
	return string(rune('0' + n))
}

func uppercaseASCII(s string) string {
	b := []byte(s)
	for i, c := range b {
		if c >= 'a' && c <= 'z' {
			b[i] = c - 32
		}
	}
	return string(b)
}
