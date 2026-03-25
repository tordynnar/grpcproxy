import asyncio

import grpc

from grpcproxy.generated import echo_pb2, echo_pb2_grpc, math_pb2, math_pb2_grpc


class EchoServiceServicer(echo_pb2_grpc.EchoServiceServicer):
    async def UnaryEcho(self, request, context):
        return echo_pb2.EchoResponse(message=request.message, source="backend")

    async def ServerStreamEcho(self, request, context):
        for i in range(3):
            yield echo_pb2.EchoResponse(
                message=f"{request.message} #{i + 1}", source="backend"
            )

    async def ClientStreamEcho(self, request_iterator, context):
        messages = []
        async for req in request_iterator:
            messages.append(req.message)
        return echo_pb2.EchoResponse(message=" ".join(messages), source="backend")

    async def BidiStreamEcho(self, request_iterator, context):
        async for req in request_iterator:
            yield echo_pb2.EchoResponse(message=req.message, source="backend")


class MathServiceServicer(math_pb2_grpc.MathServiceServicer):
    async def Add(self, request, context):
        if request.a == 0 and request.b == 0:
            await context.abort(grpc.StatusCode.INVALID_ARGUMENT, "both operands are zero")
        return math_pb2.AddResponse(result=request.a + request.b, source="backend")

    async def Fibonacci(self, request, context):
        a, b = 0, 1
        for _ in range(request.count):
            yield math_pb2.FibResponse(value=a, source="backend")
            a, b = b, a + b


async def start_backend(addr: str) -> tuple[grpc.aio.Server, int]:
    server = grpc.aio.server()
    echo_pb2_grpc.add_EchoServiceServicer_to_server(EchoServiceServicer(), server)
    math_pb2_grpc.add_MathServiceServicer_to_server(MathServiceServicer(), server)
    port = server.add_insecure_port(addr)
    await server.start()
    return server, port
