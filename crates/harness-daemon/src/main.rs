// @story 04.1
// @implements AC-01, AC-04, AC-06
//! Standalone binary entrypoint for the `cowok-design` loopback Axum harness daemon.

fn main() {
    if let Err(err) = harness_daemon::run_daemon() {
        eprintln!("[cowok-design-harness-daemon] fatal startup error: {err}");
        std::process::exit(1);
    }
}
