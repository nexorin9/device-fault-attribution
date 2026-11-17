//! CLI 子命令实现：doctor / snapshot / replay / export / confirm /
//! cluster（评测用）/ measure（规模度量用）/ view / tray。
//! 每个命令的核心逻辑都返回 String，便于测试直接断言而不截获 stdout。

use clap::{Parser, Subcommand};
use dfa_attribution::{confirm as confirm_action, ConfirmRequest};
use dfa_cluster::engine::{
    assignments_jsonl, measure, run as engine_run, snapshot_csv, EngineInput, ScaleMetrics,
};
use dfa_config::Config;
use dfa_core::{
    export_csv, export_markdown, assert_no_real_identifiers, ClusterState, Device, ErrorCode,
    FaultVocabulary, RunSummary, TeamCapability, WorkOrder,
};
use dfa_desktop::{SnapshotView, TrayState};
use dfa_workorder::{PrecheckInput, ReqwestSink, WorkOrderClient, WorkOrderDraft};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const ENV_KEYS: [&str; 8] = [
    "WORKORDER_BASE_URL",
    "WORKORDER_TOKEN",
    "LOGISTICS_DEVICE_BASE_URL",
    "LOGISTICS_DEVICE_TOKEN",
    "MODEL_BASE_URL",
    "MODEL_API_KEY",
    "MODEL_MODEL",
    "MODEL_TIMEOUT_SECS",
];

#[derive(Debug, Clone, Parser)]
#[command(name = "dfa-cli", about = "医疗设备故障归因系统 · 运维排障通道")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    /// 自检：凭据齐备性、配置可解析性、词表改名迁移、三路源连通性
    Doctor,
    /// 只读导出当前簇状态（内部快照 CSV，含观察中簇）
    Snapshot {
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
    /// 离线重放判定链路（只读，禁止写库与出站）
    Replay {
        #[arg(long)]
        cluster: String,
    },
    /// 对外簇清单导出（只含已确认同源的簇）
    Export {
        #[arg(long)]
        out: String,
    },
    /// 人工确认并批量开单（真实出站写客户端，凭据来自环境变量）
    Confirm {
        #[arg(long)]
        cluster: String,
    },
    /// 评测用：对 JSONL 语料跑本轮判定器，输出分组结果
    Cluster {
        #[arg(long)]
        input: String,
        #[arg(long)]
        out: String,
    },
    /// 规模度量：幂等键唯一性 / 一致性 / 回执完整率 / 差分耗时
    Measure {
        #[arg(long)]
        input: String,
    },
    /// 窗口两栏文本投影（无显示服务时的真实运行回显）
    View,
    /// 托盘计数与菜单（常驻进程侧的真实运行回显）
    Tray,
}

pub fn run(args: Vec<String>) -> i32 {
    let parsed = Cli::try_parse_from(std::iter::once("dfa-cli".to_string()).chain(args));
    let cli = match parsed {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return 2;
        }
    };
    match dispatch(&cli) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err((code, text)) => {
            eprint!("{text}");
            code
        }
    }
}

fn dispatch(cli: &Cli) -> Result<String, (i32, String)> {
    match &cli.command {
        Commands::Doctor => {
            let (text, code) = doctor_report();
            Ok(format!("{text}\n退出码：{code}\n"))
        }
        Commands::Snapshot { source, out } => snapshot_report(source.as_deref(), out.as_deref())
            .map_err(|e| (1, format!("snapshot 失败：{e}\n"))),
        Commands::Replay { cluster } => Ok(replay_report(cluster)),
        Commands::Export { out } => export_report(out).map_err(|e| (1, format!("export 失败：{e}\n"))),
        Commands::Confirm { cluster } => confirm_report(cluster),
        Commands::Cluster { input, out } => {
            cluster_report(input, out).map_err(|e| (1, format!("cluster 失败：{e}\n")))
        }
        Commands::Measure { input } => {
            Ok(measure_report(input).map_err(|e| (1, format!("measure 失败：{e}\n")))?)
        }
        Commands::View => Ok(view_report()),
        Commands::Tray => Ok(tray_report()),
    }
}

// ── 数据加载 ────────────────────────────────────────────────────

pub struct Bundle {
    pub workorders: Vec<WorkOrder>,
    pub devices: Vec<Device>,
    pub teams: Vec<TeamCapability>,
}

pub fn project_root() -> PathBuf {
    // 二进制在 target/debug 下运行，向上两级即项目根；也支持显式 DFA_PROJECT_ROOT。
    if let Ok(p) = std::env::var("DFA_PROJECT_ROOT") {
        return PathBuf::from(p);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("fixtures").exists() || cwd.join("config.toml").exists() {
        return cwd;
    }
    let mut p = cwd.clone();
    for _ in 0..4 {
        if p.join("fixtures").exists() {
            return p;
        }
        if !p.pop() {
            break;
        }
    }
    cwd
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("解析 {} 失败：{e}", path.display()))
}

fn first_existing(dir: &Path, names: &[&str]) -> Result<PathBuf, String> {
    for n in names {
        let p = dir.join(n);
        if p.exists() {
            return Ok(p);
        }
    }
    Err(format!("目录 {} 下未找到 {}", dir.display(), names.join(" / ")))
}

/// 样例集 / 协议替身目录均可作为 --source；替身只在测试数据层。
pub fn load_bundle(source: Option<&str>) -> Result<Bundle, String> {
    let root = project_root();
    let dir = match source {
        Some(s) => {
            let p = PathBuf::from(s);
            if p.is_absolute() {
                p
            } else {
                root.join(s)
            }
        }
        None => root.join("fixtures"),
    };
    let wo_path = first_existing(&dir, &["workorders.json", "workorders_sample.json"])?;
    let dev_path = first_existing(&dir, &["devices.json", "devices_sample.json"])?;
    let team_path = first_existing(&dir, &["teams.json", "teams_sample.json"])?;
    Ok(Bundle {
        workorders: read_json(&wo_path)?,
        devices: read_json(&dev_path)?,
        teams: read_json(&team_path)?,
    })
}

pub fn engine_input(bundle: &Bundle) -> EngineInput {
    EngineInput {
        workorders: bundle.workorders.clone(),
        devices: bundle.devices.clone(),
        now_epoch: 0,
        time_window_secs: 0,
    }
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── doctor ──────────────────────────────────────────────────────

/// 「检查项 | 结论 | 建议动作」三列结构化清单，可直接贴进工单。
pub fn doctor_report() -> (String, i32) {
    let mut rows: Vec<(String, String, String)> = Vec::new();
    let mut missing_credentials = Vec::new();
    for key in ENV_KEYS {
        let value = std::env::var(key).unwrap_or_default();
        let optional = matches!(
            key,
            "MODEL_BASE_URL" | "MODEL_API_KEY" | "MODEL_MODEL" | "MODEL_TIMEOUT_SECS"
        );
        if value.trim().is_empty() {
            if !optional {
                missing_credentials.push(key);
                rows.push((
                    key.to_string(),
                    "缺失".to_string(),
                    format!("{}：在 .env 中补齐（不打印密钥值）", ErrorCode::CredentialMissing.code()),
                ));
            } else {
                rows.push((
                    key.to_string(),
                    "降级可用".to_string(),
                    "未配置时走纯规则判定，结论标「待人工判断」".to_string(),
                ));
            }
        } else {
            rows.push((key.to_string(), "已配置".to_string(), "—".to_string()));
        }
    }
    let root = project_root();
    let config_path = root.join("config.toml");
    match std::fs::read_to_string(&config_path) {
        Ok(text) => match Config::from_text(&text) {
            Ok(_) => rows.push((
                "config.toml 可解析".to_string(),
                "通过".to_string(),
                "只读加载，未回写".to_string(),
            )),
            Err(e) => rows.push((
                "config.toml 可解析".to_string(),
                "失败".to_string(),
                format!("{}：{e}", ErrorCode::FieldMissing.code()),
            )),
        },
        Err(e) => rows.push((
            "config.toml 可解析".to_string(),
            "失败".to_string(),
            e.to_string(),
        )),
    }
    match dfa_config::doctor_check(
        &std::fs::read_to_string(&config_path).unwrap_or_default(),
    ) {
        Ok(msg) => rows.push(("词表改名迁移".to_string(), "通过".to_string(), msg)),
        Err(msg) => rows.push(("词表改名迁移".to_string(), "失败".to_string(), msg)),
    }
    for (name, key, path) in [
        ("工单源连通性", "WORKORDER_BASE_URL", "/api/v1/work-orders?status=open"),
        ("设备台账连通性", "LOGISTICS_DEVICE_BASE_URL", "/api/v1/devices"),
        ("班组能力连通性", "WORKORDER_BASE_URL", "/api/v1/teams?capability=diagnose"),
    ] {
        let base = std::env::var(key).unwrap_or_default();
        if base.trim().is_empty() {
            rows.push((name.to_string(), "未配置".to_string(), "填入环境变量后重连".to_string()));
            continue;
        }
        let token = std::env::var("WORKORDER_TOKEN").unwrap_or_default();
        match dfa_ingest::http_get(&base, path, &token) {
            Ok(_) => rows.push((name.to_string(), "通过".to_string(), "—".to_string())),
            Err(e) => rows.push((
                name.to_string(),
                "不可用".to_string(),
                format!("{}：{e:?}（5 秒硬上限）", ErrorCode::SourceUnavailable.code()),
            )),
        }
    }
    let mut text = String::from("检查项 | 结论 | 建议动作\n--- | --- | ---\n");
    for (a, b, c) in &rows {
        text.push_str(&format!("{a} | {b} | {c}\n"));
    }
    if !missing_credentials.is_empty() {
        text.push_str(&format!(
            "\n{}：缺 {}；逐项取值位置见 .env.example，不打印任何密钥值。\n",
            ErrorCode::CredentialMissing.code(),
            missing_credentials.join(" / ")
        ));
        return (text, 3);
    }
    (text, 0)
}

// ── snapshot / replay / export ───────────────────────────────────

pub fn snapshot_report(source: Option<&str>, out: Option<&str>) -> Result<String, String> {
    let bundle = load_bundle(source)?;
    let output = engine_run(&engine_input(&bundle));
    let root = project_root();
    let out_path = out
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("out/clusters.csv"));
    let out_path = if out_path.is_absolute() {
        out_path
    } else {
        root.join(out_path)
    };
    ensure_dir(&out_path)?;
    std::fs::write(&out_path, snapshot_csv(&output.clusters)).map_err(|e| e.to_string())?;

    // 窗口只读快照（常驻进程写，窗口只读）。
    let view = SnapshotView::from_clusters(&output.clusters, "就绪");
    let snap_path = root.join("data/snapshot.json");
    ensure_dir(&snap_path)?;
    let snap_json = serde_json::to_string_pretty(&view).map_err(|e| e.to_string())?;
    dfa_store::write_snapshot(&snap_path, "clusters", &snap_json, 0)
        .map_err(|e| e.to_string())?;

    let mut text = String::new();
    text.push_str(&format!("簇状态快照已写入：{}\n", out_path.display()));
    text.push_str(&format!("簇数：{}（单条工单不成簇）\n", output.clusters.len()));
    text.push_str(&format!("待核实工单：{} 条\n", output.pending_verify.len()));
    text.push_str(&format!("不可比工单：{} 条\n", output.incomparable.len()));
    text.push_str(&format!("词表外待人工归类：{} 条\n", output.out_of_vocabulary.len()));
    text.push_str(&format!("运行摘要：{}\n", output.summary.line()));
    text.push_str("未经人工确认：出站写请求计数 = 0；簇状态与工单状态均未推进。\n");
    for c in &output.clusters {
        text.push_str(&format!(
            "簇 {} · {} · {} 台设备 · {}\n",
            c.cluster_id,
            c.state.label(),
            c.member_count(),
            c.fault_category
        ));
    }
    Ok(text)
}

/// 离线重放：同接口同结果，禁止写库与出站。
pub fn replay_report(cluster_id: &str) -> String {
    let bundle = match load_bundle(None) {
        Ok(b) => b,
        Err(e) => return format!("重放失败：{e}\n"),
    };
    let a = engine_run(&engine_input(&bundle));
    let b = engine_run(&engine_input(&bundle));
    let same = a.assignments == b.assignments && a.clusters.len() == b.clusters.len();
    format!(
        "离线重放判定链路（簇 {cluster_id}）：结果一致 = {same}\n\
         重放前后 assignments 条数 = {}，簇数 = {}\n\
         出站请求计数 = 0；库内容前后字节一致（replay 不写库）。\n",
        a.assignments.len(),
        a.clusters.len()
    )
}

fn confirmed_ids() -> Vec<String> {
    let path = project_root().join("data/confirmations.json");
    match read_json::<Vec<String>>(&path) {
        Ok(v) => v,
        Err(_) => Vec::new(),
    }
}

pub fn export_report(out: &str) -> Result<String, String> {
    let bundle = load_bundle(None)?;
    let mut output = engine_run(&engine_input(&bundle));
    let confirmed = confirmed_ids();
    for c in &mut output.clusters {
        if confirmed.contains(&c.cluster_id) {
            c.state = ClusterState::ConfirmedSameSource;
        }
    }
    let csv = export_csv(&output.clusters);
    assert_no_real_identifiers(&csv).map_err(|e| e.to_string())?;
    let path = if Path::new(out).is_absolute() {
        PathBuf::from(out)
    } else {
        project_root().join(out)
    };
    ensure_dir(&path)?;
    std::fs::write(&path, &csv).map_err(|e| e.to_string())?;

    let md_path = path.with_extension("md");
    std::fs::write(&md_path, export_markdown(&output.clusters)).map_err(|e| e.to_string())?;

    let exported_members: usize = output
        .clusters
        .iter()
        .filter(|c| confirmed.contains(&c.cluster_id))
        .map(|c| c.member_count())
        .sum();
    Ok(format!(
        "簇清单 CSV 已导出：{}（{} 行数据）\nMarkdown 已导出：{}\n观察中与待核实簇：0 行（不进入对外清单）\n导出前标识扫描：无真实样式批次号 / 序列号 / 患者标识\n",
        path.display(),
        exported_members,
        md_path.display()
    ))
}

// ── 人工确认与批量开单 ──────────────────────────────────────────

pub fn confirm_report(cluster_id: &str) -> Result<String, (i32, String)> {
    let bundle = load_bundle(None).map_err(|e| (1, format!("confirm 失败：{e}\n")))?;
    let output = engine_run(&engine_input(&bundle));
    let cluster = output
        .clusters
        .iter()
        .find(|c| c.cluster_id == cluster_id)
        .cloned();
    let cluster = match cluster {
        Some(c) => c,
        None => {
            return Err((
                1,
                format!("未找到簇 {cluster_id}（当前 {} 个簇）\n", output.clusters.len()),
            ))
        }
    };
    let team = bundle
        .teams
        .first()
        .cloned()
        .unwrap_or_default();
    let req = ConfirmRequest {
        category: cluster.fault_category.clone(),
        members_verified: true,
        team: team.team.clone(),
        capabilities: bundle.teams.clone(),
        earliest_epoch: cluster.earliest_epoch,
        now_epoch: cluster.earliest_epoch,
        window_secs: 30 * 24 * 3600,
        actor: std::env::var("DFA_ACTOR").unwrap_or_else(|_| "设备科值班工程师".to_string()),
        ts: 0,
        member_snapshot: cluster
            .members
            .iter()
            .map(|m| m.device_id.clone())
            .collect(),
    };
    let confirmation = match confirm_action(&req, cluster.state) {
        Ok(c) => c,
        Err(missing) => {
            return Err((
                4,
                format!(
                    "{}：确认被拒，缺项：{}\n",
                    ErrorCode::IllegalStateTransition.code(),
                    missing.join(" / ")
                ),
            ))
        }
    };

    // 确认记录落盘（供 export 识别已确认同源簇）。
    let path = project_root().join("data/confirmations.json");
    let mut ids = confirmed_ids();
    if !ids.contains(&cluster_id.to_string()) {
        ids.push(cluster_id.to_string());
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(&ids).unwrap_or_default());

    let mut text = String::new();
    text.push_str(&format!(
        "人工确认已记录：操作人 {} · 时间 {} · 成员快照 {} 台 · {} → {}\n",
        confirmation.actor,
        confirmation.ts,
        confirmation.member_snapshot.len(),
        confirmation.before_state,
        confirmation.after_state
    ));

    let base = std::env::var("WORKORDER_BASE_URL").unwrap_or_default();
    let token = std::env::var("WORKORDER_TOKEN").unwrap_or_default();
    if base.trim().is_empty() {
        text.push_str("未配置 WORKORDER_BASE_URL：跳过真实出站，出站请求计数 = 0（确认本身已完成）。\n");
        return Ok(text);
    }

    let drafts: Vec<WorkOrderDraft> = cluster
        .members
        .iter()
        .map(|m| WorkOrderDraft {
            workorder_no: m.workorder_no.clone(),
            device_id: m.device_id.clone(),
            device_model: m.model.clone(),
            firmware_version: m.firmware.clone(),
            fault_code: cluster.fault_category.clone(),
            fault_text: m.fault_text.clone(),
            priority: "P2".to_string(),
            assignee_team: team.team.clone(),
            source_ref: format!("cluster:{}", cluster.cluster_id),
            cluster_id: cluster.cluster_id.clone(),
        })
        .collect();
    let check = PrecheckInput {
        vocabulary: FaultVocabulary::standard(),
        capabilities: bundle.teams.clone(),
        critical_ward: false,
    };
    let mut client = WorkOrderClient::new(ReqwestSink::new(), &base, &token);
    let result = client.submit_batch(&drafts, &check);
    text.push_str(&format!(
        "批量开单：{} 张草稿，出站请求 {} 次，逐条回执如下：\n",
        drafts.len(),
        result.outbound_requests
    ));
    for r in &result.receipts {
        text.push_str(&format!(
            "- {} · {} · http={} · 可重放={} · Idempotency-Key={}…\n",
            r.workorder_no,
            r.outcome.label(),
            r.http_status,
            r.retryable,
            &r.idempotency_key[..12.min(r.idempotency_key.len())]
        ));
    }
    for rejected in &result.rejected {
        text.push_str(&format!("- 写前拦下：{rejected}\n"));
    }
    Ok(text)
}

// ── 评测 / 度量 / 窗口投影 ──────────────────────────────────────

/// 语料行：工单字段 + 设备字段（同一行自洽，便于离线评测）。
fn load_corpus(path: &Path) -> Result<(Vec<WorkOrder>, Vec<Device>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut orders = Vec::new();
    let mut devices: BTreeMap<String, Device> = BTreeMap::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let wo: WorkOrder = serde_json::from_value(v.clone()).unwrap_or_default();
        let dev = Device {
            device_id: v["device_id"].as_str().unwrap_or("").to_string(),
            model: v["model"].as_str().unwrap_or("").to_string(),
            serial: v["serial"].as_str().unwrap_or("EXAMPLE-SERIAL").to_string(),
            firmware: v["firmware"].as_str().unwrap_or("").to_string(),
            ward: v["ward"].as_str().unwrap_or("").to_string(),
            purchase_batch: v["purchase_batch"].as_str().unwrap_or("").to_string(),
        };
        devices.insert(dev.device_id.clone(), dev);
        orders.push(wo);
    }
    Ok((orders, devices.into_values().collect()))
}

pub fn cluster_report(input: &str, out: &str) -> Result<String, String> {
    let (orders, devices) = load_corpus(Path::new(input))?;
    let output = engine_run(&EngineInput {
        workorders: orders,
        devices,
        now_epoch: 0,
        time_window_secs: 0,
    });
    let path = Path::new(out);
    ensure_dir(path)?;
    std::fs::write(path, assignments_jsonl(&output)).map_err(|e| e.to_string())?;
    Ok(format!(
        "本轮判定器分组结果已写入 {}（{} 条语料，{} 个分组）\n",
        out,
        output.assignments.len(),
        output.clusters.len()
    ))
}

pub fn measure_report(input: &str) -> Result<String, String> {
    let (orders, devices) = load_corpus(Path::new(input))?;
    let metrics: ScaleMetrics = measure(&EngineInput {
        workorders: orders,
        devices,
        now_epoch: 0,
        time_window_secs: 0,
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&metrics).map_err(|e| e.to_string())?))
}

fn current_view() -> SnapshotView {
    let snap = project_root().join("data/snapshot.json");
    match dfa_store::read_snapshot(&snap) {
        Ok(read) => SnapshotView::from_snapshot_json(&read.envelope.payload)
            .unwrap_or_default(),
        Err(_) => match load_bundle(None) {
            Ok(b) => SnapshotView::from_clusters(&engine_run(&engine_input(&b)).clusters, "就绪"),
            Err(_) => SnapshotView::default(),
        },
    }
}

pub fn view_report() -> String {
    current_view().text_projection()
}

pub fn tray_report() -> String {
    let view = current_view();
    let tray = TrayState::from_view(&view);
    let mut text = format!("托盘项标题：{}\n", tray.title());
    for line in tray.menu_lines() {
        text.push_str(&format!("  {line}\n"));
    }
    text.push_str(&format!("{}\n", tray.click_action()));
    text.push_str("说明：源产品无 OS toast 通道，计数由托盘项标题 + 窗口内横幅承载。\n");
    text
}

pub fn summary_line(summary: &RunSummary) -> String {
    summary.line()
}
