/// 协议帧头
#[allow(dead_code)]
pub const FRAME_HEADER: u16 = 0x1234;
/// 协议帧尾
#[allow(dead_code)]
pub const FRAME_TAIL: [u8; 2] = [0xcd, 0xef];

/// 源地址
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
#[allow(dead_code)]
pub enum SourceAddr {
    PC = 0x00,
    PS = 0x10,
}

/// 目的地址
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
#[allow(dead_code)]
pub enum DestAddr {
    PC = 0x00,
    PS = 0x10,
}

/// 协议帧结构（不含帧头帧尾的完整包）
#[derive(Debug, Clone)]
pub struct Frame {
    pub seq: u16,
    pub src: u16,
    pub dst: u16,
    pub total_packets: u16,
    pub packet_idx: u16,
    pub cmd: u16,
    pub data_len: u16,
    pub data: Vec<u8>,
}

impl Frame {
    /// 创建新帧
    pub fn new(cmd: u16, src: u16, dst: u16, data: Vec<u8>) -> Self {
        let data_len = data.len() as u16;
        Self {
            seq: 0,
            src,
            dst,
            total_packets: 1,
            packet_idx: 0,
            cmd,
            data_len,
            data,
        }
    }

    /// 序列化为字节数组
    pub fn to_bytes(&self) -> Vec<u8> {
        let total_len = 16 + self.data.len() + 4; // header(2) + fields(14) + data + reserved(2) + tail(2)
        let mut buf = Vec::with_capacity(total_len);

        // 帧头
        buf.extend_from_slice(&FRAME_HEADER.to_be_bytes());
        // 帧序号
        buf.extend_from_slice(&self.seq.to_be_bytes());
        // 源地址
        buf.extend_from_slice(&self.src.to_be_bytes());
        // 目的地址
        buf.extend_from_slice(&self.dst.to_be_bytes());
        // 总包数
        buf.extend_from_slice(&self.total_packets.to_be_bytes());
        // 包序号
        buf.extend_from_slice(&self.packet_idx.to_be_bytes());
        // 指令
        buf.extend_from_slice(&self.cmd.to_be_bytes());
        // 数据有效长度
        buf.extend_from_slice(&self.data_len.to_be_bytes());
        // 数据区
        buf.extend_from_slice(&self.data);
        // 保留2字节
        buf.extend_from_slice(&[0u8, 0u8]);
        // 帧尾
        buf.extend_from_slice(&FRAME_TAIL);

        buf
    }

    /// 从字节数组反序列化
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 18 {
            return Err("Frame too short".into());
        }

        // 校验帧头
        let header = u16::from_be_bytes([data[0], data[1]]);
        if header != FRAME_HEADER {
            return Err(format!("Invalid header: 0x{:04x}", header));
        }

        // 校验帧尾
        let tail_len = data.len();
        if data[tail_len - 2] != FRAME_TAIL[0] || data[tail_len - 1] != FRAME_TAIL[1] {
            return Err("Invalid tail".into());
        }

        let seq = u16::from_be_bytes([data[2], data[3]]);
        let src = u16::from_be_bytes([data[4], data[5]]);
        let dst = u16::from_be_bytes([data[6], data[7]]);
        let total_packets = u16::from_be_bytes([data[8], data[9]]);
        let packet_idx = u16::from_be_bytes([data[10], data[11]]);
        let cmd = u16::from_be_bytes([data[12], data[13]]);
        let data_len = u16::from_be_bytes([data[14], data[15]]);

        let data_end = 16 + data_len as usize;
        if data_end + 4 > data.len() {
            return Err("Data length exceeds frame".into());
        }

        let payload = data[16..data_end].to_vec();

        Ok(Self {
            seq,
            src,
            dst,
            total_packets,
            packet_idx,
            cmd,
            data_len,
            data: payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_roundtrip() {
        let frame = Frame::new(
            0x0100,
            SourceAddr::PC as u16,
            SourceAddr::PS as u16,
            vec![0x01, 0x02, 0x03, 0x04],
        );
        let bytes = frame.to_bytes();
        let decoded = Frame::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.cmd, 0x0100);
        assert_eq!(decoded.data, vec![0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_empty_data() {
        let frame = Frame::new(0x0102, SourceAddr::PC as u16, SourceAddr::PS as u16, vec![]);
        let bytes = frame.to_bytes();
        let decoded = Frame::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.data_len, 0);
        assert!(decoded.data.is_empty());
    }
}
