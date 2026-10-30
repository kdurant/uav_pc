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

/// 当前上位机支持的激光器类型。
/// `laserConnet` 是设备端 auto_start 的旧写法，保留兼容。
pub fn is_supported_type(laser_type: &str) -> bool {
    matches!(
        laser_type,
        "laser_connet" | "laserConnet" | "laser_5kHz"
    )
}

/// 激光器工作模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    InSide = 0x00,
    OutSide = 0x01,
}

/// 激光器实时状态（14 字节，小端）
#[derive(Debug, Clone, Copy, Default, Serialize)]
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

    // ===== 5kHz 激光器扩展字段（laser_5kHz） =====
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_laser_types() {
        assert!(is_supported_type("laser_connet"));
        assert!(is_supported_type("laserConnet"));
        assert!(is_supported_type("laser_5kHz"));
        assert!(!is_supported_type(""));
        assert!(!is_supported_type("china"));
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
