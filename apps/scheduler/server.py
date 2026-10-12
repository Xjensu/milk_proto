"""Small internal HTTP adapter for the isolated local scheduling engine."""

import json
import os
import re
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import Request, urlopen
from urllib.error import HTTPError

if os.environ.get("DISCOVERY_ENABLED") == "1":
    import discovery

from analyzer import analyze
from preference_compiler import compile_wishes
from solver import generate


MAX_BODY_BYTES = 8 * 1024 * 1024
SOLVE_SLOT = threading.BoundedSemaphore(1)


class ProgressReporter:
    """Coalesce phase and incumbent updates so API I/O never blocks CP-SAT search."""

    def __init__(self, endpoint, run_id, progress_token, cancel_event):
        self.run_id = run_id
        self.url = f"{endpoint.rstrip('/')}/{run_id}/progress"
        self.progress_token = progress_token
        self.cancel_event = cancel_event
        self._condition = threading.Condition()
        self._started_at = time.monotonic()
        self._last_solver_update_at = self._started_at
        self._latest = {"phase": "preparing", "solutions": 0, "placed": 0, "total": 0,
                        "branches": 0, "conflicts": 0, "elapsed_seconds": 0.0,
                        "solver_idle_seconds": 0.0}
        self._pending = self._latest
        self._closed = False
        self._thread = threading.Thread(target=self._deliver, daemon=True)
        self._thread.start()

    def __call__(self, progress):
        with self._condition:
            self._last_solver_update_at = time.monotonic()
            self._latest = {**progress, "solver_idle_seconds": 0.0}
            self._pending = self._latest
            self._condition.notify()

    def close(self):
        with self._condition:
            self._closed = True
            self._condition.notify()
        self._thread.join(timeout=2.5)

    def _deliver(self):
        while True:
            with self._condition:
                while self._pending is None and not self._closed:
                    self._condition.wait(timeout=2.0)
                    if self._pending is None and not self._closed:
                        self._pending = dict(self._latest)
                        self._pending["elapsed_seconds"] = round(time.monotonic() - self._started_at, 2)
                        self._pending["solver_idle_seconds"] = round(
                            time.monotonic() - self._last_solver_update_at, 2
                        )
                progress = self._pending
                self._pending = None
                closed = self._closed
            if progress is not None:
                self._send(progress)
            if closed:
                return

    def _send(self, progress):
        try:
            url = (discovery.endpoint("api-geteway") + f"/admin/internal/schedule_generation_runs/{self.run_id}/progress"
                   if os.environ.get("DISCOVERY_ENABLED") == "1" else self.url)
        except Exception:
            return
        request = Request(url, data=json.dumps({"progress": progress}).encode(), method="POST",
                          headers={"Content-Type": "application/json",
                                   "X-Schedule-Generation-Progress-Token": self.progress_token})
        try:
            with urlopen(request, timeout=1.5):
                pass
        except HTTPError as error:
            if error.code == 404:
                self.cancel_event.set()
            error.close()
        except Exception:
            # Progress is advisory; the solver result remains authoritative.
            return


class Handler(BaseHTTPRequestHandler):
    server_version = "MilkScheduler/1"

    def do_GET(self):
        if self.path != "/health":
            return self._reply(404, {"error": "NOT_FOUND"})
        self._reply(200, {"status": "ok", "engine": "ortools-cp-sat", "schema_version": 1})

    def do_POST(self):
        routes = {
            "/v1/solve": self._solve,
            "/v1/analyze": self._analyze,
            "/v1/compile-preferences": self._compile,
        }
        action = routes.get(self.path)
        if action is None:
            return self._reply(404, {"error": "NOT_FOUND"})
        acquired = False
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length < 1 or length > MAX_BODY_BYTES:
                return self._reply(413, {"error": "INVALID_BODY_SIZE"})
            snapshot = json.loads(self.rfile.read(length))
            if not isinstance(snapshot, dict) or snapshot.get("schema_version") != 1:
                return self._reply(422, {"error": "UNSUPPORTED_SNAPSHOT_SCHEMA"})
            if self.path == "/v1/solve" and not SOLVE_SLOT.acquire(blocking=False):
                return self._reply(429, {"error": "SOLVER_BUSY"})
            acquired = self.path == "/v1/solve"
            result = action(snapshot)
            self._reply(200, result)
        except (ValueError, TypeError, KeyError, json.JSONDecodeError):
            self._reply(422, {"error": "INVALID_SNAPSHOT"})
        except Exception:
            # Do not leak snapshot contents or stack traces over the service API.
            self._reply(500, {"error": "SCHEDULER_FAILURE"})
        finally:
            if acquired:
                SOLVE_SLOT.release()

    def _solve(self, snapshot):
        seconds_header = self.headers.get("X-Solve-Seconds")
        seconds = int(seconds_header) if seconds_header is not None else None
        if seconds is not None and seconds < 1:
            raise ValueError("invalid solve limit")
        run_id = self.headers.get("X-Generation-Run-ID", "")
        progress_token = self.headers.get("X-Schedule-Generation-Progress-Token", "")
        reporter = None
        cancel_event = threading.Event()
        endpoint = ("discovery" if os.environ.get("DISCOVERY_ENABLED") == "1"
                    else os.environ.get("SCHEDULE_PROGRESS_API_URL", ""))
        if run_id and progress_token and endpoint and re.fullmatch(r"[0-9a-f-]{36}", run_id) \
                and re.fullmatch(r"[0-9a-f]{64}", progress_token):
            reporter = ProgressReporter(endpoint, run_id, progress_token, cancel_event)
        try:
            return generate(snapshot, seconds=seconds, on_progress=reporter, cancel_event=cancel_event)
        finally:
            if reporter:
                reporter.close()

    @staticmethod
    def _analyze(snapshot):
        return {"findings": analyze(snapshot, snapshot.get("sandbox_proposal", []))}

    @staticmethod
    def _compile(snapshot):
        return compile_wishes(snapshot)

    def _reply(self, status, payload):
        encoded = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(encoded)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(encoded)

    def log_message(self, _format, *_args):
        # Snapshot data is private and should never appear in container logs.
        return


if __name__ == "__main__":
    address = os.environ.get("SCHEDULER_BIND", "0.0.0.0")
    port = int(os.environ.get("SCHEDULER_PORT", "8100"))
    server = ThreadingHTTPServer((address, port), Handler)
    stop = discovery.start_registration(lambda: True) if os.environ.get("DISCOVERY_ENABLED") == "1" else None
    try:
        server.serve_forever()
    finally:
        if stop:
            stop.set()
        server.server_close()
