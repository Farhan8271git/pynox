"""Ordinary Python; default execution needs no file or network grants."""

import asyncio


async def main():
    print(await asyncio.to_thread(lambda: "Hello from Pynox"))


asyncio.run(main())
