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

/// 二进制 IPC 帧头 magic：'PRV1'
const BINARY_MAGIC: u32 = 0x5052_5631;

/// 帧头 magic 长度
pub const FRAME_MAGIC_LEN: usize = 8;

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

/// 帧长度探测失败原因
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameLenError {
    /// 缓冲区数据不足，补读后可能成功
    Incomplete,
    /// 格式错误，无法继续按帧解析
    Invalid(String),
}

/// 判断当前位置是否存在第二段的起始/长度描述符。
///
/// 第二段长度为 0 时设备可能整段省略描述符，此时该位置直接是下一个通道的
/// magic（或整帧结束后的下一帧 magic），不能当作 s1/l1 读取，否则会把
/// magic 字节误读成巨大的长度值。
fn has_seg1_descriptor(data: &[u8], offset: usize) -> bool {
    if offset + 4 > data.len() {
        return false;
    }
    if read_u32_be(data, offset) == Some(CHANNEL_MAGIC) {
        return false;
    }
    if offset + FRAME_MAGIC.len() <= data.len() && data[offset..offset + FRAME_MAGIC.len()] == FRAME_MAGIC
    {
        return false;
    }
    true
}

impl PreviewFrame {
    /// 判断 data 是否以完整帧头 magic 开头（用于磁盘回放重同步/帧边界校验）
    pub fn is_frame_start(data: &[u8]) -> bool {
        data.len() >= FRAME_MAGIC.len() && data[..FRAME_MAGIC.len()] == FRAME_MAGIC
    }

    /// 探测一帧占用的字节数（不复制波形数据），用于磁盘回放建索引
    pub fn frame_len(data: &[u8]) -> Result<usize, FrameLenError> {
        if data.len() < FRAME_MAGIC.len() {
            return Err(FrameLenError::Incomplete);
        }
        if data[..FRAME_MAGIC.len()] != FRAME_MAGIC {
            return Err(FrameLenError::Invalid("帧头 magic 不符".into()));
        }
        // 8 字节帧头 + 80 字节固定头
        if data.len() < 88 {
            return Err(FrameLenError::Incomplete);
        }

        let mut offset = 88;
        for _ in 0..4 {
            if offset + 4 > data.len() {
                return Err(FrameLenError::Incomplete);
            }
            if read_u32_be(data, offset) != Some(CHANNEL_MAGIC) {
                return Err(FrameLenError::Invalid(format!("偏移 {offset} 处缺少通道 magic")));
            }
            offset += 4;

            // 通道号 + 第一段起始/长度
            if offset + 6 > data.len() {
                return Err(FrameLenError::Incomplete);
            }
            let seg0_len = read_u16_be(data, offset + 4).ok_or(FrameLenError::Incomplete)? as usize;
            offset += 6;

            let seg0_bytes = seg0_len * 2;
            if offset + seg0_bytes > data.len() {
                return Err(FrameLenError::Incomplete);
            }
            offset += seg0_bytes;

            if has_seg1_descriptor(data, offset) {
                if offset + 4 > data.len() {
                    return Err(FrameLenError::Incomplete);
                }
                let seg1_len =
                    read_u16_be(data, offset + 2).ok_or(FrameLenError::Incomplete)? as usize;
                offset += 4;

                let seg1_bytes = seg1_len * 2;
                if offset + seg1_bytes > data.len() {
                    return Err(FrameLenError::Incomplete);
                }
                offset += seg1_bytes;
            }
        }

        Ok(offset)
    }

    /// 从字节流解析预览数据帧
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        Self::parse_with_len(data).map(|(frame, _len)| frame)
    }

    /// 从字节流解析预览数据帧，同时返回实际消费的字节数。
    ///
    /// 磁盘回放需要用它推算帧长度，实时预览路径继续用 `parse`。
    pub fn parse_with_len(data: &[u8]) -> Result<(Self, usize), String> {
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

            // 第二段：部分帧（第二段长度为 0 时）连描述符都不写，
            // 表现为紧随其后的就是下一个通道 magic 或下一帧 magic，此时按无第二段处理。
            let seg1_start: u16;
            let seg1_len: u16;
            let mut seg1_data = Vec::new();

            if has_seg1_descriptor(data, offset) {
                seg1_start = read_u16_be(data, offset).unwrap_or(0);
                offset += 2;
                seg1_len = read_u16_be(data, offset).unwrap_or(0);
                offset += 2;

                let seg1_byte_len = seg1_len as usize * 2;
                if offset + seg1_byte_len <= data.len() {
                    for _ in 0..seg1_len {
                        seg1_data.push(read_u16_be(data, offset).unwrap_or(0));
                        offset += 2;
                    }
                } else {
                    break;
                }
            } else {
                seg1_start = 0;
                seg1_len = 0;
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

        Ok((
            Self {
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
            },
            offset,
        ))
    }

    /// 编码为紧凑二进制（小端），供 Tauri IPC Channel 推送。
    ///
    /// 布局：
    /// magic(4) + gps_week(4) + gps_second(f64) + sub_time(4) +
    /// az/pitch/roll/lat/lon/alt(6×f64) +
    /// encoder_bits/encoder_value/wave_channels/wave_length(4×u32) +
    /// channel_count(4) +
    /// 每通道: channel_id/seg0_start/seg0_len(u16) + seg0_data(u16×len)
    ///        + seg1_start/seg1_len(u16) + seg1_data(u16×len)
    pub fn to_binary(&self) -> Vec<u8> {
        let payload_len: usize = self
            .channels
            .iter()
            .map(|c| 12 + (c.seg0_len as usize + c.seg1_len as usize) * 2)
            .sum();
        let mut buf = Vec::with_capacity(88 + payload_len);

        buf.extend_from_slice(&BINARY_MAGIC.to_le_bytes());
        buf.extend_from_slice(&self.gps_week.to_le_bytes());
        buf.extend_from_slice(&self.gps_second.to_le_bytes());
        buf.extend_from_slice(&self.sub_time.to_le_bytes());
        for v in [
            self.azimuth,
            self.pitch,
            self.roll,
            self.latitude,
            self.longitude,
            self.altitude,
        ] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for v in [
            self.encoder_bits,
            self.encoder_value,
            self.wave_channels,
            self.wave_length,
        ] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&(self.channels.len() as u32).to_le_bytes());

        for ch in &self.channels {
            buf.extend_from_slice(&ch.channel_id.to_le_bytes());
            buf.extend_from_slice(&ch.seg0_start.to_le_bytes());
            buf.extend_from_slice(&ch.seg0_len.to_le_bytes());
            for &v in &ch.seg0_data {
                buf.extend_from_slice(&v.to_le_bytes());
            }
            buf.extend_from_slice(&ch.seg1_start.to_le_bytes());
            buf.extend_from_slice(&ch.seg1_len.to_le_bytes());
            for &v in &ch.seg1_data {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }

        buf
    }
}
