#!/usr/bin/env python3
"""由公开语料分布特征扩展出 >= 1000 条工单流（corpus/scale.jsonl）。
按型号 / 固件 / 批次分组，含缺字段与重复行；规模与来源写入元数据。
"""
import json
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEED = 20261109
TARGET = 1200
CATEGORIES = {
    "REFUSAL_TO_COOL": ["不制冷", "没冷气", "制冷不给力"],
    "NO_RESPONSE": ["没反应", "按了没动静"],
    "ALARM_ONLY": ["只会滴滴响", "一直报警"],
    "INFUSION_BLOCKED": ["输不进去"],
    "POWER_FAILURE": ["掉电", "通电没反应"],
    "DISPLAY_ABNORMAL": ["屏幕花了", "屏幕闪烁"],
}


def main():
    random.seed(SEED)
    labels = ROOT / "corpus/labels.jsonl"
    base = [json.loads(l) for l in labels.read_text(encoding="utf-8").splitlines() if l.strip()]
    models = sorted({r["model"] for r in base})[:12] or ["EXAMPLE-MODEL"]
    cats = list(CATEGORIES)
    lines = []
    BASE = 1741000000
    for i in range(TARGET):
        model = models[i % len(models)]
        cat = cats[(i // 7) % len(cats)]
        lot = f"LOT-EXAMPLE-{(i // 40) % 6 + 1:02d}"
        fw = "FW-2.1.4-EXAMPLE" if i % 5 else "FW-2.2.0-EXAMPLE"
        # 每 97 行注入一条缺字段、每 113 行注入一条重复行
        if i % 97 == 0:
            fw = ""
        if i % 113 == 0 and lines:
            dup = dict(lines[-1])
            dup["workorder_no"] = lines[-1]["workorder_no"] + "-DUP"
            lines.append(dup)
            continue
        lines.append({
            "workorder_no": f"SCALE-{i:05d}",
            "device_id": f"SCALE-DEV-{i:05d}",
            "serial": f"SER-SCALE-{i:05d}",
            "model": model,
            "firmware": fw,
            "purchase_batch": lot,
            "ward": ["重症医学科", "急诊科", "手术室", "供应室"][i % 4],
            "fault_text": random.choice(CATEGORIES[cat]),
            "reported_at": "2025-03-03 08:12:00",
            "reported_epoch": BASE + i * 30,
            "reporter_dept": "设备科",
            "fault_code": "",
        })
    out = ROOT / "corpus/scale.jsonl"
    out.write_text("".join(json.dumps(x, ensure_ascii=False) + "\n" for x in lines), encoding="utf-8")
    meta = {
        "seed": SEED,
        "requested": TARGET,
        "actual_lines": len(lines),
        "distinct_models": len(models),
        "groups": "按型号 / 固件 / 到货批次分组",
        "contains_missing_fields": True,
        "contains_duplicate_rows": True,
        "source": "corpus/labels.jsonl 的公开语料分布特征扩展",
    }
    (ROOT / "corpus/scale.meta.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"规模语料：{out} · {len(lines)} 条 · seed={SEED}")


if __name__ == "__main__":
    main()
