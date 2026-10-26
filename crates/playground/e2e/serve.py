"""Serve the release artifact under the same /dxutil/ prefix as GitHub Pages.

No SPA fallback: query-string story routes must work on an ordinary static host.
"""
import http.server
import os
from pathlib import Path

root = Path(os.environ["DXPLAYGROUND_DIR"]).resolve()
if not (root / "index.html").is_file():
    raise SystemExit(f"Build first with make build-playground: no {root / 'index.html'}")


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(root), **kwargs)

    def log_message(self, *_args):
        pass

    def do_GET(self):
        if not self.path.startswith("/dxutil/"):
            return self.send_error(404)
        self.path = self.path.removeprefix("/dxutil")
        return super().do_GET()


http.server.ThreadingHTTPServer(
    ("127.0.0.1", int(os.environ.get("DXPLAYGROUND_PORT", "4174"))), Handler
).serve_forever()
