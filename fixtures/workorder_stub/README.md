# workorder_stub · 本地协议替身

> **仅测试数据层，不是最终对接方式。** 真实对接走 `WORKORDER_BASE_URL` 指向的院内工单系统；本替身只用于契约测试与 `scripts/verify.sh` 的端到端演练。

- `server.py`：HTTP 替身，校验 `POST /api/v1/work-orders` 的 10 个必填字段、`Authorization: Bearer`、`Idempotency-Key = sha256(cluster_id|workorder_no)`；同键二次提交返回同一单号；`STUB_FAIL_WORKORDERS` 可注入指定工单的 5xx。
- `workorders.json` / `devices.json` / `teams.json`：与 `fixtures/*_sample.json` 一致的脱敏样例。
- 生产代码路径（`crates/dfa-workorder/src/`）不出现替身地址，地址只由环境变量注入。
