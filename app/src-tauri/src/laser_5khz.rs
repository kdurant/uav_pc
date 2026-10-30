//! 5kHz / 25uJ 亚纳秒激光器主控板协议（自适应通讯协议 V3.8）。
//!
//! 串口：115200，8N1，无校验。
//! 发送帧：55 AA <addr> <cmd> <A1> <A2> <A3> <A4> <sum> 33 CC
//! 校验：前 8 字节累加，取低 8 位。
//! 状态帧：AA 55 ... 共 50 字节，主控板地址 0x00。

use crate::laser::{LaserStatus, OpenMode};

/// 主控板地址
pub const MAIN_ADDR: u8 = 0x00;
/// 状态帧长度
const STATUS_LEN: usize = 50;

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b))
}

/// 构造主控板发送帧
pub fn build_frame(cmd: u8, args: [u8; 4]) -> Vec<u8> {
    let mut frame = vec![0x55, 0xAA, MAIN_ADDR, cmd, args[0], args[1], args[2], args[3]];
    let sum = checksum(&frame);
    frame.push(sum);
    frame.push(0x33);
    frame.push(0xCC);
    frame
}

/// 一键开光（文档示例 A1=0）
pub fn build_open() -> Vec<u8> {
    build_frame(0x0C, [0, 0, 0, 0])
}

/// 一键关光
pub fn build_close() -> Vec<u8> {
    build_frame(0x0D, [0, 0, 0, 0])
}

/// 触发模式：0 内触发，1 外触发
pub fn build_mode(mode: OpenMode) -> Vec<u8> {
    build_frame(0x01, [mode as u8, 0, 0, 0])
}

/// 内触发频率，单位 Hz（0-5000）
pub fn build_freq(hz: u32) -> Vec<u8> {
    let b = hz.to_le_bytes();
    build_frame(0x02, [b[0], b[1], b[2], b[3]])
}

/// 振荡级脉宽，单位 µs（50-80）
pub fn build_pulse_width(width_us: u16) -> Vec<u8> {
    let b = width_us.to_le_bytes();
    build_frame(0x03, [b[0], b[1], 0, 0])
}

/// LD1 预置电流，单位 mA；协议单位为 0.01A
pub fn build_current_ma(current_ma: u32) -> Vec<u8> {
    let centi_amp = (current_ma / 10) as u16;
    let b = centi_amp.to_le_bytes();
    build_frame(0x20, [b[0], b[1], 0, 0])
}

/// 清除所有报错
pub fn build_clear_error() -> Vec<u8> {
    build_frame(0x0E, [0, 0, 0, 0])
}

/// 获取状态帧
pub fn build_query_status() -> Vec<u8> {
    build_frame(0xFF, [0, 0, 0, 0])
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// 解析主控板 50 字节状态帧
pub fn parse_status(data: &[u8]) -> Result<LaserStatus, String> {
    if data.len() < STATUS_LEN {
        return Err(format!(
            "5kHz 激光器状态帧长度不足: {} 字节（需要 {}）",
            data.len(),
            STATUS_LEN
        ));
    }
    if data[0] != 0xAA || data[1] != 0x55 {
        return Err(format!(
            "5kHz 激光器状态帧头错误: {:02X} {:02X}",
            data[0], data[1]
        ));
    }
    if data[2] != MAIN_ADDR {
        return Err(format!("5kHz 激光器状态帧地址错误: 0x{:02X}", data[2]));
    }

    let expected = checksum(&data[..47]);
    if data[47] != expected {
        return Err(format!(
            "5kHz 激光器状态帧校验错误: 收到 0x{:02X}，计算 0x{:02X}",
            data[47], expected
        ));
    }
    if data[48] != 0x33 || data[49] != 0xCC {
        return Err("5kHz 激光器状态帧尾错误".into());
    }

    let status_flags = data[45] as u32;
    let error_flags = data[46] as u32;

    Ok(LaserStatus {
        status: (status_flags & 0x01) as u8,
        error_code: data[46],
        laser_number: Some(data[3]),
        freq_detected_hz: Some(u32_at(data, 4)),
        freq_set_hz: Some(u32_at(data, 8)),
        pulse_width_us: Some(u16_at(data, 12)),
        ld_current_ma: Some(u16_at(data, 22).saturating_mul(10)),
        temp1_c: Some(data[28] as i8),
        temp2_c: Some(data[29] as i8),
        q_delay_us: Some(u16_at(data, 34)),
        work_time_s: Some(u32_at(data, 36)),
        status_flags: Some(status_flags),
        error_flags: Some(error_flags),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_frames_match_document() {
        assert_eq!(
            build_open(),
            vec![0x55, 0xAA, 0x00, 0x0C, 0, 0, 0, 0, 0x0B, 0x33, 0xCC]
        );
        assert_eq!(
            build_close(),
            vec![0x55, 0xAA, 0x00, 0x0D, 0, 0, 0, 0, 0x0C, 0x33, 0xCC]
        );
        assert_eq!(
            build_mode(OpenMode::InSide),
            vec![0x55, 0xAA, 0x00, 0x01, 0, 0, 0, 0, 0x00, 0x33, 0xCC]
        );
        assert_eq!(
            build_mode(OpenMode::OutSide),
            vec![0x55, 0xAA, 0x00, 0x01, 0x01, 0, 0, 0, 0x01, 0x33, 0xCC]
        );
        assert_eq!(
            build_freq(5000),
            vec![0x55, 0xAA, 0x00, 0x02, 0x88, 0x13, 0, 0, 0x9C, 0x33, 0xCC]
        );
        assert_eq!(
            build_query_status(),
            vec![0x55, 0xAA, 0x00, 0xFF, 0, 0, 0, 0, 0xFE, 0x33, 0xCC]
        );
    }

    #[test]
    fn test_parse_status() {
        let mut data = vec![0u8; STATUS_LEN];
        data[0] = 0xAA;
        data[1] = 0x55;
        data[2] = 0x00;
        data[3] = 7;
        data[4..8].copy_from_slice(&5000u32.to_le_bytes());
        data[8..12].copy_from_slice(&4000u32.to_le_bytes());
        data[12..14].copy_from_slice(&60u16.to_le_bytes());
        data[22..24].copy_from_slice(&100u16.to_le_bytes());
        data[28] = 30;
        data[29] = (-5i8) as u8;
        data[34..36].copy_from_slice(&10u16.to_le_bytes());
        data[36..40].copy_from_slice(&123u32.to_le_bytes());
        data[45] = 0x03;
        data[46] = 0x00;
        data[47] = checksum(&data[..47]);
        data[48] = 0x33;
        data[49] = 0xCC;

        let status = parse_status(&data).unwrap();
        assert_eq!(status.status, 1);
        assert_eq!(status.laser_number, Some(7));
        assert_eq!(status.freq_detected_hz, Some(5000));
        assert_eq!(status.freq_set_hz, Some(4000));
        assert_eq!(status.pulse_width_us, Some(60));
        assert_eq!(status.ld_current_ma, Some(1000));
        assert_eq!(status.temp1_c, Some(30));
        assert_eq!(status.temp2_c, Some(-5));
        assert_eq!(status.q_delay_us, Some(10));
        assert_eq!(status.work_time_s, Some(123));
        assert_eq!(status.status_flags, Some(0x03));
    }
}
