import asyncio

import grpc


def _identity(x):
    return x


class TransparentProxyHandler(grpc.GenericRpcHandler):
    """Forwards any unregistered gRPC method to the backend as raw bytes."""

    def __init__(self, channel: grpc.aio.Channel):
        self._channel = channel

    def service(self, handler_call_details):
        method = handler_call_details.method
        return grpc.unary_unary_rpc_method_handler(
            self._make_unary_unary(method),
            request_deserializer=_identity,
            response_serializer=_identity,
        )

    def _make_unary_unary(self, method: str):
        async def handler(request_bytes, context):
            call = self._channel.unary_unary(
                method,
                request_serializer=_identity,
                response_deserializer=_identity,
            )
            response_bytes = await call(request_bytes)
            return response_bytes

        return handler


class TransparentProxyStreamHandler(grpc.GenericRpcHandler):
    """Handles server-streaming RPCs forwarded to the backend."""

    def __init__(self, channel: grpc.aio.Channel):
        self._channel = channel

    def service(self, handler_call_details):
        method = handler_call_details.method
        return grpc.unary_stream_rpc_method_handler(
            self._make_unary_stream(method),
            request_deserializer=_identity,
            response_serializer=_identity,
        )

    def _make_unary_stream(self, method: str):
        async def handler(request_bytes, context):
            call = self._channel.unary_stream(
                method,
                request_serializer=_identity,
                response_deserializer=_identity,
            )
            stream = call(request_bytes)
            async for response_bytes in stream:
                yield response_bytes

        return handler


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
            call = self._channel.stream_stream(
                method,
                request_serializer=_identity,
                response_deserializer=_identity,
            )
            stream = call()

            # Forward all requests from client to backend
            async def forward_requests():
                async for req_bytes in request_iterator:
                    await stream.write(req_bytes)
                await stream.done_writing()

            forward_task = asyncio.ensure_future(forward_requests())

            # Relay all responses from backend to client
            async for resp_bytes in stream:
                yield resp_bytes

            await forward_task

        return handler
