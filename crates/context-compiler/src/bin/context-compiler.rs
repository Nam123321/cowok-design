// @story 02.1
// @implements AC-02
use context_compiler::inspect_prompt_from_args;
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--inspect-prompt") {
        let root = env::var("UXUI_OS_WORKSPACE_ROOT")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        match inspect_prompt_from_args(&args, &root) {
            Ok(json) => {
                println!("{json}");
                return;
            }
            Err(err) => {
                eprintln!("{err}");
                process::exit(1);
            }
        }
    }
    eprintln!("Usage: context-compiler --inspect-prompt [--portal <PortalId>] [--horizon <short|mid|long>] [--profile <A|B|C>] [--intent <text>]");
    process::exit(2);
}
