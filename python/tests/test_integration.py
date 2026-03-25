import asyncio

import grpc
import pytest
import pytest_asyncio

from grpcproxy.backend import start_backend
from grpcproxy.generated import echo_pb2, echo_pb2_grpc, math_pb2, math_pb2_grpc
from grpcproxy.proxy import start_proxy
from grpcproxy.transparent_proxy import FullTransparentProxyHandler


@pytest_asyncio.fixture
async def env():
    backend_server, backend_port = await start_backend("localhost:0")
    proxy_server, proxy_port = await start_proxy(
        "localhost:0", f"localhost:{backend_port}"
    )

    channel = grpc.aio.insecure_channel(f"localhost:{proxy_port}")
    echo_client = echo_pb2_grpc.EchoServiceStub(channel)
    math_client = math_pb2_grpc.MathServiceStub(channel)

    yield echo_client, math_client

    await channel.close()
    await proxy_server.stop(grace=0)
    await backend_server.stop(grace=0)


async def _start_passthrough_proxy(listen_addr, backend_addr):
    """Proxy with NO registered services — everything goes through the transparent proxy."""
    backend_channel = grpc.aio.insecure_channel(backend_addr)
    server = grpc.aio.server()
    proxy_handler = FullTransparentProxyHandler(backend_channel)
    server.add_generic_rpc_handlers([proxy_handler])
    port = server.add_insecure_port(listen_addr)
    await server.start()
    return server, port


@pytest_asyncio.fixture
async def passthrough_env():
    """Environment where ALL services are forwarded through the transparent proxy."""
    backend_server, backend_port = await start_backend("localhost:0")
    proxy_server, proxy_port = await _start_passthrough_proxy(
        "localhost:0", f"localhost:{backend_port}"
    )

    channel = grpc.aio.insecure_channel(f"localhost:{proxy_port}")
    echo_client = echo_pb2_grpc.EchoServiceStub(channel)

    yield echo_client

    await channel.close()
    await proxy_server.stop(grace=0)
    await backend_server.stop(grace=0)


# --- Intercepted: EchoService (handled by proxy, uppercased, source="proxy") ---


@pytest.mark.asyncio
async def test_unary_echo_intercepted(env):
    echo_client, _ = env
    resp = await echo_client.UnaryEcho(echo_pb2.EchoRequest(message="hello"))
    assert resp.source == "proxy"
    assert resp.message == "HELLO"


@pytest.mark.asyncio
async def test_server_stream_echo_intercepted(env):
    echo_client, _ = env
    responses = []
    async for resp in echo_client.ServerStreamEcho(
        echo_pb2.EchoRequest(message="hello")
    ):
        responses.append(resp)

    assert len(responses) == 3
    for i, resp in enumerate(responses):
        assert resp.source == "proxy"
        assert resp.message == f"[PROXY] HELLO #{i + 1}"


@pytest.mark.asyncio
async def test_client_stream_echo_intercepted(env):
    echo_client, _ = env

    async def requests():
        for msg in ["hello", "world"]:
            yield echo_pb2.EchoRequest(message=msg)

    resp = await echo_client.ClientStreamEcho(requests())
    assert resp.source == "proxy"
    assert resp.message == "HELLO WORLD"


@pytest.mark.asyncio
async def test_bidi_stream_echo_intercepted(env):
    echo_client, _ = env
    messages = ["hello", "world", "foo"]

    async def requests():
        for msg in messages:
            yield echo_pb2.EchoRequest(message=msg)

    responses = []
    async for resp in echo_client.BidiStreamEcho(requests()):
        responses.append(resp)

    assert len(responses) == len(messages)
    for resp, msg in zip(responses, messages):
        assert resp.source == "proxy"
        assert resp.message == msg.upper()


# --- Forwarded: MathService (transparently proxied to backend, source="backend") ---


@pytest.mark.asyncio
async def test_add_forwarded(env):
    _, math_client = env
    resp = await math_client.Add(math_pb2.AddRequest(a=2, b=3))
    assert resp.source == "backend"
    assert resp.result == 5


@pytest.mark.asyncio
async def test_fibonacci_forwarded(env):
    _, math_client = env
    want = [0, 1, 1, 2, 3, 5, 8]
    got = []
    async for resp in math_client.Fibonacci(math_pb2.FibRequest(count=7)):
        assert resp.source == "backend"
        got.append(resp.value)
    assert got == want


# --- Transparent proxy: all 4 streaming types forwarded (source="backend") ---
# These tests verify that stream_stream_rpc_method_handler works as a universal
# handler for all RPC types, not just unary and server-streaming.


@pytest.mark.asyncio
async def test_unary_echo_forwarded(passthrough_env):
    echo = passthrough_env
    resp = await echo.UnaryEcho(echo_pb2.EchoRequest(message="hello"))
    assert resp.source == "backend"
    assert resp.message == "hello"


@pytest.mark.asyncio
async def test_server_stream_echo_forwarded(passthrough_env):
    echo = passthrough_env
    responses = []
    async for resp in echo.ServerStreamEcho(echo_pb2.EchoRequest(message="hello")):
        responses.append(resp)

    assert len(responses) == 3
    for i, resp in enumerate(responses):
        assert resp.source == "backend"
        assert resp.message == f"hello #{i + 1}"


@pytest.mark.asyncio
async def test_client_stream_echo_forwarded(passthrough_env):
    echo = passthrough_env

    async def requests():
        for msg in ["hello", "world"]:
            yield echo_pb2.EchoRequest(message=msg)

    resp = await echo.ClientStreamEcho(requests())
    assert resp.source == "backend"
    assert resp.message == "hello world"


@pytest.mark.asyncio
async def test_bidi_stream_echo_forwarded(passthrough_env):
    echo = passthrough_env
    messages = ["hello", "world", "foo"]

    async def requests():
        for msg in messages:
            yield echo_pb2.EchoRequest(message=msg)

    responses = []
    async for resp in echo.BidiStreamEcho(requests()):
        responses.append(resp)

    assert len(responses) == len(messages)
    for resp, msg in zip(responses, messages):
        assert resp.source == "backend"
        assert resp.message == msg


# --- Backend down: intercepted calls still work, forwarded calls fail ---


@pytest_asyncio.fixture
async def backend_down_env():
    """Proxy pointing at a port where nothing is listening."""
    server = grpc.aio.server()
    from grpcproxy.proxy import InterceptedEchoServicer
    echo_pb2_grpc.add_EchoServiceServicer_to_server(
        InterceptedEchoServicer(), server
    )
    proxy_handler = FullTransparentProxyHandler(
        grpc.aio.insecure_channel("localhost:1")
    )
    server.add_generic_rpc_handlers([proxy_handler])
    port = server.add_insecure_port("localhost:0")
    await server.start()

    channel = grpc.aio.insecure_channel(f"localhost:{port}")
    echo_client = echo_pb2_grpc.EchoServiceStub(channel)
    math_client = math_pb2_grpc.MathServiceStub(channel)

    yield echo_client, math_client

    await channel.close()
    await server.stop(grace=0)


@pytest.mark.asyncio
async def test_unary_echo_backend_down(backend_down_env):
    """Intercepted calls should still work when backend is down."""
    echo_client, _ = backend_down_env
    resp = await echo_client.UnaryEcho(echo_pb2.EchoRequest(message="hello"))
    assert resp.source == "proxy"
    assert resp.message == "HELLO"


@pytest.mark.asyncio
async def test_add_backend_down(backend_down_env):
    """Forwarded calls should fail with UNAVAILABLE when backend is down."""
    _, math_client = backend_down_env
    with pytest.raises(grpc.aio.AioRpcError) as exc_info:
        await math_client.Add(math_pb2.AddRequest(a=2, b=3))
    assert exc_info.value.code() == grpc.StatusCode.UNAVAILABLE


@pytest.mark.asyncio
async def test_fibonacci_backend_down(backend_down_env):
    """Forwarded streaming calls should fail with UNAVAILABLE when backend is down."""
    _, math_client = backend_down_env
    with pytest.raises(grpc.aio.AioRpcError) as exc_info:
        async for _ in math_client.Fibonacci(math_pb2.FibRequest(count=7)):
            pass
    assert exc_info.value.code() == grpc.StatusCode.UNAVAILABLE


# --- Backend error propagation: forwarded errors preserve code and message ---


@pytest.mark.asyncio
async def test_add_backend_error(env):
    """Backend errors should propagate with exact code and message through the proxy."""
    _, math_client = env
    with pytest.raises(grpc.aio.AioRpcError) as exc_info:
        await math_client.Add(math_pb2.AddRequest(a=0, b=0))
    assert exc_info.value.code() == grpc.StatusCode.INVALID_ARGUMENT
    assert exc_info.value.details() == "both operands are zero"
