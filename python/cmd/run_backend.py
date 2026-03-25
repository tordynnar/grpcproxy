import argparse
import asyncio
import signal

from grpcproxy.backend import start_backend


async def main():
    parser = argparse.ArgumentParser(description="gRPC backend server")
    parser.add_argument("--addr", default="localhost:50051", help="listen address")
    args = parser.parse_args()

    server, port = await start_backend(args.addr)
    print(f"backend listening on {args.addr} (port {port})")

    stop = asyncio.Event()
    loop = asyncio.get_event_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, stop.set)

    await stop.wait()
    print("\nshutting down...")
    await server.stop(grace=5)


if __name__ == "__main__":
    asyncio.run(main())
