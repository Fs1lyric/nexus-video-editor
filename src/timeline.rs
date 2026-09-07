use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub name: String,
    pub file_path: String,
    pub duration: f32,
    pub start_time: f32,
    pub end_time: f32,
    pub clip_type: ClipType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ClipType {
    Video,
    Audio,
    Image,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub clips: Vec<Clip>,
    pub duration: f32,
    pub fps: f32,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            clips: Vec::new(),
            duration: 0.0,
            fps: 30.0,
        }
    }

    pub fn add_clip(&mut self, clip: Clip) {
        self.clips.push(clip.clone());
        self.update_duration();
    }

    pub fn remove_clip(&mut self, index: usize) {
        if index < self.clips.len() {
            self.clips.remove(index);
            self.update_duration();
        }
    }

    pub fn update_duration(&mut self) {
        self.duration = self
            .clips
            .iter()
            .map(|c| c.end_time)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
    }

    pub fn move_clip(&mut self, index: usize, new_start: f32) {
        if index < self.clips.len() {
            let duration = self.clips[index].duration;
            self.clips[index].start_time = new_start;
            self.clips[index].end_time = new_start + duration;
            self.update_duration();
        }
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
