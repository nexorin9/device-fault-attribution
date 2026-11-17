//! dfa-cli 二进制入口：运维排障通道，不与窗口并列做第二套业务入口。

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = dfa_cli::run(args);
    std::process::exit(code);
}
