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
pub struct GpsConfig {
    /// GPS 数据文件所在目录（设备端路径）
    #[serde(default = "default_gps_dir")]
    pub dir: String,
}

fn default_gps_dir() -> String {
    "/run/media/mmcblk1p1/".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub basic: BasicConfig,
    pub capture: CaptureConfig,
    /// GPS 配置；老配置文件缺省时使用默认值
    #[serde(default)]
    pub gps: GpsConfig,
}

impl Default for BasicConfig {
    fn default() -> Self {
        Self {
            local_ip: "192.168.93.44".to_string(),
            local_port: 12345,
            remote_port: 6666,
        }
    }
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            preview_coe: 1000,
            ref_channal: 1,
            save_channal: 0xff,
            wave_len: 6000,
            first_pos: 100,
            first_len: 200,
            second_pos: 400,
            second_len: 400,
            sum_value: 0,
            max_value: 0,
            pin_threshold: 1000,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            basic: BasicConfig::default(),
            capture: CaptureConfig::default(),
            gps: GpsConfig::default(),
        }
    }
}

impl Default for GpsConfig {
    fn default() -> Self {
        Self {
            dir: default_gps_dir(),
        }
    }
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// 加载配置；若文件不存在则先写入默认配置再返回
    pub fn load_or_create(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        if std::path::Path::new(path).exists() {
            return Self::load(path);
        }

        let config = AppConfig::default();
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(path, content)?;
        log::info!("Config file not found, created default: {}", path);
        Ok(config)
    }
}
