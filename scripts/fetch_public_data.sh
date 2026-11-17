#!/usr/bin/env bash
# 公开数据源脚本化下载 + 完整性校验 + 断网降级（可重跑、幂等）。
# 数据只落 data/public/（被 .gitignore 排除，不进仓库）；绝不伪造数据。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA="$ROOT/data/public"
RECORDS="$DATA/records.json"
SNAPSHOT="$ROOT/corpus/openfda_snapshot.json"
RAW1="$DATA/raw_recall.json"
RAW2="$DATA/raw_event.json"
OFFLINE="${DFA_OFFLINE:-0}"

mkdir -p "$DATA" "$ROOT/corpus"

fail_with_guidance() {
  echo "错误：$1" >&2
  echo "获取指引：在有网络的环境执行 bash scripts/fetch_public_data.sh；" >&2
  echo "或把一份真实 openFDA 公开记录快照放到 corpus/openfda_snapshot.json 后重跑。" >&2
  echo "本脚本绝不生成伪造数据。" >&2
  exit 3
}

use_snapshot() {
  local reason="$1"
  if [ -f "$RECORDS" ]; then
    echo "[降级] $reason → 使用 data/public/records.json 本地缓存"
  elif [ -f "$SNAPSHOT" ]; then
    echo "[降级] $reason → 使用仓库附带的真实 openFDA 快照（非伪造数据）"
    cp "$SNAPSHOT" "$RECORDS"
  else
    fail_with_guidance "$reason，且首次运行无任何缓存或公开数据快照"
  fi
}

if [ "$OFFLINE" = "1" ]; then
  use_snapshot "断网模拟（DFA_OFFLINE=1）"
else
  ok=1
  curl -fsS --max-time 25 "https://api.fda.gov/device/recall.json?limit=160" -o "$RAW1" || ok=0
  curl -fsS --max-time 25 "https://api.fda.gov/device/event.json?limit=160" -o "$RAW2" || ok=0
  if [ "$ok" != "1" ]; then
    use_snapshot "公开接口不可达"
  else
    python3 "$ROOT/scripts/_normalize_public.py" "$RAW1" "$RAW2" "$RECORDS" || fail_with_guidance "公开数据规范化 / 完整性校验未通过"
    cp "$RECORDS" "$SNAPSHOT"
    echo "[下载] 已写入 ${RECORDS} (真实 openFDA 公开记录)"
  fi
fi

# 完整性校验（无论在线还是降级路径都执行）：条数 / 关键字段非空率 / 去重稳定。
python3 - "$RECORDS" <<'PYEOF'
import json, sys, hashlib
p = sys.argv[1]
try:
    with open(p, encoding="utf-8") as f:
        data = json.load(f)
except Exception as e:
    print(f"校验失败：记录文件不可读：{e}", file=sys.stderr); sys.exit(4)
records = data.get("records", [])
if len(records) < 200:
    print(f"校验失败：记录条数 {len(records)} < 200", file=sys.stderr); sys.exit(4)
key_fields = ["model", "problem"]
nonempty = sum(1 for r in records if all(str(r.get(k, "")).strip() for k in key_fields))
rate = nonempty / len(records)
if rate < 0.8:
    print(f"校验失败：关键字段非空率 {rate:.3f} < 0.800", file=sys.stderr); sys.exit(4)
ids = [r.get("record_id", "") for r in records]
if len(set(ids)) != len(ids):
    print("校验失败：去重后条数不稳定", file=sys.stderr); sys.exit(4)
digest = hashlib.sha256(json.dumps(records, ensure_ascii=False, sort_keys=True).encode()).hexdigest()[:16]
print(f"完整性校验通过：记录 {len(records)} 条，关键字段非空率 {rate:.3f}，去重后 {len(set(ids))} 条稳定，digest={digest}")
print(f"产物：{p}")
PYEOF
