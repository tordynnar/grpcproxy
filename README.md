# grpcproxy

A Go gRPC transparent proxy with selective interception. Some service calls are intercepted and handled locally with transformation logic, while all other calls are transparently forwarded to a backend server.

## How It Works

The proxy uses [mwitkow/grpc-proxy](https://github.com/mwitkow/grpc-proxy) and gRPC's `UnknownServiceHandler` mechanism:

- **EchoService** is registered on the proxy's `grpc.Server`, so gRPC dispatches those calls to the proxy's local handler. The proxy uppercases messages and sets `source="proxy"` in responses.
- **MathService** is _not_ registered on the proxy, so calls fall through to the `UnknownServiceHandler`, which transparently forwards them to the backend. Responses arrive unchanged with `source="backend"`.

No routing table or config is needed — routing is determined entirely by which services are registered on the proxy server.

## Services

### EchoService (intercepted by proxy)

| RPC | Type | Proxy Behavior |
|-----|------|----------------|
| `UnaryEcho` | Unary | Uppercases message |
| `ServerStreamEcho` | Server-streaming | Uppercases + prefixes `[PROXY]` to each message |
| `ClientStreamEcho` | Client-streaming | Uppercases concatenated messages |
| `BidiStreamEcho` | Bidi-streaming | Uppercases each message |

### MathService (forwarded to backend)

| RPC | Type | Backend Behavior |
|-----|------|------------------|
| `Add` | Unary | Returns `a + b` |
| `Fibonacci` | Server-streaming | Streams `count` Fibonacci numbers |

## Project Structure

```
proto/              Protobuf service definitions
pb/                 Generated Go code
backend/            Backend server (implements both services)
proxy/              Proxy server (intercepts EchoService, forwards MathService)
cmd/backend/        Backend entrypoint
cmd/proxy/          Proxy entrypoint
client/             CLI client for manual testing
integration_test.go End-to-end tests
```

## Prerequisites

- Go 1.22+
- `protoc` with `protoc-gen-go` and `protoc-gen-go-grpc`

## Usage

### Run tests

```sh
make test
```

### Run manually

```sh
# Terminal 1: start backend
go run ./cmd/backend

# Terminal 2: start proxy (connects to backend)
go run ./cmd/proxy

# Terminal 3: run client (connects to proxy)
go run ./client
```

### Regenerate protobuf code

```sh
make proto
```

## Tests

Six integration tests validate the routing:

- `TestUnaryEcho_Intercepted` — `source=="proxy"`, message uppercased
- `TestServerStreamEcho_Intercepted` — all streamed responses from proxy
- `TestClientStreamEcho_Intercepted` — aggregated response from proxy
- `TestBidiStreamEcho_Intercepted` — each echoed response from proxy
- `TestAdd_Forwarded` — `source=="backend"`, correct arithmetic
- `TestFibonacci_Forwarded` — `source=="backend"`, correct sequence
