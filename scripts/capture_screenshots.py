#!/usr/bin/env python3
"""采集真实运行截图到 docs/screenshots/。

每张图的文字内容都来自**真实命令 stdout**（或真实导出的文件内容），
由 Pillow 渲染成终端回显位图；不是设计稿、不是概念图。
无法在自动化环境采集 OS 原生窗口 / 托盘像素时，用同一次运行的窗口快照投影
与托盘菜单回显代替，并在 README「概念 / 实测对照」诚实标注差距。
"""
import os
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/screenshots"
FONT_CANDIDATES = [
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
]
BG = (22, 27, 35)
BAR = (31, 58, 95)
FG = (226, 234, 244)
MUTED = (140, 155, 175)


def load_font(size):
    for p in FONT_CANDIDATES:
        if Path(p).exists():
            try:
                return ImageFont.truetype(p, size)
            except Exception:
                continue
    return ImageFont.load_default()


def run_cli(args):
    env = os.environ.copy()
    env["CARGO_HTTP_PROXY"] = env.get("CARGO_HTTP_PROXY") or env.get("HTTPS_PROXY", "")
    proc = subprocess.run(
        ["cargo", "run", "-q", "-p", "dfa-cli", "--"] + args,
        cwd=ROOT, env=env, capture_output=True, text=True,
    )
    return proc.stdout.strip() or f"(命令无 stdout，退出码 {proc.returncode})"


def render(title, body, filename, subtitle):
    lines = body.splitlines()
    font = load_font(16)
    small = load_font(13)
    width = 1180
    line_h = 24
    height = 88 + max(8, len(lines)) * line_h + 70
    img = Image.new("RGB", (width, height), BG)
    d = ImageDraw.Draw(img)
    d.rectangle([0, 0, width, 52], fill=BAR)
    d.text((20, 14), title, font=load_font(20), fill=(255, 255, 255))
    d.text((20, 60), subtitle, font=small, fill=MUTED)
    y = 88
    for line in lines:
        # 终端等宽风格：过长行按字符数折行，不截断真实内容。
        while len(line) > 96:
            d.text((20, y), line[:96], font=font, fill=FG)
            y += line_h
            line = line[96:]
        d.text((20, y), line, font=font, fill=FG)
        y += line_h
    d.line([(20, height - 44), (width - 20, height - 44)], fill=(60, 72, 88))
    d.text(
        (20, height - 34),
        "真实终端回显 · 由命令 stdout / 真实导出文件渲染，非设计稿或概念图",
        font=small,
        fill=MUTED,
    )
    path = OUT / filename
    img.save(path, "PNG")
    return path


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    produced = []

    view = run_cli(["view"])
    produced.append(render(
        "① 窗口两栏快照投影（真实运行回显）",
        view,
        "01-window-projection.png",
        "命令：cargo run -p dfa-cli -- view ｜ 左栏注册表 + 右栏簇卡片 + 状态栏",
    ))

    tray = run_cli(["tray"])
    produced.append(render(
        "② 托盘计数与菜单（真实运行回显）",
        tray,
        "02-tray-count.png",
        "命令：cargo run -p dfa-cli -- tray ｜ 常驻进程侧计数与菜单项",
    ))

    export = run_cli(["export", "--out", "out/export_clusters.csv"])
    csv_path = ROOT / "out/export_clusters.csv"
    csv_text = ""
    if csv_path.exists():
        rows = csv_path.read_text(encoding="utf-8").splitlines()[:12]
        csv_text = "\n--- out/export_clusters.csv（真实导出片段）---\n" + "\n".join(rows)
    produced.append(render(
        "③ 簇清单导出与终端回显（真实运行产物）",
        export + csv_text,
        "03-cluster-export.png",
        "命令：cargo run -p dfa-cli -- export --out out/export_clusters.csv",
    ))

    doctor = run_cli(["doctor"])
    snapshot = run_cli(["snapshot", "--source", "fixtures/workorder_stub", "--out", "out/clusters.csv"])
    produced.append(render(
        "④ doctor 自检与 snapshot 簇清单（真实终端会话）",
        doctor + "\n--- snapshot ---\n" + snapshot,
        "04-doctor-snapshot.png",
        "命令：cargo run -p dfa-cli -- doctor / snapshot --source fixtures/workorder_stub",
    ))

    for p in produced:
        print(f"screenshot: {p} ({p.stat().st_size} bytes)")
    print(f"共 {len(produced)} 张真实运行截图")


if __name__ == "__main__":
    main()
