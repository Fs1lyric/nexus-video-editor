use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSettings {
    pub output_path: PathBuf,
    pub format: VideoFormat,
    pub codec: VideoCodec,
    pub bitrate: u32, // in kbps
    pub fps: f32,
    pub width: u32,
    pub height: u32,
    pub audio_bitrate: u32, // in kbps
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum VideoFormat {
    Mp4,
    Webm,
    Mov,
    Mkv,
    Avi,
    Flv,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum VideoCodec {
    H264,
    H265,
    Vp8,
    Vp9,
    ProRes,
    Dnxhd,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("output.mp4"),
            format: VideoFormat::Mp4,
            codec: VideoCodec::H264,
            bitrate: 5000,
            fps: 30.0,
            width: 1920,
            height: 1080,
            audio_bitrate: 128,
        }
    }
}

pub struct Exporter;

impl Exporter {
    pub fn export(settings: &ExportSettings) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Implement actual export using FFmpeg
        log::info!(
            "Exporting to {} with codec {:?}",
            settings.output_path.display(),
            settings.codec
        );
        Ok(())
    }
}
