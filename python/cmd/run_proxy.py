import argparse
import asyncio
import signal

from grpcproxy.proxy import start_proxy


async def main():
    parser = argparse.ArgumentParser(description="gRPC transparent proxy")
    parser.add_argument("--addr", default="localhost:50052", help="proxy listen address")
    parser.add_argument(
        "--backend", default="localhost:50051", help="backend address"
    )
    args = parser.parse_args()

    server, port = await start_proxy(args.addr, args.backend)
    print(f"proxy listening on {args.addr} (port {port}, backend: {args.backend})")

    stop = asyncio.Event()
    loop = asyncio.get_event_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, stop.set)

    await stop.wait()
    print("\nshutting down...")
    await server.stop(grace=5)


if __name__ == "__main__":
    asyncio.run(main())
