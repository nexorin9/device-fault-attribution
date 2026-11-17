#!/usr/bin/env python3
"""本地协议替身（仅测试数据层，不是最终对接方式）。

按 spec.md 的真实字段规范校验请求；同 Idempotency-Key 只产生一张单；
STUB_FAIL_WORKORDERS 可注入指定工单号的 5xx，用于失败与重放路径。
启动：python3 fixtures/workorder_stub/server.py --port 8791
"""
import argparse
import hashlib
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

REQUIRED = [
    "workorder_no", "device_id", "device_model", "firmware_version",
    "fault_code", "fault_text", "priority", "assignee_team",
    "source_ref", "cluster_id",
]
ALLOWED_PRIORITY = {"P1", "P2", "P3", "P4"}
CREATED = {}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def _send(self, code, payload):
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        self._send(200, {"status": "ready", "created": len(CREATED), "role": "test-stub-only"})

    def do_POST(self):
        if self.path.rstrip("/") != "/api/v1/work-orders":
            self._send(404, {"error": "not_found"})
            return
        length = int(self.headers.get("Content-Length", "0"))
        try:
            body = json.loads(self.rfile.read(length).decode("utf-8"))
        except Exception:
            self._send(400, {"error": "invalid_json"})
            return
        missing = [f for f in REQUIRED if f not in body or body[f] in ("", None)]
        if missing:
            self._send(400, {"error": "missing_fields", "fields": missing})
            return
        if body["priority"] not in ALLOWED_PRIORITY:
            self._send(400, {"error": "priority_out_of_range"})
            return
        auth = self.headers.get("Authorization", "")
        if not auth.startswith("Bearer "):
            self._send(401, {"error": "missing_bearer"})
            return
        key = self.headers.get("Idempotency-Key", "")
        expected = hashlib.sha256(
            f'{body["cluster_id"]}|{body["workorder_no"]}'.encode("utf-8")
        ).hexdigest()
        if not key:
            self._send(400, {"error": "missing_idempotency_key"})
            return
        if key != expected:
            self._send(400, {"error": "idempotency_key_mismatch", "expected": expected})
            return
        if body["workorder_no"] in set(filter(None, os.environ.get("STUB_FAIL_WORKORDERS", "").split(","))):
            self._send(503, {"error": "injected_failure", "workorder_no": body["workorder_no"]})
            return
        if key in CREATED:
            self._send(200, CREATED[key])
            return
        record = {
            "workorder_no": body["workorder_no"],
            "status": "created",
            "cluster_id": body["cluster_id"],
            "idempotency_key": key,
        }
        CREATED[key] = record
        self._send(201, record)

    def log_message(self, fmt, *args):
        return


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=int(os.environ.get("STUB_PORT", "8791")))
    ns = ap.parse_args()
    srv = ThreadingHTTPServer(("127.0.0.1", ns.port), Handler)
    print(f"stub-ready port={ns.port} role=test-only", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
