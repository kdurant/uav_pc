//! 离线数据回放：按帧读取硬盘上保存的原始采集数据文件。
//!
//! 文件格式与设备上传的 0x0300 预览帧一致：
//! 8 字节帧头 `01 23 45 67 89 AB CD EF` + 80 字节固定头 + 4 个通道块
//! （每块 4B magic `EB 90 A5 5A` + 通道号 + 两段位置/长度 + 数据）。
//!
//! 注意：同一文件内帧长度**不固定**——第二段长度为 0 的帧会连第二段描述符
//! 一起省略（示例文件里同时存在 3984 字节和 768 字节两种帧）。因此打开文件时
//! 顺序解析一遍建立「偏移 + 长度」索引（约 1.4MB），之后按索引随机读取单帧，
//! 不会把整个文件读进内存。

use crate::preview::{FrameLenError, PreviewFrame};
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// 建索引时的顺序读取块大小
const INDEX_CHUNK: usize = 8 * 1024 * 1024;

/// 回放文件摘要，返回给前端显示
#[derive(Debug, Clone, Serialize)]
pub struct ReplayInfo {
    /// 文件路径
    pub path: String,
    /// 文件字节数
    pub file_size: u64,
    /// 可完整解析的帧数（末尾残帧不计入）
    pub frame_count: u64,
    /// GPS 周（取首帧）
    pub gps_week: u32,
    /// 首帧 GPS 周内秒
    pub first_gps_second: f64,
    /// 末帧 GPS 周内秒
    pub last_gps_second: f64,
}

/// 单帧位置信息
#[derive(Debug, Clone, Copy)]
struct FrameEntry {
    offset: u64,
    len: u32,
}

/// 一次回放会话：持有文件句柄与帧索引
pub struct ReplaySession {
    file: File,
    frames: Vec<FrameEntry>,
    info: ReplayInfo,
}

/// 读取指定偏移的一帧原始字节
fn read_at(file: &mut File, offset: u64, len: usize) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("定位失败: {e}"))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)
        .map_err(|e| format!("读取失败: {e}"))?;
    Ok(buf)
}

impl ReplaySession {
    /// 打开回放文件并建立帧索引
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("打开文件失败: {e}"))?;
        let file_size = file
            .metadata()
            .map_err(|e| format!("读取文件信息失败: {e}"))?
            .len();

        if file_size < 88 {
            return Err("文件太小，不是采集数据文件".into());
        }

        let started = std::time::Instant::now();
        let frames = Self::build_index(&mut file, file_size)?;
        if frames.is_empty() {
            return Err("文件中未找到有效采集帧".into());
        }
        log::info!(
            "回放索引完成: {} 帧, 耗时 {:?}",
            frames.len(),
            started.elapsed()
        );

        // 首末帧 GPS 时间用于界面展示
        let first = Self::parse_entry(&mut file, frames[0])?;
        let last = Self::parse_entry(&mut file, frames[frames.len() - 1])?;

        let info = ReplayInfo {
            path: path.to_string_lossy().to_string(),
            file_size,
            frame_count: frames.len() as u64,
            gps_week: first.gps_week,
            first_gps_second: first.gps_second,
            last_gps_second: last.gps_second,
        };

        log::info!(
            "回放文件已打开: {} ({} 字节, {} 帧)",
            info.path,
            info.file_size,
            info.frame_count
        );

        Ok(Self { file, frames, info })
    }

    /// 顺序解析整个文件，记录每帧的偏移与长度
    fn build_index(file: &mut File, file_size: u64) -> Result<Vec<FrameEntry>, String> {
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("定位失败: {e}"))?;

        let mut buf: Vec<u8> = Vec::with_capacity(INDEX_CHUNK * 2);
        let mut base: u64 = 0; // buf[0] 对应的文件偏移
        let mut pos: usize = 0; // 当前帧在 buf 中的起点
        let mut frames: Vec<FrameEntry> = Vec::new();

        loop {
            if base + pos as u64 >= file_size {
                break;
            }
            if buf.len() == pos {
                if !read_more(file, &mut buf, &mut base, &mut pos)? {
                    break;
                }
                continue;
            }

            match PreviewFrame::frame_len(&buf[pos..]) {
                Ok(len) => {
                    frames.push(FrameEntry {
                        offset: base + pos as u64,
                        len: len as u32,
                    });
                    pos += len;
                }
                Err(FrameLenError::Incomplete) => {
                    // 缓冲区不足：补读；补不到说明是文件末尾残帧
                    if !read_more(file, &mut buf, &mut base, &mut pos)? {
                        break;
                    }
                }
                Err(FrameLenError::Invalid(msg)) => {
                    return Err(format!("第 {} 帧解析失败: {msg}", frames.len()));
                }
            }
        }

        Ok(frames)
    }

    fn parse_entry(file: &mut File, entry: FrameEntry) -> Result<PreviewFrame, String> {
        let buf = read_at(file, entry.offset, entry.len as usize)?;
        PreviewFrame::parse(&buf)
    }

    pub fn info(&self) -> &ReplayInfo {
        &self.info
    }

    /// 读取指定帧号的完整帧
    pub fn read_frame(&mut self, index: u64) -> Result<PreviewFrame, String> {
        let entry = *self
            .frames
            .get(index as usize)
            .ok_or_else(|| format!("帧号越界: {index}（共 {} 帧）", self.frames.len()))?;
        Self::parse_entry(&mut self.file, entry)
            .map_err(|e| format!("解析第 {index} 帧失败: {e}"))
    }
}

/// 丢弃已消费数据并追加一块新数据；返回是否还有数据可读
fn read_more(
    file: &mut File,
    buf: &mut Vec<u8>,
    base: &mut u64,
    pos: &mut usize,
) -> Result<bool, String> {
    if *pos > 0 {
        buf.drain(..*pos);
        *base += *pos as u64;
        *pos = 0;
    }
    let mut chunk = vec![0u8; INDEX_CHUNK];
    let n = file.read(&mut chunk).map_err(|e| format!("读取失败: {e}"))?;
    if n == 0 {
        return Ok(false);
    }
    buf.extend_from_slice(&chunk[..n]);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 样本文件不存在时跳过，避免在没有测试数据的机器上失败
    fn sample_path() -> Option<PathBuf> {
        let p = PathBuf::from("/home/wj/Desktop/15_294557_294591_Ocean.bin");
        p.exists().then_some(p)
    }

    #[test]
    fn test_open_sample_and_read_frames() {
        let Some(path) = sample_path() else {
            eprintln!("样本文件不存在，跳过");
            return;
        };

        let mut session = ReplaySession::open(&path).expect("打开样本文件失败");
        let info = session.info().clone();

        assert_eq!(info.frame_count, 170_999, "样本文件完整帧数应为 170999");
        assert_eq!(info.gps_week, 2442);
        assert!((info.first_gps_second - 294557.4).abs() < 1e-6);

        // 首帧：4 个通道，均带第二段
        let first = session.read_frame(0).expect("读取首帧失败");
        assert_eq!(first.channels.len(), 4);
        assert_eq!(first.channels[0].seg1_len, 400);

        // 末帧：样本文件末尾是只有第一段的短帧
        let last_index = info.frame_count - 1;
        let last = session.read_frame(last_index).expect("读取末帧失败");
        assert_eq!(last.channels.len(), 4);
        assert_eq!(last.channels[0].seg0_len, 80);
        assert_eq!(last.channels[0].seg1_len, 0);

        // 越界帧号应报错
        assert!(session.read_frame(info.frame_count).is_err());

        // 均匀抽样 200 帧，覆盖长短两种帧
        let step = (info.frame_count / 200).max(1);
        let mut index = 0;
        while index < info.frame_count {
            session
                .read_frame(index)
                .unwrap_or_else(|e| panic!("读取第 {index} 帧失败: {e}"));
            index += step;
        }
    }
}
