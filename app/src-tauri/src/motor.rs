//! 电机控制（MAXON EPOS2，经 0x0f00 透传）。
//!
//! 与设备端 `tools/auto_start/src/motor.rs` 保持一致：
//! - 命令字采用 EPOS2 厂商帧：`0x1103`(OpCode+len) + 索引 + 子索引/节点号 + 数据
//! - 未尾追加 CRC-CCITT，再按 "首字大端、其余字小端" 转字节
//! - 发送分 4 步：OpCode → 数据 → 主动 0x4F → 再发 0x4F
//!
//! 这里只负责帧构造与应答解析，收发时序在 commands.rs 中完成。

#![allow(dead_code)]

use serde::Serialize;

/// 单步等待电机应答的超时时间
pub const MOTOR_RESPONSE_TIMEOUT_MS: u64 = 500;

/// 电机操作结果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MotorStatus {
    Success,
    Failed,
    NoFeature,
    NoImplement,
}

/// 电机实时状态
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct MotorState {
    /// 实际位置（编码器单位）
    pub position: i32,
    /// 实际速度（r/s）
    pub velocity: i32,
}

/// EPOS2 控制命令
#[derive(Debug, Clone, Copy)]
pub enum MotorCmd {
    Disable,
    ClearFault,
    Shutdown,
    Enable,
    Halt,
    ProfileVelocityMode,
    MaxProfileVelocity(u16),
    QuickstopDeceleration(u16),
    ProfileAcceleration(u16),
    ProfileDeceleration(u16),
    TargetVelocity(u16),
    ProfilePositionMode,
    ProfileVelocity(u16),
    TargetPosition(i32),
    AbsolutePositionStart,
    HomeMode,
    StartHoming,
    StopHoming,
    HomingMethod(i32),
}

impl MotorCmd {
    /// 命令字（不含 CRC）
    pub fn words(self) -> Vec<u16> {
        use MotorCmd::*;
        match self {
            Disable => vec![0x1103, 0x6040, 0x0100, 0x0100, 0x0000],
            ClearFault => vec![0x1103, 0x6040, 0x0100, 0x0080, 0x0000],
            Shutdown => vec![0x1103, 0x6040, 0x0100, 0x0006, 0x0000],
            Enable => vec![0x1103, 0x6040, 0x0100, 0x000f, 0x0000],
            Halt => vec![0x1103, 0x6040, 0x0100, 0x010f, 0x0000],
            ProfileVelocityMode => vec![0x1103, 0x6060, 0x0100, 0x0003, 0x0000],
            // 与设备端例程一致：最大速度的索引字是 0x6040
            MaxProfileVelocity(v) => vec![0x1103, 0x6040, 0x0100, v, 0x0000],
            QuickstopDeceleration(v) => vec![0x1103, 0x6085, 0x0100, v, 0x0000],
            ProfileAcceleration(v) => vec![0x1103, 0x6083, 0x0100, v, 0x0000],
            ProfileDeceleration(v) => vec![0x1103, 0x6084, 0x0100, v, 0x0000],
            TargetVelocity(v) => vec![0x1103, 0x60ff, 0x0100, v, 0x0000],
            ProfilePositionMode => vec![0x1103, 0x6060, 0x0100, 0x0001, 0x0000],
            ProfileVelocity(v) => vec![0x1103, 0x6081, 0x0100, v, 0x0000],
            TargetPosition(pos) => vec![
                0x1103,
                0x607a,
                0x0100,
                (pos & 0xffff) as u16,
                ((pos >> 16) & 0xffff) as u16,
            ],
            AbsolutePositionStart => vec![0x1103, 0x6040, 0x0100, 0x003f, 0x0000],
            HomeMode => vec![0x1103, 0x6060, 0x0100, 0x0006, 0x0000],
            StartHoming => vec![0x1103, 0x6040, 0x0100, 0x001f, 0x0000],
            StopHoming => vec![0x1103, 0x6040, 0x0100, 0x000f, 0x0000],
            HomingMethod(m) => vec![0x1103, 0x6098, 0x0100, m as u16, 0x0000],
        }
    }

    /// 完整命令（已追加 CRC）
    pub fn frame(self) -> Vec<u8> {
        transmit_word2byte(&word_plus_crc(self.words()))
    }
}

/// 求解电机的默认工作参数（与设备端 init 一致）
pub fn init_sequence() -> Vec<MotorCmd> {
    vec![
        MotorCmd::ClearFault,
        MotorCmd::Shutdown,
        MotorCmd::Halt,
        MotorCmd::ProfileVelocityMode,
        MotorCmd::MaxProfileVelocity(3571),
        MotorCmd::QuickstopDeceleration(45),
        MotorCmd::ProfileAcceleration(45),
        MotorCmd::ProfileDeceleration(45),
    ]
}

/// 速度模式启动：先写目标速度，再使能
pub fn run_sequence(speed: u16) -> Vec<MotorCmd> {
    vec![MotorCmd::TargetVelocity(speed), MotorCmd::Enable]
}

/// 停止：目标速度清零并使能，保持驱动器可控
pub fn stop_sequence() -> Vec<MotorCmd> {
    vec![MotorCmd::TargetVelocity(0), MotorCmd::Enable]
}

/// 绝对定位：故障复位 → 位置模式 → 参数 → 目标位置 → 立即启动
pub fn move_to_sequence(position: i32) -> Vec<MotorCmd> {
    vec![
        MotorCmd::ClearFault,
        MotorCmd::ProfilePositionMode,
        MotorCmd::ProfileVelocity(500),
        MotorCmd::QuickstopDeceleration(45),
        MotorCmd::ProfileAcceleration(45),
        MotorCmd::ProfileDeceleration(45),
        MotorCmd::TargetPosition(position),
        MotorCmd::AbsolutePositionStart,
    ]
}

/// 回零：故障复位 → 回零模式 → 回零方法 → 使能 → 启动
pub fn home_sequence(method: i32) -> Vec<MotorCmd> {
    vec![
        MotorCmd::ClearFault,
        MotorCmd::HomeMode,
        MotorCmd::HomingMethod(method),
        MotorCmd::Shutdown,
        MotorCmd::Enable,
        MotorCmd::Halt,
        MotorCmd::StartHoming,
    ]
}

/// 读取实际速度的命令字（带 CRC）
pub fn read_velocity_frame() -> Vec<u8> {
    transmit_word2byte(&word_plus_crc(vec![0x1001, 0x606c, 0x0100]))
}

/// 读取实际位置的命令字（带 CRC）
pub fn read_position_frame() -> Vec<u8> {
    transmit_word2byte(&word_plus_crc(vec![0x1001, 0x6064, 0x0100]))
}

// ==================== CRC 与字节序 ====================

/// CRC-CCITT（多项式 0x1021，初值 0）
pub fn calc_field_crc(data: &[u16]) -> u16 {
    let mut crc: u16 = 0;
    for &c in data {
        let mut shifter: u16 = 0x8000;
        loop {
            let carry = crc & 0x8000;
            crc <<= 1;
            if c & shifter != 0 {
                crc = crc.wrapping_add(1);
            }
            if carry != 0 {
                crc ^= 0x1021;
            }
            shifter >>= 1;
            if shifter == 0 {
                break;
            }
        }
    }
    crc
}

/// 追加 CRC 字
pub fn word_plus_crc(mut word: Vec<u16>) -> Vec<u16> {
    word.push(0x0000);
    let n = word.len();
    let crc = calc_field_crc(&word);
    word[n - 1] = crc;
    word
}

/// 字数组转字节帧：首字大端，其余字低字节在前
pub fn transmit_word2byte(word: &[u16]) -> Vec<u8> {
    let mut array = Vec::with_capacity(word.len() * 2);
    array.push(((word[0] & 0xff00) >> 8) as u8);
    array.push((word[0] & 0x00ff) as u8);
    for &w in &word[1..] {
        array.push((w & 0x00ff) as u8);
        array.push(((w & 0xff00) >> 8) as u8);
    }
    array
}

// ==================== 应答解析 ====================

/// 解析实际速度应答（payload 长度 11，速度在 byte5/6，低字节在前）
pub fn parse_actual_velocity(payload: &[u8]) -> Option<i32> {
    if payload.len() != 0x0b {
        return None;
    }
    Some(((payload[6] as u16) << 8).wrapping_add(payload[5] as u16) as i32)
}

/// 解析实际位置应答（payload 长度 11，位置在 byte5..9，小端 i32）
pub fn parse_actual_position(payload: &[u8]) -> Option<i32> {
    if payload.len() != 0x0b {
        return None;
    }
    Some(i32::from_le_bytes([
        payload[5], payload[6], payload[7], payload[8],
    ]))
}

/// 角度 → 编码器步数（一圈 163840 步）

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_matches_known_epos_frames() {
        assert_eq!(
            word_plus_crc(MotorCmd::StartHoming.words()).last().copied(),
            Some(0xac31)
        );
        assert_eq!(
            word_plus_crc(MotorCmd::StopHoming.words()).last().copied(),
            Some(0xef52)
        );
        // auto_start 中硬编码的 0xf906 对应回零方法 34
        assert_eq!(
            word_plus_crc(MotorCmd::HomingMethod(34).words())
                .last()
                .copied(),
            Some(0xf906)
        );
    }

    #[test]
    fn test_frame_layout() {
        // Enable: 11 字节 = 5 个字 + CRC，首字大端
        let f = MotorCmd::Enable.frame();
        assert_eq!(f.len(), 12);
        assert_eq!(&f[..2], &[0x11, 0x03]);
        assert_eq!(&f[2..], &[0x40, 0x60, 0x00, 0x01, 0x0f, 0x00, 0x00, 0x00, 0x52, 0xef]);
    }

    #[test]
    fn test_parse_actual_position() {
        let mut p = vec![0u8; 11];
        p[5..9].copy_from_slice(&(-123456i32).to_le_bytes());
        assert_eq!(parse_actual_position(&p), Some(-123456));
        assert_eq!(parse_actual_position(&[0u8; 10]), None);
    }

    #[test]
    fn test_parse_actual_velocity() {
        // 0x03e8 = 1000：低字节在 byte5，高字节在 byte6
        let mut p = vec![0u8; 11];
        p[5] = 0xe8;
        p[6] = 0x03;
        assert_eq!(parse_actual_velocity(&p), Some(1000));
    }

    #[test]
    fn test_sequences_are_non_empty() {
        assert!(!init_sequence().is_empty());
        assert_eq!(run_sequence(100).len(), 2);
        assert_eq!(stop_sequence().len(), 2);
        assert_eq!(move_to_sequence(1000).len(), 8);
        assert_eq!(home_sequence(34).len(), 7);
        assert!(!read_velocity_frame().is_empty());
        assert!(!read_position_frame().is_empty());
    }
}
