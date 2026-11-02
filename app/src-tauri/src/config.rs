use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// 雷达/激光器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarConfig {
    /// 上位机显示名称，例如 "5K 雷达"
    #[serde(default = "default_radar_display_name")]
    pub display_name: String,
    /// 激光器类型，用于选择通信协议；当前支持 laser_connet
    #[serde(default = "default_laser_type")]
    pub laser_type: String,
    /// 自定义激光器信息，仅用于界面展示
    #[serde(default)]
    pub laser_info: String,
}

fn default_radar_display_name() -> String {
    "雷达".to_string()
}

fn default_laser_type() -> String {
    "laser_connet".to_string()
}

impl Default for RadarConfig {
    fn default() -> Self {
        Self {
            display_name: default_radar_display_name(),
            laser_type: default_laser_type(),
            laser_info: String::new(),
        }
    }
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

/// SSD 文件下载配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsdConfig {
    /// TCP 下载分块大小（MB），仅支持 1、2、4
    #[serde(default = "default_ssd_chunk_mb")]
    pub chunk_mb: u32,
}

fn default_ssd_chunk_mb() -> u32 {
    1
}

impl Default for SsdConfig {
    fn default() -> Self {
        Self {
            chunk_mb: default_ssd_chunk_mb(),
        }
    }
}

/// 板载温度历史配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureConfig {
    /// 历史文件中最多保存的点数（每 10 秒一点）
    #[serde(default = "default_temp_points")]
    pub save_points: usize,
    /// 曲线最多显示的点数
    #[serde(default = "default_temp_points")]
    pub display_points: usize,
}

fn default_temp_points() -> usize {
    180
}

impl Default for TemperatureConfig {
    fn default() -> Self {
        Self {
            save_points: default_temp_points(),
            display_points: default_temp_points(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub basic: BasicConfig,
    /// 雷达名称/激光器信息；老配置文件缺省时使用默认值
    #[serde(default)]
    pub radar: RadarConfig,
    pub capture: CaptureConfig,
    /// GPS 配置；老配置文件缺省时使用默认值
    #[serde(default)]
    pub gps: GpsConfig,
    /// SSD 下载配置；老配置文件缺省时使用默认值
    #[serde(default)]
    pub ssd: SsdConfig,
    /// 板载温度历史配置；老配置文件缺省时使用默认值
    #[serde(default)]
    pub temperature: TemperatureConfig,
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
            radar: RadarConfig::default(),
            capture: CaptureConfig::default(),
            gps: GpsConfig::default(),
            ssd: SsdConfig::default(),
            temperature: TemperatureConfig::default(),
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
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// 加载配置；若文件不存在则先写入默认配置再返回
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        if path.exists() {
            return Self::load(path);
        }

        let config = AppConfig::default();
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(path, content)?;
        log::info!(
            "Config file not found, created default: {}",
            path.display()
        );
        Ok(config)
    }

    /// 校验配置合法性（保存前的最后一道关卡，前端校验可被绕过）
    pub fn validate(&self) -> Result<(), String> {
        // 网络
        let local_ip = self.basic.local_ip.trim();
        if local_ip.parse::<std::net::IpAddr>().is_err() {
            return Err(format!(
                "[basic].local_ip 不是合法的 IP 地址: {:?}",
                self.basic.local_ip
            ));
        }
        if self.basic.local_port == 0 {
            return Err("[basic].local_port 必须在 1~65535 之间".to_string());
        }
        if self.basic.remote_port == 0 {
            return Err("[basic].remote_port 必须在 1~65535 之间".to_string());
        }
        if !self.basic.broadcast_search {
            let device_ip = self.basic.device_ip.trim();
            if device_ip.is_empty() {
                return Err(
                    "[basic].broadcast_search = false 时必须配置 [basic].device_ip".to_string(),
                );
            }
            if device_ip.parse::<std::net::IpAddr>().is_err() {
                return Err(format!(
                    "[basic].device_ip 不是合法的 IP 地址: {:?}",
                    self.basic.device_ip
                ));
            }
        }

        // 雷达 / 激光器
        let laser_type = self.radar.laser_type.trim();
        if !laser_type.is_empty()
            && !matches!(laser_type, "laser_connet" | "laserConnet" | "laser_5kHz")
        {
            return Err(format!(
                "[radar].laser_type 不支持: {:?}（可选 laser_connet / laser_5kHz）",
                self.radar.laser_type
            ));
        }

        // 采集参数
        if self.capture.ref_channal > 3 {
            return Err("[capture].ref_channal 必须在 0~3 之间".to_string());
        }
        if self.capture.save_channal > 0xff {
            return Err("[capture].save_channal 必须在 0~255 之间".to_string());
        }
        if self.capture.wave_len == 0 {
            return Err("[capture].wave_len 必须大于 0".to_string());
        }

        // GPS
        if self.gps.dir.trim().is_empty() {
            return Err("[gps].dir 不能为空".to_string());
        }

        // SSD 分块大小（与下载实现一致，仅支持 1/2/4 MB）
        if !matches!(self.ssd.chunk_mb, 1 | 2 | 4) {
            return Err(format!(
                "[ssd].chunk_mb 只能为 1、2 或 4，当前为 {}",
                self.ssd.chunk_mb
            ));
        }

        // 温度历史点数
        const MAX_TEMP_POINTS: usize = 100_000;
        if self.temperature.save_points == 0 || self.temperature.save_points > MAX_TEMP_POINTS {
            return Err(format!(
                "[temperature].save_points 必须在 1~{} 之间",
                MAX_TEMP_POINTS
            ));
        }
        if self.temperature.display_points == 0
            || self.temperature.display_points > MAX_TEMP_POINTS
        {
            return Err(format!(
                "[temperature].display_points 必须在 1~{} 之间",
                MAX_TEMP_POINTS
            ));
        }

        Ok(())
    }

    /// 保存配置：先写临时文件再原子替换，覆盖前把旧文件备份为 .bak。
    /// 注意：参数名 `path` 是 config.toml 的完整路径。
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let content = toml::to_string_pretty(self)?;

        // 先写同目录临时文件，避免写入中断导致配置损坏
        let tmp_path = path.with_extension("toml.tmp");
        std::fs::write(&tmp_path, content)?;

        // 覆盖前备份旧配置，方便出错后手工恢复
        if path.exists() {
            let backup = path.with_extension("toml.bak");
            std::fs::copy(path, &backup)?;
        }

        // Windows 下 rename 不能覆盖已存在文件，需先删除（旧文件已备份）
        #[cfg(windows)]
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        std::fs::rename(&tmp_path, path)?;

        log::info!("Config saved: {}", path.display());
        Ok(())
    }
}

/// config.toml 完整路径：启动加载与界面保存共用，避免两处路径不一致
pub fn config_path() -> PathBuf {
    app_dir().join("config.toml")
}

/// 可执行文件所在目录。
/// `config.toml` 与温度历史文件都放在这里，避免依赖启动时的工作目录。
pub fn app_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
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
        assert_eq!(config.temperature.save_points, 180);
        assert_eq!(config.temperature.display_points, 180);
        assert_eq!(config.ssd.chunk_mb, 1);
        assert_eq!(config.radar.display_name, "雷达");
        assert_eq!(config.radar.laser_type, "laser_connet");
        assert_eq!(config.radar.laser_info, "");
    }

    #[test]
    fn test_radar_config() {
        let toml = format!(
            "[basic]\nlocal_ip = \"192.168.1.111\"\nlocal_port = 12345\nremote_port = 6666\n[radar]\ndisplay_name = \"5K 雷达\"\nlaser_type = \"laser_connet\"\nlaser_info = \"Laser A / 20kHz\"\n{}",
            CAPTURE
        );
        let config: AppConfig = toml::from_str(&toml).unwrap();
        assert_eq!(config.radar.display_name, "5K 雷达");
        assert_eq!(config.radar.laser_type, "laser_connet");
        assert_eq!(config.radar.laser_info, "Laser A / 20kHz");
    }

    #[test]
    fn test_ssd_chunk_mb_from_config() {
        let toml = format!(
            "[basic]\nlocal_ip = \"192.168.1.111\"\nlocal_port = 12345\nremote_port = 6666\n[ssd]\nchunk_mb = 4\n{}",
            CAPTURE
        );
        let config: AppConfig = toml::from_str(&toml).unwrap();
        assert_eq!(config.ssd.chunk_mb, 4);
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

    #[test]
    fn test_default_config_passes_validation() {
        AppConfig::default().validate().unwrap();
    }

    #[test]
    fn test_validate_rejects_bad_ip() {
        let mut config = AppConfig::default();
        config.basic.local_ip = "192.168.1".to_string();
        assert!(config.validate().is_err());

        config.basic.local_ip = "192.168.1.10".to_string();
        config.basic.broadcast_search = false;
        config.basic.device_ip = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_bad_ports_and_chunk() {
        let mut config = AppConfig::default();
        config.basic.local_port = 0;
        assert!(config.validate().is_err());

        let mut config = AppConfig::default();
        config.ssd.chunk_mb = 3;
        assert!(config.validate().is_err());

        let mut config = AppConfig::default();
        config.temperature.save_points = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_save_and_reload() {
        let dir = std::env::temp_dir().join(format!(
            "uav_pc_config_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        let mut config = AppConfig::default();
        config.radar.display_name = "测试雷达".to_string();
        config.save(&path).unwrap();

        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.radar.display_name, "测试雷达");

        // 第二次保存应生成备份文件
        config.radar.display_name = "测试雷达 2".to_string();
        config.save(&path).unwrap();
        assert!(path.with_extension("toml.bak").exists());
        assert_eq!(
            AppConfig::load(&path).unwrap().radar.display_name,
            "测试雷达 2"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
