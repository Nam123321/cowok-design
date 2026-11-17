// @story 08.2
// @implements AC-04, AC-07
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoFrameCheckpoint {
    pub frame_id: String,
    pub timestamp_s: f64,
    pub action_label: String,
    pub ui_intent: String,
    pub coordinate_bounds: Option<[f64; 4]>, // [x, y, w, h]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoStoryboardReceipt {
    pub video_source: String,
    pub duration_s: f64,
    pub total_checkpoints: usize,
    pub checkpoints: Vec<VideoFrameCheckpoint>,
    pub generated_at: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VideoIntakeError {
    #[error("Storyboard log contains zero valid checkpoints")]
    EmptyStoryboard,
    #[error("Invalid storyboard line format at line {line}: {detail}")]
    InvalidLineFormat { line: usize, detail: String },
    #[error("Invalid timestamp at line {line}: {detail}")]
    InvalidTimestamp { line: usize, detail: String },
    #[error("Non-monotonic timestamp at line {line}: {current_ms}ms < previous {prev_ms}ms")]
    NonMonotonicTimestamp {
        line: usize,
        prev_ms: u64,
        current_ms: u64,
    },
    #[error("Empty action label or UI intent at line {line}")]
    EmptyCheckpointField { line: usize },
}

pub struct VideoIntakeProcessor;

impl VideoIntakeProcessor {
    /// Strict fail-closed storyboard parser enforcing `[frame: mm:ss.xx] -> Action: Intent` grammar (AC-07).
    pub fn parse_storyboard_text_strict(
        video_source: &str,
        raw_log: &str,
    ) -> Result<VideoStoryboardReceipt, VideoIntakeError> {
        if video_source.trim().is_empty() {
            return Err(VideoIntakeError::InvalidLineFormat {
                line: 0,
                detail: "video_source must not be empty".to_string(),
            });
        }
        let mut checkpoints = Vec::new();
        let mut duration_max = 0.0f64;
        let mut prev_ms: Option<u64> = None;

        for (idx, line) in raw_log.lines().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let cp = Self::parse_line_checked(line_num, trimmed)?;
            let curr_ms = (cp.timestamp_s * 1000.0).round() as u64;
            if let Some(prev) = prev_ms {
                if curr_ms < prev {
                    return Err(VideoIntakeError::NonMonotonicTimestamp {
                        line: line_num,
                        prev_ms: prev,
                        current_ms: curr_ms,
                    });
                }
            }
            prev_ms = Some(curr_ms);
            if cp.timestamp_s > duration_max {
                duration_max = cp.timestamp_s;
            }
            checkpoints.push(cp);
        }

        if checkpoints.is_empty() {
            return Err(VideoIntakeError::EmptyStoryboard);
        }

        Ok(VideoStoryboardReceipt {
            video_source: video_source.to_string(),
            duration_s: duration_max,
            total_checkpoints: checkpoints.len(),
            checkpoints,
            generated_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Strictly parses timestamped storyboard checkpoints (delegates fail-closed to `parse_storyboard_text_strict`).
    pub fn parse_storyboard_text(
        video_source: &str,
        raw_log: &str,
    ) -> Result<VideoStoryboardReceipt, VideoIntakeError> {
        Self::parse_storyboard_text_strict(video_source, raw_log)
    }

    fn parse_line_checked(
        line_num: usize,
        trimmed: &str,
    ) -> Result<VideoFrameCheckpoint, VideoIntakeError> {
        if !trimmed.starts_with('[') {
            return Err(VideoIntakeError::InvalidLineFormat {
                line: line_num,
                detail: "Missing opening '[' frame header".to_string(),
            });
        }
        let close_bracket = trimmed.find(']').ok_or_else(|| {
            VideoIntakeError::InvalidLineFormat {
                line: line_num,
                detail: "Missing closing ']' in frame header".to_string(),
            }
        })?;
        let header = &trimmed[1..close_bracket];
        let rest = trimmed[close_bracket + 1..].trim();
        let timestamp_s = Self::parse_timestamp_checked(line_num, header)?;

        let after_arrow = rest
            .strip_prefix("->")
            .ok_or_else(|| VideoIntakeError::InvalidLineFormat {
                line: line_num,
                detail: "Expected '-> Action Label: UI Intent' after frame header".to_string(),
            })?
            .trim();
        let (raw_label, raw_intent) =
            after_arrow
                .split_once(':')
                .ok_or_else(|| VideoIntakeError::InvalidLineFormat {
                    line: line_num,
                    detail: "Missing ':' separating Action Label and UI Intent".to_string(),
                })?;

        let action_label = raw_label.trim();
        let ui_intent = raw_intent.trim();
        if action_label.is_empty() || ui_intent.is_empty() {
            return Err(VideoIntakeError::EmptyCheckpointField { line: line_num });
        }

        Ok(VideoFrameCheckpoint {
            frame_id: format!("frame_{:06.2}", timestamp_s),
            timestamp_s,
            action_label: action_label.to_string(),
            ui_intent: ui_intent.to_string(),
            coordinate_bounds: None,
        })
    }

    /// Enforces strictly `[frame: mm:ss.xx]` grammar (`00..=59` mins/secs, exactly 2 fractional digits).
    pub fn parse_timestamp_checked(line_num: usize, header: &str) -> Result<f64, VideoIntakeError> {
        let trimmed_hdr = header.trim();
        let part = trimmed_hdr
            .strip_prefix("frame:")
            .ok_or_else(|| VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Missing required 'frame:' header prefix in '[{trimmed_hdr}]'"),
            })?
            .trim();
        let segments: Vec<&str> = part.split(':').collect();
        if segments.len() != 2 {
            return Err(VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Expected strict mm:ss.xx format, got '{part}'"),
            });
        }

        let mins = Self::parse_uint_digits(line_num, segments[0], 2, 2, "minutes")?;
        if mins > 59 {
            return Err(VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Minutes '{mins}' exceeds 59"),
            });
        }

        let secs = Self::parse_seconds_checked(line_num, segments[1])?;
        let total = (mins as f64) * 60.0 + secs;
        if !total.is_finite() || total < 0.0 {
            return Err(VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: "Timestamp is negative or non-finite".to_string(),
            });
        }
        Ok(total)
    }

    fn parse_uint_digits(
        line_num: usize,
        raw: &str,
        min_len: usize,
        max_len: usize,
        label: &str,
    ) -> Result<u64, VideoIntakeError> {
        if raw.len() < min_len || raw.len() > max_len || !raw.bytes().all(|b| b.is_ascii_digit()) {
            return Err(VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Invalid {label} segment '{raw}'"),
            });
        }
        raw.parse::<u64>()
            .map_err(|_| VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Invalid numeric {label} '{raw}'"),
            })
    }

    fn parse_seconds_checked(line_num: usize, raw: &str) -> Result<f64, VideoIntakeError> {
        let (whole_str, frac) = raw.split_once('.').ok_or_else(|| {
            VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Missing required fractional seconds '.xx' in '{raw}'"),
            }
        })?;
        let whole = Self::parse_uint_digits(line_num, whole_str, 2, 2, "seconds")?;
        if whole > 59 {
            return Err(VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Seconds '{whole}' exceeds 59"),
            });
        }
        if frac.len() != 2 || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return Err(VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Fractional seconds must be exactly '.xx' (2 digits), got '.{frac}'"),
            });
        }
        raw.parse::<f64>()
            .map_err(|_| VideoIntakeError::InvalidTimestamp {
                line: line_num,
                detail: format!("Invalid seconds value '{raw}'"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_storyboard_text() {
        let text = r#"
[frame: 00:01.25] -> CTA Click: User clicks primary checkout button
[frame: 00:02.80] -> Modal Fly-in: Shipping address drawer slides from right
[frame: 00:04.10] -> Form Fill: Autofill address with smooth transition
"#;
        let receipt = VideoIntakeProcessor::parse_storyboard_text("demo.mp4", text).unwrap();
        assert_eq!(receipt.total_checkpoints, 3);
        assert_eq!(receipt.checkpoints[0].frame_id, "frame_001.25");
        assert_eq!(receipt.checkpoints[0].action_label, "CTA Click");
        assert_eq!(
            receipt.checkpoints[0].ui_intent,
            "User clicks primary checkout button"
        );
        assert_eq!(receipt.duration_s, 4.10);
    }
}
