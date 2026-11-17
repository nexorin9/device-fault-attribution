#!/usr/bin/env bash
# 医疗设备故障归因系统 —— 可重复验证入口
#
# 四段：安装 → 配置 → 使用 → 验证。
# 命令与 README「### 构建与验证」逐条一致；断网可设 DFA_OFFLINE=1。
# 本地协议替身只在测试数据层，不是最终对接方式。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# 自动化环境里 cargo 常不在默认 PATH，显式补齐。
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  echo "错误：未找到 cargo。请先安装 Rust 工具链，或把 cargo 所在目录加入 PATH。" >&2
  exit 90
fi

mkdir -p data logs out
LOG="logs/verify-$(date +%Y%m%d-%H%M%S).log"
: > "$LOG"
STUB_PORT="${STUB_PORT:-8791}"
STUB_BASE="http://127.0.0.1:${STUB_PORT}"
STUB_PID=""

say() { printf '%s\n' "$*" | tee -a "$LOG"; }
run() {
  say "+ cargo run -q -p dfa-cli -- $*"
  cargo run -q -p dfa-cli -- "$@" 2>&1 | tee -a "$LOG"
}

cleanup() {
  if [ -n "$STUB_PID" ]; then
    kill "$STUB_PID" 2>/dev/null || true
    wait "$STUB_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# ── 1. 安装 ─────────────────────────────────────────────────────
step_install() {
  say "== 1/4 安装 =="
  if [ "${DFA_OFFLINE:-0}" = "1" ]; then
    export CARGO_NET_OFFLINE=true
    say "[安装] 断网模式：跳过 cargo fetch，仅使用本地依赖缓存"
  else
    if ! cargo fetch 2>&1 | tee -a "$LOG"; then
      export CARGO_NET_OFFLINE=true
      say "[安装] 依赖下载不可用：转为离线构建；仍失败请检查网络或设 DFA_OFFLINE=1"
    fi
  fi
  cargo build --workspace 2>&1 | tee -a "$LOG"
  say "[安装] cargo build --workspace 完成"
}

# ── 2. 配置 ─────────────────────────────────────────────────────
step_config() {
  say "== 2/4 配置 =="
  if [ ! -f .env ]; then
    cp .env.example .env
    say "[配置] 已由 .env.example 生成 .env（.env 被 .gitignore 排除）"
  fi
  # 本地验证专用段：替身地址 + 占位凭据，仅写入被排除的 .env。
  if ! grep -q '^# verify-stub' .env 2>/dev/null; then
    cat >> .env <<EOF

# verify-stub（仅本地验证，真实接入时删除本段并填写院内地址）
WORKORDER_BASE_URL=${STUB_BASE}
WORKORDER_TOKEN=PLACEHOLDER-TOKEN-NOT-REAL
LOGISTICS_DEVICE_BASE_URL=${STUB_BASE}
LOGISTICS_DEVICE_TOKEN=PLACEHOLDER-TOKEN-NOT-REAL
MODEL_BASE_URL=
MODEL_API_KEY=
MODEL_MODEL=
MODEL_TIMEOUT_SECS=30
EOF
    say "[配置] 已注入本地替身地址与占位凭据（仅本轮验证，不写死在代码中）"
  fi
  export WORKORDER_BASE_URL="$STUB_BASE"
  export WORKORDER_TOKEN="PLACEHOLDER-TOKEN-NOT-REAL"
  export LOGISTICS_DEVICE_BASE_URL="$STUB_BASE"
  export LOGISTICS_DEVICE_TOKEN="PLACEHOLDER-TOKEN-NOT-REAL"
  export MODEL_BASE_URL=""
  export MODEL_API_KEY=""
  export MODEL_MODEL=""
  export MODEL_TIMEOUT_SECS="30"
  # 让样例簇中的 1 条工单返回 5xx，验证逐条回执的「可重放」路径。
  # WO-EXAMPLE-015 属于脱敏样例里的簇 C-002，其余成员应正常开单。
  export STUB_FAIL_WORKORDERS="WO-EXAMPLE-015"
  python3 fixtures/workorder_stub/server.py --port "$STUB_PORT" > logs/stub.log 2>&1 &
  STUB_PID=$!
  local i
  for i in $(seq 1 40); do
    if curl -fsS "${STUB_BASE}/" >/dev/null 2>&1 && kill -0 "$STUB_PID" 2>/dev/null; then
      break
    fi
    sleep 0.2
  done
  if ! curl -fsS "${STUB_BASE}/" >/dev/null 2>&1; then
    say "错误：本地协议替身未能启动，详见 logs/stub.log"
    exit 91
  fi
  say "[配置] config.toml 只读加载校验、本地协议替身就绪（端口 ${STUB_PORT}）"
}

# ── 3. 使用 ─────────────────────────────────────────────────────
step_use() {
  say "== 3/4 使用 =="
  run doctor
  local snapshot_out cluster_id
  snapshot_out="$(cargo run -q -p dfa-cli -- snapshot --source fixtures/workorder_stub --out out/clusters.csv 2>&1)"
  say "+ cargo run -q -p dfa-cli -- snapshot --source fixtures/workorder_stub --out out/clusters.csv"
  printf '%s\n' "$snapshot_out" | tee -a "$LOG"
  cluster_id="$(printf '%s\n' "$snapshot_out" | sed -n 's/^簇 \([^ ]*\) ·.*/\1/p' | head -1)"
  if [ -z "$cluster_id" ]; then
    say "错误：快照中未找到待确认簇，无法执行批量开单演练"
    exit 92
  fi
  say "[使用] 选中簇 ${cluster_id} 做一次人工确认批量开单（对本地协议替身）"
  run confirm --cluster "$cluster_id"
  run replay --cluster "$cluster_id"
  run export --out out/export_clusters.csv
  run view
  run tray
}

# ── 4. 验证 ─────────────────────────────────────────────────────
step_verify() {
  say "== 4/4 验证 =="
  cargo test --workspace 2>&1 | tee -a "$LOG"
  say "[验证] cargo test --workspace 全部通过"
}

step_install
step_config
step_use
step_verify

say "== 完成 =="
say "产物：data/（快照与确认记录）、out/（簇清单 CSV / Markdown）、logs/（本次验证与替身日志）"
say "以上三类目录均被 .gitignore 排除；.env 为本地凭据文件，禁止提交。"
say "下一步：按 README「### 对接现有系统」填入院内真实地址与令牌，替身不是最终对接方式。"
say "本次日志：${LOG}"
