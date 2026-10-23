use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicConfig {
    pub local_ip: String,
    pub local_port: u16,
    pub remote_port: u16,
    /// 是否通过 UDP 广播搜索设备；false 时直接使用 device_ip 通信
    #[serde(default = "default_broadcast_search")]
    pub broadcast_search: bool,
    /// broadcast_search = false 时使用的设备 IP 地址
    #[serde(default)]
    pub device_ip: String,
}

fn default_broadcast_search() -> bool {
    true
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
            broadcast_search: true,
            device_ip: String::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    const CAPTURE: &str = r#"
[capture]
preview_coe = 1000
ref_channal = 1
save_channal = 255
wave_len = 6000
first_pos = 100
first_len = 200
second_pos = 400
second_len = 400
sum_value = 0
max_value = 0
pin_threshold = 1000
"#;

    #[test]
    fn test_broadcast_search_defaults_to_true() {
        let toml = format!(
            "[basic]\nlocal_ip = \"192.168.1.111\"\nlocal_port = 12345\nremote_port = 6666\n{}",
            CAPTURE
        );
        let config: AppConfig = toml::from_str(&toml).unwrap();
        assert!(config.basic.broadcast_search);
        assert_eq!(config.basic.device_ip, "");
    }

    #[test]
    fn test_broadcast_search_disabled() {
        let toml = format!(
            "[basic]\nlocal_ip = \"192.168.1.111\"\nlocal_port = 12345\nremote_port = 6666\nbroadcast_search = false\ndevice_ip = \"192.168.1.100\"\n{}",
            CAPTURE
        );
        let config: AppConfig = toml::from_str(&toml).unwrap();
        assert!(!config.basic.broadcast_search);
        assert_eq!(config.basic.device_ip, "192.168.1.100");
    }
}
