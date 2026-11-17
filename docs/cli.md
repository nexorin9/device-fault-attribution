# CLI 子命令参考

> 本文是 `README.md` 的参数层展开。CLI 只定位为运维与排障通道，业务主路径在桌面窗口。

## 命令一览

| 命令 | 用途 | 关键参数 | 退出码 |
|------|------|----------|--------|
| `doctor` | 8 个环境变量齐备性、`config.toml` 可解析性、词表改名迁移、三路源连通性 | 无 | 0 全通过；3 缺凭据 |
| `snapshot` | 只读导出当前簇状态到 stdout 与 CSV | `--source <目录>`、`--out <路径>` | 0 / 1（读失败） |
| `replay` | 离线重放判定链路，不写库、不出站 | `--cluster <簇号>` | 0 |
| `export` | 导出只含已确认同源簇的 CSV 与 Markdown | `--out <csv 路径>` | 0 / 1 |
| `confirm` | 人工确认并批量开单（真实出站写） | `--cluster <簇号>` | 0 / 1 / 4 |
| `view` | 窗口两栏的文本投影（无显示服务时的真实回显） | 无 | 0 |
| `tray` | 托盘计数与菜单回显 | 无 | 0 |
| `cluster` | 评测用：对 JSONL 语料跑判定器并输出分组 | `--input`、`--out` | 0 / 1 |
| `measure` | 规模度量：幂等键唯一性 / 簇内一致性 / 回执完整率 / 差分耗时 | `--input` | 0 / 1 |

每次调用对外部源有 5 秒硬上限；超时与断连返回可区分错误码，不 panic。`DFA_PROJECT_ROOT` 可显式指定项目根，`DFA_ACTOR` 指定确认操作人。

## 统一错误码

`ERR_SOURCE_UNAVAILABLE` / `ERR_CREDENTIAL_MISSING` / `ERR_FIELD_MISSING` / `ERR_VOCABULARY_OUT_OF_RANGE` / `ERR_IDEMPOTENCY_CONFLICT` / `ERR_MODEL_UNREACHABLE` / `ERR_ILLEGAL_TRANSITION` / `ERR_CAPABILITY_MISSING`。

## 只读边界

`snapshot`、`replay`、`view`、`tray`、`export` 均不写库、不出站；`export` 只读取 `data/confirmations.json` 中已人工确认的簇号。唯一改变外部业务状态的是 `confirm`。
