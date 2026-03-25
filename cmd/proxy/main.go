package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/tordynnar/grpcproxy/proxy"
)

func main() {
	addr := flag.String("addr", "localhost:50052", "proxy listen address")
	backendAddr := flag.String("backend", "localhost:50051", "backend address")
	flag.Parse()

	srv, lis, err := proxy.StartProxy(*addr, *backendAddr)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("proxy listening on %s (backend: %s)\n", lis.Addr(), *backendAddr)

	ch := make(chan os.Signal, 1)
	signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM)
	<-ch
	fmt.Println("\nshutting down...")
	srv.GracefulStop()
}
