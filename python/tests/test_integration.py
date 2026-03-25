import asyncio

import grpc
import pytest
import pytest_asyncio

from grpcproxy.backend import start_backend
from grpcproxy.generated import echo_pb2, echo_pb2_grpc, math_pb2, math_pb2_grpc
from grpcproxy.proxy import start_proxy


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
