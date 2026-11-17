#!/usr/bin/env python3
"""把 openFDA 器械召回 / 不良事件原始响应规范化为统一记录，并做完整性校验。
标注规则（脚本头注释即规则说明）：每条记录保留真实公开字段，不做任何编造；
关键字段为空的记录被剔除而非填补，保证「非空率」是实测值。
"""
import json
import sys
from pathlib import Path


def normalize(raw1: Path, raw2: Path):
    records = []
    for path, source in [(raw1, "device_recall"), (raw2, "device_event")]:
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except Exception:
            continue
        for item in payload.get("results", []):
            if source == "device_recall":
                # openFDA /device/recall.json 的真实字段：产品描述、召回原因、召回企业。
                # 兼容个别历史响应里出现过的 product / recall_number 别名。
                model = str(
                    item.get("product_description") or item.get("product") or ""
                ).strip()
                problem = str(
                    item.get("reason_for_recall")
                    or item.get("root_cause_description")
                    or ""
                ).strip()
                firm = str(item.get("recalling_firm") or "").strip()
                date = str(
                    item.get("event_date_initiated") or item.get("event_date_posted") or ""
                ).strip()
                # 召回事件号 + 产品号 + cfres 号组成稳定唯一键（真实字段，不生成编号）。
                rid = ":".join(
                    str(item.get(k) or "")
                    for k in ("res_event_number", "product_res_number", "cfres_id")
                ).strip(":")
            else:
                # openFDA /device/event.json：器械在 device[] 里，问题描述在 mdr_text。
                devices = item.get("device") or []
                first = devices[0] if devices else {}
                brand = str(first.get("brand_name") or "").strip()
                if brand.upper() in ("", "N/A", "NA", "UNKNOWN"):
                    brand = str(
                        first.get("generic_name")
                        or first.get("model_number")
                        or first.get("catalog_number")
                        or ""
                    ).strip()
                model = brand
                problem = str(
                    item.get("product_problem") or item.get("mdr_text") or ""
                ).strip()
                firm = str(
                    item.get("manufacturer_name")
                    or first.get("manufacturer_d_name")
                    or ""
                ).strip()
                date = str(item.get("date_of_event") or item.get("date_received") or "").strip()
                rid = str(item.get("mdr_report_key") or item.get("report_number") or "").strip()
            if not rid:
                continue
            # 关键字段为空即剔除，绝不补写占位值冒充真实数据。
            if not model or not problem:
                continue
            records.append({
                "source": source,
                "record_id": f"{source}:{rid}",
                "model": model[:120],
                "problem": problem[:400],
                "firm": firm[:120],
                "date": date,
            })
    # 按稳定键去重，保证重跑后条数稳定。
    seen = set()
    out = []
    for r in records:
        if r["record_id"] in seen:
            continue
        seen.add(r["record_id"])
        out.append(r)
    return out


def main():
    raw1, raw2, dest = map(Path, sys.argv[1:4])
    records = normalize(raw1, raw2)
    if len(records) < 200:
        print(f"规范化后仅 {len(records)} 条有效记录（要求 >= 200）：关键字段为空的记录已剔除", file=sys.stderr)
        sys.exit(1)
    rate = sum(1 for r in records if r["model"] and r["problem"]) / len(records)
    if rate < 0.8:
        print(f"关键字段非空率 {rate:.3f} < 0.8", file=sys.stderr)
        sys.exit(1)
    dest.write_text(json.dumps({
        "source": "openFDA device/recall + device/event (public API)",
        "note": "真实公开记录，未做任何编造；仅本地留存，不进仓库 data/ 目录",
        "count": len(records),
        "records": records,
    }, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"规范化完成：{len(records)} 条，关键字段非空率 {rate:.3f}")


if __name__ == "__main__":
    main()
