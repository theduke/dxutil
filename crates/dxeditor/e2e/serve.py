"""Serve Dioxus output with SPA route fallback for the editor harness."""
import http.server
import os
from pathlib import Path

root = Path(os.environ["DXEDITOR_HARNESS_DIR"]).resolve()
if not (root / "index.html").is_file():
    raise SystemExit(f"Build the harness first: no {root / 'index.html'}")


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(root), **kwargs)

    def log_message(self, *_args):
        pass

    def do_GET(self):
        target = root / self.path.split("?", 1)[0].lstrip("/")
        if not target.is_file() and "." not in target.name:
            self.path = "/index.html"
        return super().do_GET()


http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ.get("DXEDITOR_PORT", "4173"))), Handler).serve_forever()
