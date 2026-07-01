use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicConfig {
    pub local_ip: String,
    pub local_port: u16,
    pub remote_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub preview_coe: u32,
    pub ref_channal: u32,
    pub save_channal: u32,
    pub wave_len: u32,
    pub first_pos: u32,
    pub first_len: u32,
    pub second_pos: u32,
    pub second_len: u32,
    pub sum_value: u32,
    pub max_value: u32,
    pub pin_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub basic: BasicConfig,
    pub capture: CaptureConfig,
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
