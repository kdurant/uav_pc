//! 激光器控制（经 0x0200 串口命令透传）。
//!
//! 命令格式与设备端 `tools/auto_start/src/laser.rs` 保持一致：
//! - 打开/关闭：`AA 55 C1 01 01 01 00` / `AA 55 C1 01 00 00 00`
//! - 工作模式：`AA 55 CE 01 <mode> <mode> 00`（00=内触发，01=外触发）
//! - 设置电流：`AA 55 C3 02 <lo> <hi> <sum_lo> <sum_hi>`，电流 ×10
//! - 查询状态：`AA 55 D3 00 00 00`，应答 payload 偏移 4 起 14 字节为状态
//!
//! 应答固定前缀为 `55 AA <cmd> 01`，其后的字节是设备附加状态。

#![allow(dead_code)]

use serde::Serialize;

/// 等待激光器应答的超时时间
pub const LASER_RESPONSE_TIMEOUT_MS: u64 = 1000;

/// 使用本模块 AA55 协议的激光器类型。
/// 其余类型由 `control.rs` 分派（nanjing_5k 走 5kHz 协议；nanjing_20k / keopsys_20k 协议待确认）。
pub fn is_supported_type(laser_type: &str) -> bool {
    matches!(laser_type, "connet_20k" | "nanjing_20k")
}

/// 激光器工作模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    InSide = 0x00,
    OutSide = 0x01,
}

/// 激光器基本信息（0xD1）
#[derive(Debug, Clone, Default, Serialize)]
pub struct LaserBasicInfo {
    /// 项目号
    pub project_no: String,
    /// 硬件版本
    pub hw_version: String,
    /// 软件版本
    pub sw_version: String,
    /// 设备 SN
    pub sn: String,
    /// 输出功率单位
    pub power_unit: String,
    /// 输出功率最大值
    pub power_max: u32,
    /// 功率设置方式
    pub power_set_mode: String,
    /// 产品类别
    pub product_class: String,
    /// 种子温度最小值（℃）
    pub seed_temp_min: f64,
    /// 种子温度最大值（℃）
    pub seed_temp_max: f64,
}

/// 实时信息项（含义来自 0xD2）
#[derive(Debug, Clone, Serialize)]
pub struct LaserRealtimeItem {
    pub label: String,
    pub value: String,
}

/// 激光器实时状态（14 字节，小端）
#[derive(Debug, Clone, Default, Serialize)]
pub struct LaserStatus {
    /// 开关状态：0x00 关闭，其它为开启
    pub status: u8,
    pub error_code: u8,
    /// 箱体温度（℃）
    pub box_temp: f32,
    /// 泵浦激光器 1 温度（℃）
    pub pump1_temp: f32,
    /// 泵浦激光器 2 温度（℃）
    pub pump2_temp: f32,
    /// 泵浦激光器 1 电流（mA）
    pub pump1_current: u16,
    /// 泵浦激光器 2 电流（mA）
    pub pump2_current: u16,
    /// 输出功率（mW）
    pub export_power: u32,

    // ===== 5kHz 激光器扩展字段（nanjing_5k） =====
    /// 激光器编号
    pub laser_number: Option<u8>,
    /// 检测到的触发频率（Hz）
    pub freq_detected_hz: Option<u32>,
    /// 设定的触发频率（Hz）
    pub freq_set_hz: Option<u32>,
    /// 振荡级脉宽（µs）
    pub pulse_width_us: Option<u16>,
    /// LD 实际预置电流（mA）
    pub ld_current_ma: Option<u16>,
    /// 温度检测 1（℃）
    pub temp1_c: Option<i8>,
    /// 温度检测 2（℃）
    pub temp2_c: Option<i8>,
    /// 振荡级 LD 温控实际温度（℃，来自 0x3C 帧）
    pub ld_temp_c: Option<f64>,
    /// 倍频晶体温控实际温度（℃，来自 0x3D 帧）
    pub shg_temp_c: Option<f64>,
    /// 振荡级 LD 温控错误位（0x3C 帧，bit0 过温 / bit1 过低）
    pub ld_temp_error: Option<u8>,
    /// 倍频晶体温控错误位（0x3D 帧，bit0 过温 / bit1 过低）
    pub shg_temp_error: Option<u8>,
    /// 调 Q 延时（µs）
    pub q_delay_us: Option<u16>,
    /// 出光/工作时间（s）
    pub work_time_s: Option<u32>,
    /// 原始状态标志位
    pub status_flags: Option<u32>,
    /// 原始错误标志位
    pub error_flags: Option<u32>,

    // ===== Connet 20k 扩展信息 =====
    /// 基本信息（0xD1）
    pub basic: Option<LaserBasicInfo>,
    /// 实时信息（0xD3，含义由 0xD2 定义）
    pub realtime: Vec<LaserRealtimeItem>,
}

/// 打开激光器
pub fn build_open() -> Vec<u8> {
    vec![0xAA, 0x55, 0xC1, 0x01, 0x01, 0x01, 0x00]
}

/// 关闭激光器
pub fn build_close() -> Vec<u8> {
    vec![0xAA, 0x55, 0xC1, 0x01, 0x00, 0x00, 0x00]
}

/// 设置工作模式（内/外触发）
pub fn build_set_mode(mode: OpenMode) -> Vec<u8> {
    let m = mode as u8;
    vec![0xAA, 0x55, 0xCE, 0x01, m, m, 0x00]
}

/// 设置激光器电流（mA），设备侧按 ×10 写入
pub fn build_set_current(current_ma: u32) -> Vec<u8> {
    let current = (current_ma as u16).wrapping_mul(10);
    let lo = (current & 0xFF) as u8;
    let hi = ((current >> 8) & 0xFF) as u8;
    // 校验和 = 低字节 + 高字节（与设备端例程一致）
    let checksum = lo as u16 + hi as u16;
    vec![
        0xAA,
        0x55,
        0xC3,
        0x02,
        lo,
        hi,
        (checksum & 0xFF) as u8,
        ((checksum >> 8) & 0xFF) as u8,
    ]
}

/// 查询实时状态
pub fn build_query_status() -> Vec<u8> {
    vec![0xAA, 0x55, 0xD3, 0x00, 0x00, 0x00]
}

/// 查询基本信息（0xD1）
pub fn build_query_basic() -> Vec<u8> {
    vec![0xAA, 0x55, 0xD1, 0x00, 0x00, 0x00]
}

/// 查询实时信息含义（0xD2）
pub fn build_query_realtime_meanings() -> Vec<u8> {
    vec![0xAA, 0x55, 0xD2, 0x00, 0x00, 0x00]
}

/// 应答固定前缀长度：`55 AA <cmd> <len>`。
/// 其后的字节是设备返回的附加状态（例如电流设置应答 `55 AA C3 01 04 04 00`
/// 中的 `04 04`），不能作为失败依据。
pub const ACK_PREFIX_LEN: usize = 4;

/// 校验应答：只比较固定前缀
pub fn matches_ack(payload: &[u8], expect: &[u8]) -> bool {
    payload.len() >= ACK_PREFIX_LEN
        && expect.len() >= ACK_PREFIX_LEN
        && payload[..ACK_PREFIX_LEN] == expect[..ACK_PREFIX_LEN]
}

/// 打开/关闭应答
pub fn open_ack() -> &'static [u8] {
    &[0x55, 0xAA, 0xC1, 0x01, 0x00, 0x00, 0x00]
}

/// 模式设置应答
pub fn mode_ack() -> &'static [u8] {
    &[0x55, 0xAA, 0xCE, 0x01, 0x00, 0x00, 0x00]
}

/// 电流设置应答
pub fn current_ack() -> &'static [u8] {
    &[0x55, 0xAA, 0xC3, 0x01, 0x00, 0x00, 0x00]
}

/// 温度原始值 → 摄氏度：raw / 100 - 273.15
fn parse_temp(raw: u16) -> f32 {
    raw as f32 / 100.0 - 273.15
}

/// 解析实时状态应答。
///
/// 状态字段位于 payload 偏移 4 处，共 14 字节：
/// status / error_code / boxTemp / pump1Temp / pump2Temp /
/// pump1Current / pump2Current / exportPower（均小端）。
pub fn parse_status(payload: &[u8]) -> Result<LaserStatus, String> {
    if payload.len() < 4 + 14 {
        return Err(format!(
            "激光器状态应答长度不足: {} 字节（至少 18）",
            payload.len()
        ));
    }
    let d = &payload[4..18];
    let raw_box = u16::from_le_bytes([d[2], d[3]]);
    let raw_pump1 = u16::from_le_bytes([d[4], d[5]]);
    let raw_pump2 = u16::from_le_bytes([d[6], d[7]]);
    Ok(LaserStatus {
        status: d[0],
        error_code: d[1],
        box_temp: parse_temp(raw_box),
        pump1_temp: parse_temp(raw_pump1),
        pump2_temp: parse_temp(raw_pump2),
        pump1_current: u16::from_le_bytes([d[8], d[9]]),
        pump2_current: u16::from_le_bytes([d[10], d[11]]),
        export_power: u16::from_le_bytes([d[12], d[13]]) as u32 * 10,
        ..Default::default()
    })
}

/// 解析 0xD2 应答中的实时信息含义列表
pub fn parse_realtime_meanings(payload: &[u8]) -> Vec<u8> {
    if payload.len() < 6 || payload[0] != 0x55 || payload[1] != 0xAA || payload[2] != 0xD2 {
        return Vec::new();
    }
    // 长度在 payload[3]；数据区第一个字节为保留位，其后是含义列表
    let n = payload[3] as usize;
    if payload.len() < 4 + n {
        return Vec::new();
    }
    payload[5..4 + n].to_vec()
}

fn temp_c(raw: u16) -> f64 {
    raw as f64 / 100.0 - 273.15
}

fn realtime_label(code: u8) -> &'static str {
    match code {
        0x10 => "泵浦温度",
        0x11 => "泵浦1温度",
        0x12 => "泵浦2温度",
        0x1D => "种子温度",
        0x1F => "模块温度",
        0x20 => "泵浦电流",
        0x21 => "泵浦1电流",
        0x22 => "泵浦2电流",
        0x50 => "输出功率",
        0x5E => "泵浦功率",
        _ => "实时信息",
    }
}

/// 解析 0xD3 应答，按 0xD2 的含义列表生成实时信息项
pub fn parse_realtime_status(payload: &[u8], meanings: &[u8]) -> Result<LaserStatus, String> {
    if payload.len() < 8 || payload[0] != 0x55 || payload[1] != 0xAA || payload[2] != 0xD3 {
        return Err("激光器实时信息帧头错误".into());
    }
    let n = payload[3] as usize;
    if n < 2 || payload.len() < 4 + n + 2 {
        return Err(format!("激光器实时信息长度错误: {}", payload.len()));
    }

    let status = payload[4];
    let alarm = payload[5];
    let data = &payload[6..4 + n];

    let mut out = LaserStatus {
        status,
        error_code: alarm,
        ..Default::default()
    };

    for (i, chunk) in data.chunks_exact(2).enumerate() {
        let raw = u16::from_le_bytes([chunk[0], chunk[1]]);
        let code = meanings.get(i).copied().unwrap_or(0);
        let label = realtime_label(code);
        let value = if (0x10..=0x1F).contains(&code) {
            if raw == 0 {
                "—".to_string()
            } else {
                format!("{:.2} ℃", temp_c(raw))
            }
        } else if (0x20..=0x2F).contains(&code) {
            format!("{raw} mA")
        } else {
            format!("{raw}")
        };

        // 兼容旧字段（箱体温度、泵浦温度、电流、功率）
        match code {
            0x10 | 0x1F if raw > 0 => out.box_temp = temp_c(raw) as f32,
            0x11 if raw > 0 => out.pump1_temp = temp_c(raw) as f32,
            0x12 if raw > 0 => out.pump2_temp = temp_c(raw) as f32,
            0x20 | 0x21 => out.pump1_current = raw,
            0x22 => out.pump2_current = raw,
            0x50 => out.export_power = raw as u32,
            _ => {}
        }

        out.realtime.push(LaserRealtimeItem {
            label: label.to_string(),
            value,
        });
    }

    Ok(out)
}

/// 解析 0xD1 基本信息
pub fn parse_basic_info(payload: &[u8]) -> Result<LaserBasicInfo, String> {
    if payload.len() < 4 + 36 || payload[0] != 0x55 || payload[1] != 0xAA || payload[2] != 0xD1 {
        return Err("激光器基本信息帧格式错误".into());
    }
    let d = &payload[4..4 + 36];

    // 该固件的项目号/版本/SN 以数值字节表示，不是 ASCII
    let project_no = format!("{}{}{}", d[0], d[1], d[2]);
    let hw_version = format!("{}.{}", d[3], d[4]);
    let sw_version = format!("{}.{}.{}", d[5], d[6], d[7]);
    let sn: String = d[8..17].iter().map(|b| b.to_string()).collect();

    let power_unit = match d[17] {
        0 => "mW",
        1 => "W",
        2 => "uW",
        _ => "未知",
    };
    let power_set_mode = match d[20] {
        1 => "设置功率",
        2 => "设置电流",
        3 => "设置 DA",
        4 => "电位器",
        5 => "固定功率",
        6 => "通过电压设置",
        7 => "设置泵浦功率",
        8 => "设置峰值功率",
        9 => "设置增益",
        10 => "设置监测端功率",
        _ => "未知",
    };
    let product_class = match d[31] {
        2 => "脉冲激光器（不带放大）",
        3 => "脉冲激光器（带放大）",
        _ => "未知",
    };

    Ok(LaserBasicInfo {
        project_no,
        hw_version,
        sw_version,
        sn,
        power_unit: power_unit.to_string(),
        power_max: u16::from_le_bytes([d[18], d[19]]) as u32,
        power_set_mode: power_set_mode.to_string(),
        product_class: product_class.to_string(),
        seed_temp_min: temp_c(u16::from_le_bytes([d[32], d[33]])),
        seed_temp_max: temp_c(u16::from_le_bytes([d[34], d[35]])),
    })
}

fn checksum_u16(bytes: &[u8]) -> u16 {
    bytes
        .iter()
        .fold(0u16, |acc, &b| acc.wrapping_add(b as u16))
}

/// 从连续字节流中取出完整的 Connet 应答帧（`55 AA <cmd> <len> data sum`）。
///
/// 返回 `(命令字, 完整帧字节)`；不完整的尾部保留，等待后续数据。
pub fn take_frames(buf: &mut Vec<u8>) -> Vec<(u8, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0;

    while i + 6 <= buf.len() {
        if buf[i] != 0x55 || buf[i + 1] != 0xAA {
            i += 1;
            continue;
        }
        let len = buf[i + 3] as usize;
        let total = len + 6;
        if i + total > buf.len() {
            break;
        }
        let expected = checksum_u16(&buf[i + 4..i + 4 + len]);
        let got = u16::from_le_bytes([buf[i + 4 + len], buf[i + 4 + len + 1]]);
        if expected != got {
            // 部分固件的校验范围与文档不一致；帧头/长度正确时仍按帧切分，
            // 避免整条状态链路因校验差异而完全不可用。
            log::warn!(
                "Laser frame checksum mismatch: cmd=0x{:02X}, got=0x{:04X}, calc=0x{:04X}",
                buf[i + 2],
                got,
                expected
            );
        }
        out.push((buf[i + 2], buf[i..i + total].to_vec()));
        i += total;
    }

    if i > 0 {
        buf.drain(..i);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_laser_types() {
        assert!(is_supported_type("connet_20k"));
        assert!(is_supported_type("nanjing_20k"));
        assert!(!is_supported_type("nanjing_5k"));
        assert!(!is_supported_type("keopsys_20k"));
        assert!(!is_supported_type(""));
        assert!(!is_supported_type("laser_connet"));
    }

    #[test]
    fn test_open_close_frame() {
        assert_eq!(build_open(), vec![0xAA, 0x55, 0xC1, 0x01, 0x01, 0x01, 0x00]);
        assert_eq!(build_close(), vec![0xAA, 0x55, 0xC1, 0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_set_mode_frame() {
        assert_eq!(
            build_set_mode(OpenMode::InSide),
            vec![0xAA, 0x55, 0xCE, 0x01, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            build_set_mode(OpenMode::OutSide),
            vec![0xAA, 0x55, 0xCE, 0x01, 0x01, 0x01, 0x00]
        );
    }

    #[test]
    fn test_set_current_checksum() {
        // 1000mA × 10 = 10000 = 0x2710，校验和 = 0x10 + 0x27 = 0x37
        assert_eq!(
            build_set_current(1000),
            vec![0xAA, 0x55, 0xC3, 0x02, 0x10, 0x27, 0x37, 0x00]
        );
    }

    #[test]
    fn test_parse_status() {
        let mut payload = vec![0u8; 18];
        payload[4] = 0x01;
        payload[5] = 0x00;
        payload[6..8].copy_from_slice(&(29815u16).to_le_bytes()); // 25.00℃
        payload[8..10].copy_from_slice(&(30315u16).to_le_bytes()); // 30.00℃
        payload[10..12].copy_from_slice(&(28815u16).to_le_bytes()); // 15.00℃
        payload[12..14].copy_from_slice(&1234u16.to_le_bytes());
        payload[14..16].copy_from_slice(&2345u16.to_le_bytes());
        payload[16..18].copy_from_slice(&350u16.to_le_bytes());

        let s = parse_status(&payload).unwrap();
        assert_eq!(s.status, 1);
        assert!((s.box_temp - 25.0).abs() < 0.01);
        assert!((s.pump1_temp - 30.0).abs() < 0.01);
        assert!((s.pump2_temp - 15.0).abs() < 0.01);
        assert_eq!(s.pump1_current, 1234);
        assert_eq!(s.pump2_current, 2345);
        assert_eq!(s.export_power, 3500);
    }

    #[test]
    fn test_parse_status_too_short() {
        assert!(parse_status(&[0u8; 17]).is_err());
    }

    #[test]
    fn test_matches_ack() {
        assert!(matches_ack(
            &[0x55, 0xAA, 0xC1, 0x01, 0x00, 0x00, 0x00],
            open_ack()
        ));
        // 附加状态字节非 0 时仍视为成功（电流设置实测返回 04 04）
        assert!(matches_ack(
            &[0x55, 0xAA, 0xC3, 0x01, 0x04, 0x04, 0x00],
            &[0x55, 0xAA, 0xC3, 0x01, 0x00, 0x00, 0x00]
        ));
        assert!(!matches_ack(&[0x55, 0xAA, 0xC1, 0x02, 0x00, 0x00], open_ack()));
        assert!(!matches_ack(&[0x55, 0xAA], open_ack()));
    }
}
