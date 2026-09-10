//! nexus — a small video editor built on top of the `ffmpeg` / `ffprobe`
//! command-line tools.
//!
//! Scope of v1: a single video track. You import clips, set an in/out point on
//! each, order them, and export the concatenation to an mp4. No effects, no
//! transitions, no audio mixing — just the parts that actually work well when
//! you shell out to ffmpeg.
//!
//! The [`render`] module does the ffmpeg orchestration; [`project`] is the data
//! model (serialisable to JSON); [`media`] wraps `ffprobe`.

pub mod media;
pub mod project;
pub mod render;

pub use media::{probe, MediaInfo};
pub use project::{Clip, Project, Timeline};
pub use render::{concat, export_project, trim, ProgressFn};
