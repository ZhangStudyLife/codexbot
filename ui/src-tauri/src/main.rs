#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn cli_command_requested() -> bool {
    std::env::args().nth(1).is_some_and(|argument| {
        matches!(
            argument.as_str(),
            "setup"
                | "pair"
                | "doctor"
                | "start"
                | "stop"
                | "daemon"
                | "hook"
                | "--help"
                | "-h"
                | "--version"
                | "-V"
        )
    })
}

fn run_cli() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("错误：无法启动 CodexBot 运行时：{error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(codexbot::cli::run()) {
        Ok(code) => ExitCode::from(code.clamp(0, u8::MAX as i32) as u8),
        Err(error) => {
            let detail = codexbot::security::redact_secrets(&error.to_string());
            eprintln!("错误：{}", detail.replace(['\r', '\n'], " "));
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    if cli_command_requested() {
        return run_cli();
    }

    codexbot_desktop_lib::run();
    ExitCode::SUCCESS
}
