#!/usr/bin/env python3
"""簇引擎与「仅关键词匹配」基线的可复现对比。

本轮判定器由产品代码真实产出（dfa-cli cluster），基线在脚本内实现：
仅按工单自由文本关键词分组，不做硬约束、不做确认预算、不查注册表。
指标：聚类纯度（配对判定正确率）、误并率（不同源被并到一起）、
漏并率（同源未被并到一起）。固定随机种子重跑两次，对比表字节一致。
"""
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEED = 20261109
LABELS = ROOT / "corpus/labels.jsonl"
ASSIGNMENTS = ROOT / "corpus/cluster_assignments.jsonl"
REPORT = ROOT / "docs/clustering-benchmark.md"
KEYWORDS = ["不制冷", "没反应", "滴滴响", "输不进去", "花屏", "掉电", "报警", "故障"]


def baseline_group(text: str) -> str:
    for kw in KEYWORDS:
        if kw in text:
            return kw
    return "UNKNOWN"


def load_pairs():
    rows = [json.loads(l) for l in LABELS.read_text(encoding="utf-8").splitlines() if l.strip()]
    by_pair = {}
    for r in rows:
        by_pair.setdefault(r["pair_id"], []).append(r)
    return rows, [v for v in by_pair.values() if len(v) == 2]


def run_product_cluster():
    env = os.environ.copy()
    env["CARGO_HTTP_PROXY"] = env.get("CARGO_HTTP_PROXY") or env.get("HTTPS_PROXY", "")
    env.setdefault("PATH", os.environ.get("PATH", ""))
    proc = subprocess.run(
        ["cargo", "run", "-q", "-p", "dfa-cli", "--", "cluster",
         "--input", str(LABELS), "--out", str(ASSIGNMENTS)],
        cwd=ROOT, env=env, capture_output=True, text=True,
    )
    if proc.returncode != 0:
        print(proc.stdout, proc.stderr, file=sys.stderr)
        raise SystemExit("产品判定器运行失败")
    groups = {}
    for line in ASSIGNMENTS.read_text(encoding="utf-8").splitlines():
        if line.strip():
            d = json.loads(line)
            groups[d["workorder_no"]] = d["group"]
    return groups


def effective_group(groups: dict, workorder_no: str) -> str:
    """未判定 / 不可比 / 超窗的工单视为各自独立，不算被并到一起。"""
    g = groups.get(workorder_no, "MISSING")
    if g.startswith(("PENDING_MANUAL", "INCOMPARABLE", "EXPIRED", "MISSING")):
        return "SOLO:" + workorder_no
    return g


def metrics(pairs, group_fn):
    wrong_merge = 0
    miss = 0
    correct = 0
    for a, b in pairs:
        same = group_fn(a) == group_fn(b)
        if a["label"] == 0 and same:
            wrong_merge += 1
        elif a["label"] == 1 and not same:
            miss += 1
        else:
            correct += 1
    n_pos = sum(1 for a, _ in pairs if a["label"] == 1)
    n_neg = len(pairs) - n_pos
    return {
        "purity": correct / len(pairs),
        "false_merge_rate": wrong_merge / n_neg if n_neg else 0.0,
        "miss_rate": miss / n_pos if n_pos else 0.0,
        "wrong_merge": wrong_merge,
        "miss": miss,
        "correct": correct,
        "n_pos": n_pos,
        "n_neg": n_neg,
    }


def main():
    rows, pairs = load_pairs()
    if len(rows) < 200 or not pairs:
        raise SystemExit("语料不足：请先运行 build_corpus.py")
    groups = run_product_cluster()
    ours = metrics(pairs, lambda r: effective_group(groups, r["workorder_no"]))
    base = metrics(pairs, lambda r: baseline_group(r["fault_text"]))

    ok = ours["false_merge_rate"] <= base["false_merge_rate"] and ours["miss_rate"] <= base["miss_rate"]
    report = f"""# 簇引擎与基线对比（可复现）

> 基线：仅关键词匹配的同源判定（按工单自由文本关键词分组，无硬约束、无确认预算、不查注册表）。
> 对照：本轮差分 + 硬约束门 + 受控词表 + 别名回退 + 确认预算的判定器（由 `dfa-cli cluster` 真实产出）。

| 项目 | 数值 |
|---|---|
| 语料条数（工单行） | {len(rows)} |
| 配对数（正例 {ours['n_pos']} / 负例 {ours['n_neg']}） | {len(pairs)} |
| 随机种子 | {SEED} |
| 基线实现位置 | `scripts/eval_clustering.py::baseline_group`（产品内同名基线在 `crates/dfa-cluster/src/baseline.rs`，仅对比用） |
| 语料来源 | openFDA 器械召回 / 不良事件公开记录派生，见 `corpus/labels.meta.json` |

| 指标 | 本轮判定器 | 仅关键词基线 | 差值（本轮 − 基线） |
|---|---|---|---|
| 聚类纯度（配对判定正确率） | {ours['purity']:.4f} | {base['purity']:.4f} | {ours['purity'] - base['purity']:+.4f} |
| 误并率（不同源被并到一起） | {ours['false_merge_rate']:.4f} | {base['false_merge_rate']:.4f} | {ours['false_merge_rate'] - base['false_merge_rate']:+.4f} |
| 漏并率（同源未被并到一起） | {ours['miss_rate']:.4f} | {base['miss_rate']:.4f} | {ours['miss_rate'] - base['miss_rate']:+.4f} |

- 本轮误并 {ours['wrong_merge']} 例 / 基线 {base['wrong_merge']} 例；本轮漏并 {ours['miss']} 例 / 基线 {base['miss']} 例。
- 验收断言：本轮误并率 ≤ 基线 **且** 本轮漏并率 ≤ 基线 → **{'通过' if ok else '未通过'}**。
- 可复核：固定种子 `{SEED}` 依次运行 `python3 scripts/build_corpus.py` 与 `python3 scripts/eval_clustering.py`，本文件内容字节一致。
- 基线实现只用于对比验收，不进入产品主路径。
"""
    REPORT.write_text(report, encoding="utf-8")
    print(f"对比完成：{REPORT} · 本轮误并 {ours['false_merge_rate']:.4f} / 基线 {base['false_merge_rate']:.4f} · 本轮漏并 {ours['miss_rate']:.4f} / 基线 {base['miss_rate']:.4f}")
    if not ok:
        raise SystemExit("验收失败：本轮判定器未同时不劣于基线")
    print("验收通过：本轮误并率与漏并率均不劣于基线")


if __name__ == "__main__":
    main()
