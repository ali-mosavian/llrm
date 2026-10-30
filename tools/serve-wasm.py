#!/usr/bin/env python3
"""Serves runtime/wasm for the browser page (web/index.html), with the headers SharedArrayBuffer needs.

usage: tools/serve-wasm.py [--host 127.0.0.1] [--port 8099]
Build a program next to the page first: runtime/wasm/build.sh PROGRAM.BAS runtime/wasm/web/program.wasm
then open http://127.0.0.1:8099/web/index.html (?wasm=NAME.wasm for another module in web/).
"""
import argparse
import functools
import http.server
import pathlib


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".mjs": "text/javascript"}

    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8099)
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parent.parent / "runtime" / "wasm"
    handler = functools.partial(Handler, directory=str(root))
    with http.server.ThreadingHTTPServer((args.host, args.port), handler) as server:
        print(f"http://{args.host}:{args.port}/web/index.html")
        server.serve_forever()


if __name__ == "__main__":
    main()
