#!/usr/bin/env python3
"""真实规模度量：调用产品代码（dfa-cli measure）在 >= 1000 条工单流上跑一轮
差分与成簇，把幂等键唯一性 / 簇内一致性 / 逐条回执完整率 / 单轮差分耗时 /
内存驻留写入 docs/scale-metrics.md。
"""
import json
import os
import platform
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus/scale.jsonl"
REPORT = ROOT / "docs/scale-metrics.md"


def main():
    if not CORPUS.exists():
        raise SystemExit("缺少规模语料：请先运行 python3 scripts/gen_scale_corpus.py")
    n_lines = sum(1 for l in CORPUS.read_text(encoding="utf-8").splitlines() if l.strip())
    if n_lines < 1000:
        raise SystemExit(f"规模不足：{n_lines} < 1000")
    env = os.environ.copy()
    env["CARGO_HTTP_PROXY"] = env.get("CARGO_HTTP_PROXY") or env.get("HTTPS_PROXY", "")
    proc = subprocess.run(
        ["cargo", "run", "-q", "-p", "dfa-cli", "--", "measure", "--input", str(CORPUS)],
        cwd=ROOT, env=env, capture_output=True, text=True,
    )
    if proc.returncode != 0:
        print(proc.stdout, proc.stderr, file=sys.stderr)
        raise SystemExit("规模度量命令运行失败")
    # cargo run 可能把编译提示写到 stdout，取最后一个 JSON 对象。
    text = proc.stdout.strip()
    start = text.find("{")
    m = json.loads(text[start:])
    checks = {
        "幂等键唯一性 = 100%": abs(m["idempotency_uniqueness"] - 1.0) < 1e-9,
        "簇内一致性 >= 99%": m["cluster_consistency"] >= 0.99,
        "逐条回执完整率 = 100%": abs(m["receipt_completeness"] - 1.0) < 1e-9,
        "单轮差分耗时 <= 200ms/1000 条": m["single_diff_elapsed_ms"] <= 200,
    }
    report = f"""# 簇引擎真实规模度量

| 项目 | 数值 |
|---|---|
| 语料条数 | {n_lines}（corpus/scale.jsonl，含缺字段与重复行） |
| 运行机器 | {platform.system()} {platform.machine()} / Python {platform.python_version()} / CPU {os.cpu_count()} |
| 产品判定器 | `dfa-cli measure` → `dfa-cluster::engine::measure` |
| 度量时间口径 | `std::time::Instant` 实测单次逐 tick 差分耗时 |

| 指标 | 实测 | 验收阈值 | 结论 |
|---|---|---|---|
| 幂等键唯一数 / 总数 | {m['unique_idempotency_keys']} / {m['total_members']} | 唯一性 = 100% | {'通过' if checks['幂等键唯一性 = 100%'] else '未通过'}（{m['idempotency_uniqueness']:.4f}） |
| 簇内一致性（同键成员硬约束一致率） | {m['cluster_consistency']:.4f} | >= 99% | {'通过' if checks['簇内一致性 >= 99%'] else '未通过'} |
| 逐条回执完整率 | {m['receipt_completeness']:.4f} | = 100% | {'通过' if checks['逐条回执完整率 = 100%'] else '未通过'} |
| 单轮差分耗时 | {m['single_diff_elapsed_ms']:.3f} ms | <= 200 ms / 1000 条 | {'通过' if checks['单轮差分耗时 <= 200ms/1000 条'] else '未通过'} |
| 内存驻留（结构估算） | {m['resident_bytes']} 字节 | 记录项 | 已落表 |

- 簇成员总数 {m['total_members']}，语料总行数 {m['total_workorders']}；缺字段与重复行按「不可比 / 去重」分流，不计入簇成员。
- 确认预算断言（`max(新预算, 携带预算−1)` 在 >= 5 tick 序列恒成立）由 `cargo test -p dfa-cluster --test confirm_budget` 覆盖。
- 全部阈值断言：{'通过' if all(checks.values()) else '未通过'}。
"""
    REPORT.write_text(report, encoding="utf-8")
    print(f"规模度量完成：{REPORT} · {n_lines} 条 · 差分 {m['single_diff_elapsed_ms']:.3f} ms")
    if not all(checks.values()):
        for k, v in checks.items():
            if not v:
                print(f"未通过：{k}", file=sys.stderr)
        raise SystemExit("规模度量阈值未通过")


if __name__ == "__main__":
    main()
