use serde::{Deserialize, Serialize};

pub const AVAILABLE_EFFECTS: &[&str] = &[
    "Blur",
    "Sharpen",
    "Color Correction",
    "Saturation",
    "Brightness/Contrast",
    "Hue Shift",
    "Fade In",
    "Fade Out",
    "Speed Ramp",
    "Slow Motion",
    "Reverse",
    "Crop",
    "Scale",
    "Rotate",
    "Flip",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    pub name: String,
    pub parameters: Vec<EffectParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectParameter {
    pub name: String,
    pub value: ParameterValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
}

impl Effect {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            parameters: Vec::new(),
        }
    }

    pub fn add_parameter(&mut self, param: EffectParameter) {
        self.parameters.push(param);
    }
}
