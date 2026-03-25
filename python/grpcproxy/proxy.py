import grpc

from grpcproxy.generated import echo_pb2, echo_pb2_grpc
from grpcproxy.transparent_proxy import FullTransparentProxyHandler


class InterceptedEchoServicer(echo_pb2_grpc.EchoServiceServicer):
    async def UnaryEcho(self, request, context):
        return echo_pb2.EchoResponse(
            message=request.message.upper(), source="proxy"
        )

    async def ServerStreamEcho(self, request, context):
        upper = request.message.upper()
        for i in range(3):
            yield echo_pb2.EchoResponse(
                message=f"[PROXY] {upper} #{i + 1}", source="proxy"
            )

    async def ClientStreamEcho(self, request_iterator, context):
        messages = []
        async for req in request_iterator:
            messages.append(req.message)
        return echo_pb2.EchoResponse(
            message=" ".join(messages).upper(), source="proxy"
        )

    async def BidiStreamEcho(self, request_iterator, context):
        async for req in request_iterator:
            yield echo_pb2.EchoResponse(
                message=req.message.upper(), source="proxy"
            )


async def start_proxy(
    listen_addr: str, backend_addr: str
) -> tuple[grpc.aio.Server, int]:
    backend_channel = grpc.aio.insecure_channel(backend_addr)

    server = grpc.aio.server()

    # Register EchoService first — these calls are intercepted.
    echo_pb2_grpc.add_EchoServiceServicer_to_server(
        InterceptedEchoServicer(), server
    )

    # Add transparent proxy handler last — it forwards everything else to backend.
    proxy_handler = FullTransparentProxyHandler(backend_channel)
    server.add_generic_rpc_handlers([proxy_handler])

    port = server.add_insecure_port(listen_addr)
    await server.start()
    return server, port
