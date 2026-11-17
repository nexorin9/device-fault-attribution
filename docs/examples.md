# 输出样例与终端回显

> 所有标识均为占位形式，不含真实批次号、序列号或患者标识。`scripts/verify.sh` 可复现下列输出。

## doctor 检查项

```text
检查项 | 结论 | 建议动作
--- | --- | ---
WORKORDER_BASE_URL | 已配置 | —
WORKORDER_TOKEN | 已配置 | —
MODEL_BASE_URL | 降级可用 | 未配置时走纯规则判定，结论标「待人工判断」
config.toml 可解析 | 通过 | 只读加载，未回写
词表改名迁移 | 通过 | 词表标识符：无待迁移改名
工单源连通性 | 通过 | —
```

缺凭据时输出 `ERR_CREDENTIAL_MISSING：缺 WORKORDER_TOKEN …`，并给 `.env.example` 取值位置，不打印任何密钥值。

## snapshot 只读快照

```text
簇数：2（单条工单不成簇）
待核实工单：4 条
不可比工单：2 条
词表外待人工归类：1 条
运行摘要：扫描=17 新增=17 复现=0 成簇=2 降级=0 出站写=0 脏行=0 截断=0
未经人工确认：出站写请求计数 = 0；簇状态与工单状态均未推进。
簇 C-002 · 观察中 · 4 台设备 · REFUSAL_TO_COOL
```

## confirm 逐条回执（对本地协议替身）

```text
批量开单：4 张草稿，出站请求 10 次，逐条回执如下：
- WO-EXAMPLE-001 · 成功 · http=201 · 可重放=false
- WO-EXAMPLE-015 · 可重放 · http=503 · 可重放=true
```

失败项复用原 `Idempotency-Key` 重放，已成功项不重复提交。

## 窗口两栏投影与托盘

`dfa-cli view` 输出左栏注册表 / 待核实与右栏簇卡片；`dfa-cli tray` 输出待处理条数、新增簇数与菜单项。真实截图见 `docs/screenshots/`。

## 评测与规模度量

`docs/clustering-benchmark.md` 记录本轮判定器与关键词基线的误并 / 漏并率；`docs/scale-metrics.md` 记录 ≥1000 条工单流下的幂等键唯一性、簇内一致性、逐条回执完整率与单轮差分耗时。
