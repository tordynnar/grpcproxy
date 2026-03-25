package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/tordynnar/grpcproxy/backend"
)

func main() {
	addr := flag.String("addr", "localhost:50051", "listen address")
	flag.Parse()

	srv, lis, err := backend.StartServer(*addr)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("backend listening on %s\n", lis.Addr())

	ch := make(chan os.Signal, 1)
	signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM)
	<-ch
	fmt.Println("\nshutting down...")
	srv.GracefulStop()
}
