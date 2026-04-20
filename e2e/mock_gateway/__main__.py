"""CLI for mock-gateway.

    python3 -m e2e.mock_gateway --fixture e2e/fixtures/simple-qa --port 18900
"""
import argparse
import socket
from pathlib import Path

import uvicorn

from e2e.mock_gateway.replay import Cadence
from e2e.mock_gateway.server import create_app


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--fixture", type=Path, required=True)
    p.add_argument("--port", type=int, default=0)
    p.add_argument("--cadence",
                   choices=[c.value for c in Cadence],
                   default=Cadence.COMPRESSED.value)
    args = p.parse_args()

    app = create_app(args.fixture, cadence=Cadence(args.cadence))
    if args.port == 0:
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            args.port = s.getsockname()[1]
    print(f"mock-gateway listening on http://127.0.0.1:{args.port}", flush=True)
    uvicorn.run(app, host="127.0.0.1", port=args.port, log_level="warning")


if __name__ == "__main__":
    main()
