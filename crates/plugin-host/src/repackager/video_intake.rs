use serde::{Deserialize, Serialize};

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

pub struct VideoIntakeProcessor;

impl VideoIntakeProcessor {
    /// Parses timestamped storyboard logs / vision markers into structured frame checkpoints
    pub fn parse_storyboard_text(video_source: &str, raw_log: &str) -> VideoStoryboardReceipt {
        let mut checkpoints = Vec::new();
        let mut duration_max = 0.0;

        for line in raw_log.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || !trimmed.starts_with('[') {
                continue;
            }

            // Expected format: [frame: 00:01.50] -> Action Label: UI Intent Description
            if let Some(close_bracket) = trimmed.find(']') {
                let header = &trimmed[1..close_bracket];
                let rest = trimmed[close_bracket + 1..].trim();

                let timestamp_s = Self::parse_timestamp(header);
                if timestamp_s > duration_max {
                    duration_max = timestamp_s;
                }

                let (action_label, ui_intent) = if let Some(arrow_idx) = rest.find("->") {
                    let after_arrow = rest[arrow_idx + 2..].trim();
                    if let Some(colon_idx) = after_arrow.find(':') {
                        (
                            after_arrow[..colon_idx].trim().to_string(),
                            after_arrow[colon_idx + 1..].trim().to_string(),
                        )
                    } else {
                        (after_arrow.to_string(), "User interaction".to_string())
                    }
                } else {
                    ("Observed Frame".to_string(), rest.to_string())
                };

                let frame_id = format!("frame_{:06.2}", timestamp_s);

                checkpoints.push(VideoFrameCheckpoint {
                    frame_id,
                    timestamp_s,
                    action_label,
                    ui_intent,
                    coordinate_bounds: None,
                });
            }
        }

        let total = checkpoints.len();
        VideoStoryboardReceipt {
            video_source: video_source.to_string(),
            duration_s: duration_max,
            total_checkpoints: total,
            checkpoints,
            generated_at: chrono::Utc::now().timestamp(),
        }
    }

    fn parse_timestamp(header: &str) -> f64 {
        // e.g. "frame: 00:01.50" or "01:23.45"
        let part = if let Some(idx) = header.find(':') {
            &header[idx + 1..]
        } else {
            header
        }
        .trim();

        let segments: Vec<&str> = part.split(':').collect();
        match segments.len() {
            1 => segments[0].parse::<f64>().unwrap_or(0.0),
            2 => {
                let mins: f64 = segments[0].parse().unwrap_or(0.0);
                let secs: f64 = segments[1].parse().unwrap_or(0.0);
                mins * 60.0 + secs
            }
            3 => {
                let hrs: f64 = segments[0].parse().unwrap_or(0.0);
                let mins: f64 = segments[1].parse().unwrap_or(0.0);
                let secs: f64 = segments[2].parse().unwrap_or(0.0);
                hrs * 3600.0 + mins * 60.0 + secs
            }
            _ => 0.0,
        }
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
        let receipt = VideoIntakeProcessor::parse_storyboard_text("demo.mp4", text);
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
