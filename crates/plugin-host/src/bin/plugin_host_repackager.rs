// @story 08.2
// @implements AC-01, AC-02, AC-05, AC-06, AC-07
use plugin_host::{SkillRepackager, VideoIntakeProcessor};
use serde::Deserialize;
use std::io::Read;

#[derive(Deserialize)]
struct VideoIntakeRequest {
    video_source: String,
    raw_log: String,
}

#[derive(Deserialize)]
struct RepackageRequest {
    source_dir: String,
    dest_dir: String,
    skill_name: String,
    description: String,
    capabilities: Vec<String>,
    upstream_url: Option<String>,
}

#[derive(Deserialize)]
struct VerifyBundleRequest {
    bundle_dir: String,
}

#[derive(Deserialize)]
struct CommitBundleRequest {
    tmp_dir: String,
    dst_dir: String,
    staging_token: String,
}

const MAX_IPC_JSON_BYTES: usize = 1_048_576;

fn run() -> Result<String, String> {
    let mode = std::env::args()
        .nth(1)
        .ok_or_else(|| "Missing mode argument: repackage | stage-bundle | verify-bundle | commit-bundle | video-intake".to_string())?;
    let mut input = String::new();
    std::io::stdin()
        .take((MAX_IPC_JSON_BYTES + 1) as u64)
        .read_to_string(&mut input)
        .map_err(|e| format!("Failed reading stdin JSON: {e}"))?;
    if input.len() > MAX_IPC_JSON_BYTES {
        return Err(format!(
            "IPC payload exceeds {MAX_IPC_JSON_BYTES} byte limit"
        ));
    }

    match mode.as_str() {
        "repackage" => {
            let req: RepackageRequest =
                serde_json::from_str(&input).map_err(|e| format!("Invalid repackage JSON: {e}"))?;
            let receipt = SkillRepackager::repackage_directory(
                &req.source_dir,
                &req.dest_dir,
                &req.skill_name,
                &req.description,
                req.capabilities,
                req.upstream_url.as_deref(),
            )?;
            serde_json::to_string(&receipt).map_err(|e| e.to_string())
        }
        "verify-bundle" => {
            let req: VerifyBundleRequest = serde_json::from_str(&input)
                .map_err(|e| format!("Invalid verify-bundle JSON: {e}"))?;
            let lock = SkillRepackager::verify_repackaged_bundle(&req.bundle_dir)?;
            serde_json::to_string(&lock).map_err(|e| e.to_string())
        }
        "stage-bundle" => {
            let req: RepackageRequest = serde_json::from_str(&input)
                .map_err(|e| format!("Invalid stage-bundle JSON: {e}"))?;
            let staged = SkillRepackager::stage_directory(
                &req.source_dir,
                &req.dest_dir,
                &req.skill_name,
                &req.description,
                req.capabilities,
                req.upstream_url.as_deref(),
            )?;
            serde_json::to_string(&staged).map_err(|e| e.to_string())
        }
        "commit-bundle" => {
            let req: CommitBundleRequest = serde_json::from_str(&input)
                .map_err(|e| format!("Invalid commit-bundle JSON: {e}"))?;
            SkillRepackager::commit_staged_bundle_atomic(
                &req.tmp_dir,
                &req.dst_dir,
                &req.staging_token,
            )?;
            Ok("{}".to_string())
        }
        "video-intake" => {
            let req: VideoIntakeRequest = serde_json::from_str(&input)
                .map_err(|e| format!("Invalid video-intake JSON: {e}"))?;
            let receipt =
                VideoIntakeProcessor::parse_storyboard_text_strict(&req.video_source, &req.raw_log)
                    .map_err(|e| e.to_string())?;
            serde_json::to_string(&receipt).map_err(|e| e.to_string())
        }
        other => Err(format!("Unknown mode '{other}'")),
    }
}

fn main() {
    match run() {
        Ok(json_out) => println!("{json_out}"),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
