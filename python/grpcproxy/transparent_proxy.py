import asyncio

import grpc


def _identity(x):
    return x


class FullTransparentProxyHandler(grpc.GenericRpcHandler):
    """Transparent proxy that forwards any unknown RPC method to the backend.

    Uses stream_stream as a universal handler for all RPC types (unary,
    server-streaming, client-streaming, bidi). This works because at the
    gRPC wire protocol level there is no distinction — unary is just a
    stream of one message. grpcio adapts accordingly.
    """

    def __init__(self, channel: grpc.aio.Channel):
        self._channel = channel

    def service(self, handler_call_details):
        method = handler_call_details.method

        return grpc.stream_stream_rpc_method_handler(
            self._make_stream_stream(method),
            request_deserializer=_identity,
            response_serializer=_identity,
        )

    def _make_stream_stream(self, method: str):
        async def handler(request_iterator, context):
            try:
                # Forward incoming request metadata to the backend.
                incoming_metadata = context.invocation_metadata()

                call = self._channel.stream_stream(
                    method,
                    request_serializer=_identity,
                    response_deserializer=_identity,
                )
                stream = call(metadata=incoming_metadata)

                # Forward all requests from client to backend
                async def forward_requests():
                    try:
                        async for req_bytes in request_iterator:
                            await stream.write(req_bytes)
                        await stream.done_writing()
                    except grpc.aio.AioRpcError:
                        pass  # Error will be surfaced when reading responses

                forward_task = asyncio.create_task(forward_requests())

                # Relay response initial metadata back to the client.
                initial_metadata = await stream.initial_metadata()
                if initial_metadata:
                    await context.send_initial_metadata(initial_metadata)

                # Relay all responses from backend to client
                async for resp_bytes in stream:
                    yield resp_bytes

                # Relay trailing metadata back to the client.
                trailing_metadata = await stream.trailing_metadata()
                if trailing_metadata:
                    context.set_trailing_metadata(trailing_metadata)

                await forward_task
            except grpc.aio.AioRpcError as e:
                await context.abort(e.code(), e.details())

        return handler
