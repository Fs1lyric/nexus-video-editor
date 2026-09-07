use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub fps: f32,
    pub width: u32,
    pub height: u32,
    pub duration: f32,
}

impl Project {
    pub fn new() -> Self {
        Self {
            name: "Untitled Project".to_string(),
            path: PathBuf::new(),
            fps: 30.0,
            width: 1920,
            height: 1080,
            duration: 0.0,
        }
    }

    pub fn set_resolution(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn set_fps(&mut self, fps: f32) {
        self.fps = fps;
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}
