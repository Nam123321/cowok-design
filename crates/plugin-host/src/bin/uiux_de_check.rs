// @story 08.5
// @implements AC-01 (CLI uiux-os de-check exposing all 7 built-in DE tools)
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use plugin_host::de_tools::{run_builtin_de_suite, BuiltInDeTool, DeSuiteInput};

fn print_usage() {
    eprintln!(
        "Usage: uiux-de-check --workspace <dir> --target <rel-path> [--tools scan-ui,intake,tokens,nfa-packs,archify,get_artifact,notes-check] [--json]"
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut workspace = PathBuf::from(".");
    let mut target: Option<String> = None;
    let mut tools: Vec<BuiltInDeTool> = Vec::new();
    let mut json_output = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--workspace" | "-w" => {
                i += 1;
                if let Some(val) = args.get(i) {
                    workspace = PathBuf::from(val);
                } else {
                    print_usage();
                    return ExitCode::from(2);
                }
            }
            "--target" | "-t" => {
                i += 1;
                if let Some(val) = args.get(i) {
                    target = Some(val.clone());
                } else {
                    print_usage();
                    return ExitCode::from(2);
                }
            }
            "--tools" => {
                i += 1;
                if let Some(csv) = args.get(i) {
                    for part in csv.split(',').filter(|s| !s.trim().is_empty()) {
                        match BuiltInDeTool::parse_name(part) {
                            Some(t) => tools.push(t),
                            None => {
                                eprintln!("Unknown DE tool: {part}");
                                return ExitCode::from(2);
                            }
                        }
                    }
                } else {
                    print_usage();
                    return ExitCode::from(2);
                }
            }
            "--json" => json_output = true,
            "--help" | "-h" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => {
                if target.is_none() && !other.starts_with('-') {
                    target = Some(other.to_string());
                } else {
                    eprintln!("Unknown argument: {other}");
                    print_usage();
                    return ExitCode::from(2);
                }
            }
        }
        i += 1;
    }

    let Some(target_path) = target else {
        print_usage();
        return ExitCode::from(2);
    };

    let input = DeSuiteInput {
        workspace_root: &workspace,
        target_artifact_path: &target_path,
        content_override: None,
        tools: &tools,
        token_pairs: &[],
    };

    match run_builtin_de_suite(&input) {
        Ok(report) => {
            if json_output {
                match serde_json::to_string_pretty(&report) {
                    Ok(s) => println!("{s}"),
                    Err(e) => {
                        eprintln!("JSON serialization error: {e}");
                        return ExitCode::from(2);
                    }
                }
            } else {
                println!(
                    "uiux-os de-check | target={} | passed={} | duration={}ms | cost=$0.00",
                    report.target_artifact_path, report.passed, report.total_duration_ms
                );
                for res in &report.results {
                    println!(
                        "  - {:<14} passed={:<5} findings={:<3} ({}us)",
                        res.tool.as_str(),
                        res.passed,
                        res.findings.len(),
                        res.duration_micros
                    );
                }
            }
            if report.passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(err) => {
            eprintln!("DE_CHECK_ERROR: {err}");
            ExitCode::from(2)
        }
    }
}
