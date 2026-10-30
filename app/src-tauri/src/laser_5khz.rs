//! 5kHz / 25uJ 亚纳秒激光器主控板协议（自适应通讯协议 V3.8）。
//!
//! 串口：115200，8N1，无校验。
//! 发送帧：55 AA <addr> <cmd> <A1> <A2> <A3> <A4> <sum> 33 CC
//! 校验：前 8 字节累加，取低 8 位。
//! 状态帧：AA 55 ... 共 50 字节，主控板地址 0x00。

use crate::laser::{LaserStatus, OpenMode};

/// 主控板地址
pub const MAIN_ADDR: u8 = 0x00;
/// 主控板第二帧地址（LD 预置电流等字段）
pub const MAIN_ADDR_2: u8 = 0x99;
/// 振荡级 LD 温控板地址
pub const TEMP_ADDR_LD: u8 = 0x3C;
/// 倍频晶体温控板地址
pub const TEMP_ADDR_SHG: u8 = 0x3D;
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

fn i32_at(data: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// 解析任意板卡的 50 字节状态帧（不校验地址）
fn parse_status_frame(data: &[u8]) -> Result<LaserStatus, String> {
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

    let mut status = LaserStatus {
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
    };

    // 温控板帧的 8-11 字节是实际温度（×10000，有符号），46 字节是错误位
    match data[2] {
        TEMP_ADDR_LD => {
            status.ld_temp_c = Some(i32_at(data, 8) as f64 / 10000.0);
            status.ld_temp_error = Some(data[46]);
        }
        TEMP_ADDR_SHG => {
            status.shg_temp_c = Some(i32_at(data, 8) as f64 / 10000.0);
            status.shg_temp_error = Some(data[46]);
        }
        _ => {}
    }

    Ok(status)
}

/// 判断 50 字节数据是否是结构完整的任意板卡状态帧
fn is_complete_frame(data: &[u8]) -> bool {
    data.len() >= STATUS_LEN
        && data[0] == 0xAA
        && data[1] == 0x55
        && checksum(&data[..STATUS_LEN - 3]) == data[STATUS_LEN - 3]
        && data[STATUS_LEN - 2] == 0x33
        && data[STATUS_LEN - 1] == 0xCC
}

/// 从连续字节流中取出所有完整的板卡状态帧（按到达顺序）。
///
/// 串口总线上主控板(0x00/0x99)、驱动板(0x0A)、温控板(0x3C/0x3D)会同时
/// 应答，设备可能把多帧拼在一次透传里，也可能分多次返回。这里逐帧扫描，
/// 并保留不完整的尾部等待后续数据。
pub fn take_status_frames(buf: &mut Vec<u8>) -> Vec<(u8, LaserStatus)> {
    let mut frames = Vec::new();
    let mut i = 0;

    while i + 2 <= buf.len() {
        if buf[i] != 0xAA || buf[i + 1] != 0x55 {
            i += 1;
            continue;
        }
        if buf.len() - i < STATUS_LEN {
            // 当前候选帧不完整：若后面已经出现新的帧头，说明这是丢字节
            // 留下的残片，跳过它继续找，避免阻塞后续完整帧。
            if buf[i + 2..].windows(2).any(|w| w == [0xAA, 0x55]) {
                i += 1;
                continue;
            }
            break; // 等待更多数据
        }
        if is_complete_frame(&buf[i..i + STATUS_LEN]) {
            let addr = buf[i + 2];
            if let Ok(status) = parse_status_frame(&buf[i..i + STATUS_LEN]) {
                frames.push((addr, status));
            }
            i += STATUS_LEN; // 完整帧，整帧跳过
        } else {
            i += 1;
        }
    }

    if i > 0 {
        buf.drain(..i);
    }
    frames
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

        let status = parse_status_frame(&data).unwrap();
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

    fn make_status_frame(addr: u8, number: u8) -> Vec<u8> {
        let mut data = vec![0u8; STATUS_LEN];
        data[0] = 0xAA;
        data[1] = 0x55;
        data[2] = addr;
        data[3] = number;
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
        data
    }

    #[test]
    fn test_take_status_frames_by_address() {        let other = make_status_frame(0x3D, 21);
        let main_frame = make_status_frame(MAIN_ADDR, 7);
        let aux_frame = make_status_frame(MAIN_ADDR_2, 7);

        // 其它板卡的帧也会被取出，由调用方按地址过滤
        let mut buf = other.clone();
        let frames = take_status_frames(&mut buf);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, 0x3D);
        assert!(buf.is_empty());

        // 多帧拼接：温控帧 + 主控第一帧 + 主控第二帧
        let mut buf = [other.clone(), main_frame.clone(), aux_frame.clone()].concat();
        let frames = take_status_frames(&mut buf);
        let addrs: Vec<u8> = frames.iter().map(|(a, _)| *a).collect();
        assert_eq!(addrs, vec![0x3D, MAIN_ADDR, MAIN_ADDR_2]);
        assert!(buf.is_empty());

        // 主控帧被拆成两次到达：第一次保留尾部，拼上后成功
        let mut buf = main_frame[..30].to_vec();
        assert!(take_status_frames(&mut buf).is_empty());
        assert_eq!(buf.len(), 30);
        buf.extend_from_slice(&main_frame[30..]);
        let frames = take_status_frames(&mut buf);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, MAIN_ADDR);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_parse_temp_frames() {
        // 温控帧：8-11 字节是实际温度（×10000，有符号），46 字节是错误位
        let mut ld = make_status_frame(TEMP_ADDR_LD, 7);
        ld[8..12].copy_from_slice(&380038i32.to_le_bytes());
        ld[46] = 0x01; // ERR_TEMP_OVER
        ld[47] = checksum(&ld[..47]);

        let mut shg = make_status_frame(TEMP_ADDR_SHG, 7);
        shg[8..12].copy_from_slice(&(-12345i32).to_le_bytes());
        shg[46] = 0x02; // ERR_TEMP_LOWER
        shg[47] = checksum(&shg[..47]);

        let mut buf = [ld, shg].concat();
        let frames = take_status_frames(&mut buf);
        assert_eq!(frames.len(), 2);

        let (addr, status) = &frames[0];
        assert_eq!(*addr, TEMP_ADDR_LD);
        assert!((status.ld_temp_c.unwrap() - 38.0038).abs() < 1e-9);
        assert_eq!(status.ld_temp_error, Some(0x01));

        let (addr, status) = &frames[1];
        assert_eq!(*addr, TEMP_ADDR_SHG);
        assert!((status.shg_temp_c.unwrap() + 1.2345).abs() < 1e-9);
        assert_eq!(status.shg_temp_error, Some(0x02));
    }
}
