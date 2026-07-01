use serde::Serialize;

/// 预览数据帧头部（88字节固定头 + 通道可变数据）
#[derive(Debug, Clone, Serialize)]
pub struct PreviewFrame {
    /// GPS 周
    pub gps_week: u32,
    /// GPS 秒（本周秒数）
    pub gps_second: f64,
    /// 细分时间（100纳秒单位）
    pub sub_time: u32,
    /// 方位角（偏航角）
    pub azimuth: f64,
    /// 俯仰角
    pub pitch: f64,
    /// 横滚角
    pub roll: f64,
    /// GPS 纬度
    pub latitude: f64,
    /// GPS 经度
    pub longitude: f64,
    /// GPS 高度
    pub altitude: f64,
    /// 码盘位数
    pub encoder_bits: u32,
    /// 码盘读数
    pub encoder_value: u32,
    /// 波形通道数
    pub wave_channels: u32,
    /// 波形长度（未使用）
    pub wave_length: u32,
    /// 各通道波形数据
    pub channels: Vec<ChannelData>,
}

/// 单个通道的波形数据
#[derive(Debug, Clone, Serialize)]
pub struct ChannelData {
    /// 通道号 (0=0000, 1=0F0F, 2=F0F0, 3=FFFF)
    pub channel_id: u16,
    /// 第一段起始位置
    pub seg0_start: u16,
    /// 第一段长度
    pub seg0_len: u16,
    /// 第一段数据（u16 数组）
    pub seg0_data: Vec<u16>,
    /// 第二段起始位置
    pub seg1_start: u16,
    /// 第二段长度
    pub seg1_len: u16,
    /// 第二段数据（u16 数组）
    pub seg1_data: Vec<u16>,
}

/// 通道数据帧头标记
const CHANNEL_MAGIC: u32 = 0xEB_90_A5_5A;

/// 帧头 magic（8 字节）
const FRAME_MAGIC: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];

fn read_u32_be(data: &[u8], pos: usize) -> Option<u32> {
    if pos + 4 > data.len() {
        return None;
    }
    Some(u32::from_be_bytes([
        data[pos],
        data[pos + 1],
        data[pos + 2],
        data[pos + 3],
    ]))
}

fn read_u16_be(data: &[u8], pos: usize) -> Option<u16> {
    if pos + 2 > data.len() {
        return None;
    }
    Some(u16::from_be_bytes([data[pos], data[pos + 1]]))
}

fn read_f64_be(data: &[u8], pos: usize) -> Option<f64> {
    if pos + 8 > data.len() {
        return None;
    }
    Some(f64::from_be_bytes([
        data[pos],
        data[pos + 1],
        data[pos + 2],
        data[pos + 3],
        data[pos + 4],
        data[pos + 5],
        data[pos + 6],
        data[pos + 7],
    ]))
}

impl PreviewFrame {
    /// 从字节流解析预览数据帧
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        // 校验帧头 magic（8 字节）
        if data.len() < 8 || data[0..8] != FRAME_MAGIC {
            return Err("Invalid preview frame magic".into());
        }

        let mut offset = 8;

        // 固定头部 88 字节
        let gps_week = read_u32_be(data, offset).ok_or("EOF at gps_week")?;
        offset += 4;
        let gps_second = read_f64_be(data, offset).ok_or("EOF at gps_second")?;
        offset += 8;
        let sub_time = read_u32_be(data, offset).ok_or("EOF at sub_time")?;
        offset += 4;
        let azimuth = read_f64_be(data, offset).ok_or("EOF at azimuth")?;
        offset += 8;
        let pitch = read_f64_be(data, offset).ok_or("EOF at pitch")?;
        offset += 8;
        let roll = read_f64_be(data, offset).ok_or("EOF at roll")?;
        offset += 8;
        let latitude = read_f64_be(data, offset).ok_or("EOF at latitude")?;
        offset += 8;
        let longitude = read_f64_be(data, offset).ok_or("EOF at longitude")?;
        offset += 8;
        let altitude = read_f64_be(data, offset).ok_or("EOF at altitude")?;
        offset += 8;
        let encoder_bits = read_u32_be(data, offset).ok_or("EOF at encoder_bits")?;
        offset += 4;
        let encoder_value = read_u32_be(data, offset).ok_or("EOF at encoder_value")?;
        offset += 4;
        let wave_channels = read_u32_be(data, offset).ok_or("EOF at wave_channels")?;
        offset += 4;
        let wave_length = read_u32_be(data, offset).ok_or("EOF at wave_length")?;
        offset += 4;
        // 固定头部结束，offset = 88

        let mut channels: Vec<ChannelData> = Vec::new();

        // 解析 4 个通道（A/B/C/D）
        for _ch_idx in 0..4 {
            if offset + 12 > data.len() {
                break; // 没有更多数据
            }

            // 通道帧头 magic
            let magic = read_u32_be(data, offset);
            if magic != Some(CHANNEL_MAGIC) {
                break; // 通道数据不完整或不存在
            }
            offset += 4;

            let channel_id = read_u16_be(data, offset).ok_or("EOF at channel_id")?;
            offset += 2;
            let seg0_start = read_u16_be(data, offset).ok_or("EOF at seg0_start")?;
            offset += 2;
            let seg0_len = read_u16_be(data, offset).ok_or("EOF at seg0_len")?;
            offset += 2;

            let seg0_byte_len = seg0_len as usize * 2;
            let mut seg0_data = Vec::with_capacity(seg0_len as usize);
            if offset + seg0_byte_len <= data.len() {
                for _ in 0..seg0_len {
                    seg0_data.push(read_u16_be(data, offset).unwrap_or(0));
                    offset += 2;
                }
            } else {
                break;
            }

            // 第二段（可能不存在）
            let seg1_start: u16;
            let seg1_len: u16;
            let mut seg1_data = Vec::new();

            if offset + 4 <= data.len() {
                seg1_start = read_u16_be(data, offset).unwrap_or(0);
                offset += 2;
                seg1_len = read_u16_be(data, offset).unwrap_or(0);
                offset += 2;

                let seg1_byte_len = seg1_len as usize * 2;
                if seg1_len > 0 && offset + seg1_byte_len <= data.len() {
                    for _ in 0..seg1_len {
                        seg1_data.push(read_u16_be(data, offset).unwrap_or(0));
                        offset += 2;
                    }
                }
            } else {
                break;
            }

            channels.push(ChannelData {
                channel_id,
                seg0_start,
                seg0_len,
                seg0_data,
                seg1_start,
                seg1_len,
                seg1_data,
            });
        }

        Ok(Self {
            gps_week,
            gps_second,
            sub_time,
            azimuth,
            pitch,
            roll,
            latitude,
            longitude,
            altitude,
            encoder_bits,
            encoder_value,
            wave_channels,
            wave_length,
            channels,
        })
    }
}
