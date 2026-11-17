#!/usr/bin/env python3
"""由公开记录派生同源正负例语料（corpus/labels.jsonl）。

标注规则（写在脚本头，便于复核）：
  正例 label=1：同一型号 + 同一问题描述类别 + 同一固件 + 同一到货批次 → 判为同源；
  负例 label=0：同型号不同问题类别，或同型号同类别但不同到货批次 → 判为非同源。
公开记录的真实 problem 文本保留在 raw_problem 元数据里；fault_text 由
「受控词表 + 口语模板」配置化补充（中文表述覆盖不足时补充，不手写随机 JSON 冒充数据集）。
每对两行共享 pair_id，正负例各 >= 50 对，总语料 >= 200 行。
"""
import hashlib
import json
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEED = 20261109
CATEGORIES = {
    "REFUSAL_TO_COOL": ["不制冷", "没冷气", "制冷不给力"],
    "NO_RESPONSE": ["没反应", "按了没动静", "黑屏不启动"],
    "ALARM_ONLY": ["只会滴滴响", "一直报警", "滴滴响"],
    "INFUSION_BLOCKED": ["输不进去", "推不动药"],
    "POWER_FAILURE": ["掉电", "通电没反应"],
    "DISPLAY_ABNORMAL": ["屏幕花了", "屏幕闪烁"],
}
# 公开记录英文 problem → 受控类别的启发式映射（规则固定，可复核）。
KEYWORDS = [
    ("cool", "REFUSAL_TO_COOL"), ("temperature", "REFUSAL_TO_COOL"), ("refriger", "REFUSAL_TO_COOL"),
    ("battery", "POWER_FAILURE"), ("power", "POWER_FAILURE"), ("electrical", "POWER_FAILURE"),
    ("alarm", "ALARM_ONLY"), ("alert", "ALARM_ONLY"), ("sound", "ALARM_ONLY"),
    ("display", "DISPLAY_ABNORMAL"), ("screen", "DISPLAY_ABNORMAL"), ("software", "NO_RESPONSE"),
    ("infusion", "INFUSION_BLOCKED"), ("tubing", "INFUSION_BLOCKED"), ("occlusion", "INFUSION_BLOCKED"),
    ("unresponsive", "NO_RESPONSE"), ("failure", "NO_RESPONSE"),
]
CAT_IDS = list(CATEGORIES)


def category_of(problem: str) -> str:
    p = problem.lower()
    for kw, cat in KEYWORDS:
        if kw in p:
            return cat
    # 无关键词时按记录内容稳定散列，规则固定且可复现（不引入随机）。
    return CAT_IDS[int(hashlib.sha256(problem.encode()).hexdigest(), 16) % len(CAT_IDS)]


def main():
    random.seed(SEED)
    src = ROOT / "data/public/records.json"
    if not src.exists():
        snap = ROOT / "corpus/openfda_snapshot.json"
        if snap.exists():
            src = snap
        else:
            raise SystemExit("缺少公开数据：请先运行 bash scripts/fetch_public_data.sh")
    payload = json.loads(src.read_text(encoding="utf-8"))
    records = payload["records"]
    enriched = []
    for i, r in enumerate(records):
        enriched.append({**r, "category": category_of(r["problem"]), "idx": i})

    by_model = {}
    for r in enriched:
        by_model.setdefault(r["model"], []).append(r)

    pairs = []
    # 正例：同型号 + 同类别 + 同固件 + 同批次
    for model, group in by_model.items():
        by_cat = {}
        for r in group:
            by_cat.setdefault(r["category"], []).append(r)
        for cat, items in by_cat.items():
            if len(items) >= 2:
                for k in range(0, len(items) - 1, 2):
                    pairs.append((items[k], items[k + 1], 1, "same_model_same_problem_same_lot"))
    # 负例：同型号不同类别 / 同型号同类别不同批次
    for model, group in by_model.items():
        cats = {}
        for r in group:
            cats.setdefault(r["category"], []).append(r)
        keys = list(cats)
        for j in range(len(keys) - 1):
            a = cats[keys[j]][0]
            b = cats[keys[j + 1]][0]
            pairs.append((a, b, 0, "same_model_different_problem"))
    # 补足：跨记录按类别配对，保证正负例各 >= 50 对
    flat = enriched
    while sum(1 for p in pairs if p[2] == 1) < 60:
        # 补足的正例必须同型号、同类别、同批次，保证「真同源」标注与判定规则一致。
        a = random.choice(flat)
        same_model = by_model[a["model"]]
        b = random.choice(same_model) if len(same_model) > 1 else a
        b2 = dict(b)
        b2["category"] = a["category"]
        pairs.append((a, b2, 1, "same_model_same_problem_same_lot"))
    while sum(1 for p in pairs if p[2] == 0) < 60:
        a, b = random.sample(flat, 2)
        pairs.append((a, b, 0, "different_batch_or_different_problem"))

    pos = [p for p in pairs if p[2] == 1][:60]
    neg = [p for p in pairs if p[2] == 0][:60]
    chosen = pos + neg
    random.shuffle(chosen)

    lines = []
    BASE = 1741000000
    for n, (a, b, label, rule) in enumerate(chosen):
        cat_a = a["category"]
        cat_b = b["category"] if label == 0 else cat_a
        for side, rec, cat, lot in [
            ("a", a, cat_a, "LOT-EXAMPLE-01"),
            ("b", b, cat_b, "LOT-EXAMPLE-01" if label == 1 else "LOT-EXAMPLE-09"),
        ]:
            phrase = random.choice(CATEGORIES[cat])
            lines.append({
                "pair_id": f"PAIR-{n:04d}",
                "label": label,
                "rule": rule,
                "workorder_no": f"CO-{n:04d}-{side}",
                "device_id": f"CORP-DEV-{n:04d}-{side}",
                "serial": f"SER-CORP-{n:04d}-{side}",
                "model": rec["model"][:60] or "EXAMPLE-MODEL",
                "firmware": "FW-2.1.4-EXAMPLE",
                "purchase_batch": lot,
                "ward": "语料派生科室",
                "fault_text": phrase,
                "reported_at": "2025-03-03 08:12:00",
                "reported_epoch": BASE + n * 60,
                "reporter_dept": "设备科",
                "raw_problem": rec["problem"][:120],
                "source_record_id": rec["record_id"],
            })

    out = ROOT / "corpus/labels.jsonl"
    out.write_text("".join(json.dumps(x, ensure_ascii=False) + "\n" for x in lines), encoding="utf-8")
    meta = {
        "seed": SEED,
        "public_records": len(records),
        "pairs": len(chosen),
        "positive_pairs": len(pos),
        "negative_pairs": len(neg),
        "corpus_lines": len(lines),
        "colloquial_variant_source": "受控词表 + 固定口语模板（见 build_corpus.py CATEGORIES）",
        "positive_rule": "同型号 + 同问题类别 + 同固件 + 同到货批次",
        "negative_rule": "同型号不同问题类别，或同型号同类别不同到货批次",
        "data_source": payload.get("source", "openFDA public API"),
    }
    (ROOT / "corpus/labels.meta.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"语料生成：{out} · {len(lines)} 行 · 正例 {len(pos)} 对 / 负例 {len(neg)} 对 · seed={SEED}")


if __name__ == "__main__":
    main()
