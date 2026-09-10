//! The editable document: a project with a single ordered list of clips.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    #[serde(default)]
    pub timeline: Timeline,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Untitled".into(),
            width: 1920,
            height: 1080,
            fps: 30.0,
            timeline: Timeline::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub clips: Vec<Clip>,
}

/// One clip on the timeline: a slice `[src_in, src_out)` of a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub source: PathBuf,
    pub src_in: f64,
    pub src_out: f64,
}

impl Clip {
    pub fn duration(&self) -> f64 {
        (self.src_out - self.src_in).max(0.0)
    }
}

impl Project {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading project {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing project {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)
            .with_context(|| format!("writing project {}", path.display()))?;
        Ok(())
    }

    /// Total running time of the timeline, in seconds.
    pub fn duration(&self) -> f64 {
        self.timeline.clips.iter().map(Clip::duration).sum()
    }

    pub fn next_id(&self) -> u64 {
        self.timeline.clips.iter().map(|c| c.id).max().unwrap_or(0) + 1
    }

    pub fn push_clip(&mut self, source: PathBuf, src_in: f64, src_out: f64) {
        let id = self.next_id();
        self.timeline.clips.push(Clip {
            id,
            source,
            src_in,
            src_out,
        });
    }

    /// Which clip is playing at timeline time `t`, and the corresponding
    /// position inside that clip's source file.
    pub fn locate(&self, t: f64) -> Option<(usize, f64)> {
        let clips = &self.timeline.clips;
        let mut acc = 0.0;
        for (i, c) in clips.iter().enumerate() {
            let d = c.duration();
            if t < acc + d || i + 1 == clips.len() {
                let into_src = c.src_in + (t - acc);
                return Some((i, into_src.clamp(c.src_in, c.src_out)));
            }
            acc += d;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(id: u64, a: f64, b: f64) -> Clip {
        Clip {
            id,
            source: PathBuf::from("/x.mp4"),
            src_in: a,
            src_out: b,
        }
    }

    #[test]
    fn duration_is_never_negative() {
        assert_eq!(clip(1, 5.0, 2.0).duration(), 0.0);
        assert_eq!(clip(1, 2.0, 5.0).duration(), 3.0);
    }

    #[test]
    fn locate_walks_the_timeline() {
        let mut p = Project::default();
        p.timeline.clips = vec![clip(1, 0.0, 2.0), clip(2, 10.0, 13.0)];
        // 0.5s in -> first clip, 0.5s into source
        assert_eq!(p.locate(0.5), Some((0, 0.5)));
        // 2.5s in -> second clip (starts at t=2), 0.5s past its src_in of 10
        assert_eq!(p.locate(2.5), Some((1, 10.5)));
        // past the end clamps to the last clip's out point
        assert_eq!(p.locate(99.0), Some((1, 13.0)));
    }

    #[test]
    fn json_round_trips() {
        let mut p = Project {
            name: "Demo".into(),
            width: 1280,
            height: 720,
            fps: 30.0,
            ..Default::default()
        };
        p.push_clip(PathBuf::from("/a.mp4"), 1.0, 4.0);
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "Demo");
        assert_eq!(back.timeline.clips.len(), 1);
        assert_eq!(back.duration(), 3.0);
    }
}
