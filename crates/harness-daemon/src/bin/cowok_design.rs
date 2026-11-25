// @story 12.1
// @implements AC-01, AC-02, AC-03, AC-04
//! Single-binary CLI entrypoint for `cowok-design` (Channel 1 Distribution).

fn main() {
    let code = harness_daemon::cli::run_cli_from_env();
    if code != 0 {
        std::process::exit(code);
    }
}
