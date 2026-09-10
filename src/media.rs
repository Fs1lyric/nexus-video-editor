//! Thin wrapper around `ffprobe`.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// The `ffmpeg` binary to invoke. Override with `NEXUS_FFMPEG`.
pub fn ffmpeg_bin() -> String {
    std::env::var("NEXUS_FFMPEG").unwrap_or_else(|_| "ffmpeg".into())
}

/// The `ffprobe` binary to invoke. Override with `NEXUS_FFPROBE`.
pub fn ffprobe_bin() -> String {
    std::env::var("NEXUS_FFPROBE").unwrap_or_else(|_| "ffprobe".into())
}

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_audio: bool,
}

/// Probe a media file. Returns an error if `ffprobe` is missing or the file is
/// not readable media.
pub fn probe(path: &Path) -> Result<MediaInfo> {
    let output = Command::new(ffprobe_bin())
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .context("could not run ffprobe — is ffmpeg installed and on PATH?")?;

    if !output.status.success() {
        bail!("ffprobe could not read {}", path.display());
    }

    let v: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("ffprobe returned output that was not valid JSON")?;

    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let video = streams.iter().find(|s| s["codec_type"] == "video");
    let has_audio = streams.iter().any(|s| s["codec_type"] == "audio");

    let duration = v["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| {
            video
                .and_then(|s| s["duration"].as_str())
                .and_then(|s| s.parse::<f64>().ok())
        })
        .unwrap_or(0.0);

    let (width, height, fps) = match video {
        Some(s) => {
            let w = s["width"].as_u64().unwrap_or(0) as u32;
            let h = s["height"].as_u64().unwrap_or(0) as u32;
            let fps = parse_rational(s["avg_frame_rate"].as_str().unwrap_or("0/0"))
                .filter(|f| *f > 0.0)
                .or_else(|| parse_rational(s["r_frame_rate"].as_str().unwrap_or("0/0")))
                .unwrap_or(30.0);
            (w, h, fps)
        }
        None => (0, 0, 30.0),
    };

    Ok(MediaInfo {
        duration,
        width,
        height,
        fps,
        has_audio,
    })
}

fn parse_rational(s: &str) -> Option<f64> {
    let (n, d) = s.split_once('/')?;
    let n: f64 = n.trim().parse().ok()?;
    let d: f64 = d.trim().parse().ok()?;
    if d == 0.0 {
        None
    } else {
        Some(n / d)
    }
}
