.PHONY: proto build test run-backend run-proxy run-client

PROTOC_FLAGS = --go_out=. --go_opt=module=github.com/tordynnar/grpcproxy \
               --go-grpc_out=. --go-grpc_opt=module=github.com/tordynnar/grpcproxy

proto:
	protoc $(PROTOC_FLAGS) proto/*.proto

build: proto
	go build ./...

test:
	go test -v -count=1 ./...

run-backend:
	go run ./cmd/backend

run-proxy:
	go run ./cmd/proxy

run-client:
	go run ./client
