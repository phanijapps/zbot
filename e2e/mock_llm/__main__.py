"""Run mock-llm from the command line.

    python3 -m e2e.mock_llm --fixture e2e/fixtures/simple-qa --port 18800
"""
import argparse
import socket
from pathlib import Path

import uvicorn

from e2e.mock_llm.server import create_app


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--fixture", type=Path, required=True)
    p.add_argument("--port", type=int, default=0,
                   help="0 = OS-assigned; the chosen port is printed on start")
    p.add_argument("--strict", action="store_true",
                   help="strict messages_hash matching")
    args = p.parse_args()

    app = create_app(args.fixture, strict_hashing=args.strict)
    if args.port == 0:
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            args.port = s.getsockname()[1]
    print(f"mock-llm listening on http://127.0.0.1:{args.port}", flush=True)
    uvicorn.run(app, host="127.0.0.1", port=args.port, log_level="warning")


if __name__ == "__main__":
    main()
