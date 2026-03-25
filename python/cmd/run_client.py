import argparse
import asyncio

import grpc

from grpcproxy.generated import echo_pb2, echo_pb2_grpc, math_pb2, math_pb2_grpc


async def main():
    parser = argparse.ArgumentParser(description="gRPC test client")
    parser.add_argument("--addr", default="localhost:50052", help="proxy address")
    args = parser.parse_args()

    async with grpc.aio.insecure_channel(args.addr) as channel:
        echo = echo_pb2_grpc.EchoServiceStub(channel)
        math = math_pb2_grpc.MathServiceStub(channel)

        # 1. UnaryEcho (intercepted)
        print("--- UnaryEcho (intercepted) ---")
        resp = await echo.UnaryEcho(echo_pb2.EchoRequest(message="hello world"))
        print(f"  message={resp.message!r} source={resp.source!r}")

        # 2. ServerStreamEcho (intercepted)
        print("--- ServerStreamEcho (intercepted) ---")
        async for resp in echo.ServerStreamEcho(
            echo_pb2.EchoRequest(message="hello")
        ):
            print(f"  message={resp.message!r} source={resp.source!r}")

        # 3. ClientStreamEcho (intercepted)
        print("--- ClientStreamEcho (intercepted) ---")

        async def client_messages():
            for msg in ["hello", "world", "foo"]:
                yield echo_pb2.EchoRequest(message=msg)

        resp = await echo.ClientStreamEcho(client_messages())
        print(f"  message={resp.message!r} source={resp.source!r}")

        # 4. BidiStreamEcho (intercepted)
        print("--- BidiStreamEcho (intercepted) ---")
        stream = echo.BidiStreamEcho(client_messages())
        async for resp in stream:
            print(f"  message={resp.message!r} source={resp.source!r}")

        # 5. Add (forwarded)
        print("--- Add (forwarded) ---")
        resp = await math.Add(math_pb2.AddRequest(a=17, b=25))
        print(f"  result={resp.result} source={resp.source!r}")

        # 6. Fibonacci (forwarded)
        print("--- Fibonacci (forwarded) ---")
        async for resp in math.Fibonacci(math_pb2.FibRequest(count=10)):
            print(f"  value={resp.value} source={resp.source!r}")


if __name__ == "__main__":
    asyncio.run(main())
