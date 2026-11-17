# 架构、审计链与状态机

## 数据流

```text
入站三路（只读 · Bearer 凭据取自环境变量）
  设备台账 GET /devices       ─┐
  故障工单 GET /work-orders   ─┼─► dfa-ingest（就绪三值 / 扇入去重 / 退避 / 生命周期）
  班组能力 GET /teams        ─┘              │
                                             ▼
                       dfa-registry（序列号 + 型号双因子 · 歧义拒绝 → 待核实）
                                             │
                                             ▼
   dfa-cluster 差分三态 → 硬约束门 → 词表精确 → 别名回退 → 同源成簇
   dfa-attribution 语义判定 / 根因草稿 / 步骤排序 / 规则守门
   dfa-model 单一 OpenAI-compatible 网关（不可达即降级纯规则）
                                             │ 人工确认（唯一推进者）
                                             ▼
   dfa-workorder（写前校验 → 幂等键 → 逐条 POST → 逐条回执）
                                             │
                                             ▼
   dfa-store（SQLite · 只增不改审计链 · 快照原子写）
                          ┌────────────────┴────────────────┐
                          ▼                                 ▼
              dfa-desktop 窗口（只读快照）        dfa-cli doctor / snapshot / replay
              + 系统托盘计数与横幅                  （每次调用 5 秒硬上限）
```

## 确定性判定顺序

1. 硬约束（型号 / 固件 / 批次 / 时间窗）过滤候选集；缺字段判「不可比」，不猜测。
2. 受控词表精确匹配 → 别名表回退 → 模型语义同一化；词表外一律 `PENDING_MANUAL`。
3. 确认预算与簇状态机（观察中 → 已确认同源 → 已开单 → 已闭环）；唯一推进者是人工确认。
4. 写前 10 字段与枚举校验 → `Idempotency-Key: sha256(cluster_id|workorder_no)` → 逐条回执。

## 只增不改审计链

每条审计条目 `entry_hash = sha256(prev_hash + 规范化条目)`，规范化串为
`seq|ts|actor|action|target|payload_digest|prev_hash|cluster_id`。篡改任一字段都会使 `verify_audit_chain()` 失败。回执、确认动作、PII 剥离、模型调用元数据均落在链上，可按簇反查「为何并进这个簇」。

## 降级边界

模型超时 / 连接失败 / 5xx / 不可解析 / schema 不符，退避重试 ≤2 次后降级为纯规则判定，标「规则判定 · 待人工判断」。降级前后簇状态、工单状态、审计链条目数 diff = 0，出站写请求计数 = 0。
