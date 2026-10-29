//! 点云 / DEM 生成：把原始采集波形转换成水下地形点云。
//!
//! 数据链路（参数依据实测样本与硬件确认）：
//! - ADC 1 GHz，1 个样点 = 1 ns，往返换算 `0.15 m/样点`
//! - 每帧是旋转扫描的一个角度切片，角度取自帧头 +76 字段（17 位，131072 计数/圈）
//! - 每帧两个提取窗口：窗口 0 覆盖水面回波，窗口 1 覆盖水底回波
//! - 4 个通道是同一信号的不同增益，选一个未饱和通道做峰值检测
//! - 窗口起点相对激光发射时刻有一个固定偏移 T0（按 GPS 高度与水面回波自动解算）
//!
//! 处理全程按顺序流式读文件，不建索引、不整文件入内存，因此可以处理
//! 几十 GB 的单文件；通过 `cloud-progress` 事件上报进度。

use crate::preview::FrameLenError;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// 样点对应的往返距离（米）：1 ns × c / 2
const METERS_PER_SAMPLE: f64 = 0.15;
/// 角度编码器每圈计数
const ENCODER_PER_REV: f64 = 131072.0;
/// 帧头 magic
const FRAME_MAGIC: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
/// 通道块 magic
const CHANNEL_MAGIC: u32 = 0xEB90A55A;
/// 顺序读取块大小
const CHUNK: usize = 8 * 1024 * 1024;
/// 点云点数上限（体素去重后仍超限时停止追加，避免内存失控）
const MAX_POINTS: usize = 6_000_000;
/// 水位（水面）判定用的饱和阈值（12bit ADC）
const SATURATION: u16 = 3900;

/// 点云生成参数
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudParams {
    /// 起始帧号
    pub start_frame: u64,
    /// 处理帧数，0 表示直到文件结束
    pub frame_count: u64,
    /// 点云体素边长（米）
    pub voxel: f32,
    /// DEM 格网边长（米）
    pub dem_cell: f32,
    /// 扫描镜偏离天底的角度（度）
    pub scan_angle_deg: f32,
    /// 是否做水下折射改正
    pub refraction: bool,
}

impl Default for CloudParams {
    fn default() -> Self {
        Self {
            start_frame: 0,
            frame_count: 0,
            voxel: 0.3,
            dem_cell: 1.0,
            scan_angle_deg: 15.0,
            refraction: true,
        }
    }
}

/// 一帧的解析结果
#[derive(Debug, Clone, Copy)]
struct Echo {
    /// 角度编码器计数
    enc: u32,
    /// GPS 高度（米）
    altitude: f64,
    /// 纬度 / 经度
    lat: f64,
    lon: f64,
    /// 水面回波（绝对样点号, 幅值）
    surface: Option<(f64, f32)>,
    /// 水底回波（绝对样点号, 幅值）
    bottom: Option<(f64, f32)>,
}

fn be_u32(d: &[u8], p: usize) -> Option<u32> {
    (p + 4 <= d.len()).then(|| u32::from_be_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]))
}

fn be_u16(d: &[u8], p: usize) -> Option<u16> {
    (p + 2 <= d.len()).then(|| u16::from_be_bytes([d[p], d[p + 1]]))
}

fn be_f64(d: &[u8], p: usize) -> Option<f64> {
    if p + 8 > d.len() {
        return None;
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[p..p + 8]);
    Some(f64::from_be_bytes(b))
}

/// 单窗口峰值：返回（相对窗口起点的样点号, 幅值）
fn window_peak(seg: &[u16]) -> Option<(f64, f32)> {
    if seg.is_empty() {
        return None;
    }
    let (idx, &max) = seg
        .iter()
        .enumerate()
        .max_by_key(|(_, &v)| v)
        .map(|(i, v)| (i, v))?;
    if max == 0 {
        return None;
    }
    // 半高宽内做质心细化，提升亚样点精度
    let lo = idx.saturating_sub(2);
    let hi = (idx + 3).min(seg.len());
    let th = (max as f32) * 0.5;
    let mut sum_w = 0.0f64;
    let mut sum_iw = 0.0f64;
    for (i, &v) in seg.iter().enumerate().take(hi).skip(lo) {
        let w = v as f32;
        if w >= th {
            sum_w += w as f64;
            sum_iw += i as f64 * w as f64;
        }
    }
    let pos = if sum_w > 0.0 {
        sum_iw / sum_w
    } else {
        idx as f64
    };
    Some((pos, max as f32))
}

/// 解析一帧，抽取水面 / 水底回波
fn analyze_frame(data: &[u8]) -> Option<Echo> {
    let enc = be_u32(data, 76)?;
    let altitude = be_f64(data, 64)?;
    let lat = be_f64(data, 48)?;
    let lon = be_f64(data, 56)?;
    // 通道 0 作为时间基准（其余通道是同一信号的不同增益），
    // 幅值取未饱和通道里的最大值，用于着色
    let mut surface: Option<f64> = None;
    let mut bottom: Option<f64> = None;
    let mut amp: f32 = 0.0;
    let mut offset = 88usize;

    for ch in 0..4 {
        if offset + 10 > data.len() || be_u32(data, offset) != Some(CHANNEL_MAGIC) {
            break;
        }
        let s0 = be_u16(data, offset + 6)? as usize;
        let l0 = be_u16(data, offset + 8)? as usize;
        offset += 10;

        if offset + l0 * 2 > data.len() {
            break;
        }
        let seg0: Vec<u16> = (0..l0)
            .map(|i| be_u16(data, offset + i * 2).unwrap_or(0))
            .collect();
        offset += l0 * 2;

        // 第二段（可能整段缺失）
        let mut s1 = 0usize;
        let mut seg1: Vec<u16> = Vec::new();
        if offset + 8 <= data.len()
            && be_u32(data, offset) != Some(CHANNEL_MAGIC)
            && data[offset..offset + 8] != FRAME_MAGIC
        {
            s1 = be_u16(data, offset)? as usize;
            let l1 = be_u16(data, offset + 2)? as usize;
            offset += 4;
            if offset + l1 * 2 <= data.len() {
                seg1 = (0..l1)
                    .map(|i| be_u16(data, offset + i * 2).unwrap_or(0))
                    .collect();
                offset += l1 * 2;
            } else {
                break;
            }
        }

        let p0 = window_peak(&seg0);
        let p1 = window_peak(&seg1);

        if ch == 0 {
            surface = p0.map(|x| s0 as f64 + x.0);
            bottom = p1.map(|x| s1 as f64 + x.0);
            amp = p0.map(|x| x.1).unwrap_or(0.0);
        } else if let Some(a) = p0.map(|x| x.1) {
            if a < SATURATION as f32 && a > amp {
                amp = a;
            }
        }
    }

    let Some(surface) = surface else {
        return None;
    };
    // 底回波必须在水面之后、且深度合理（0.15~450 m）
    let bottom = bottom.filter(|&b| {
        let d = b - surface;
        d > 1.0 && d < 3000.0
    });
    Some(Echo {
        enc,
        altitude,
        lat,
        lon,
        surface: Some((surface, amp)),
        bottom: bottom.map(|b| (b, amp)),
    })
}

/// 顺序帧读取器：分块读文件，按帧边界切分
struct FrameReader {
    file: File,
    buf: Vec<u8>,
    /// buf[0] 在文件中的偏移
    base: u64,
    /// 当前帧在 buf 中的起点
    pos: usize,
    file_size: u64,
}

impl FrameReader {
    fn new(path: &Path) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("打开文件失败: {e}"))?;
        let file_size = file
            .metadata()
            .map_err(|e| format!("读取文件信息失败: {e}"))?
            .len();
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("定位失败: {e}"))?;
        Ok(Self {
            file,
            buf: Vec::with_capacity(CHUNK * 2),
            base: 0,
            pos: 0,
            file_size,
        })
    }

    fn refill(&mut self) -> Result<bool, String> {
        if self.pos > 0 {
            self.buf.drain(..self.pos);
            self.base += self.pos as u64;
            self.pos = 0;
        }
        let mut chunk = vec![0u8; CHUNK];
        let n = self.file.read(&mut chunk).map_err(|e| format!("读取失败: {e}"))?;
        if n == 0 {
            return Ok(false);
        }
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(true)
    }

    /// 取下一帧所在区间 [start, start+len)
    fn next_frame(&mut self) -> Result<Option<(usize, usize)>, String> {
        loop {
            if self.base + self.pos as u64 >= self.file_size {
                return Ok(None);
            }
            if self.buf.len() == self.pos {
                if !self.refill()? {
                    return Ok(None);
                }
                continue;
            }
            match crate::preview::PreviewFrame::frame_len(&self.buf[self.pos..]) {
                Ok(len) => {
                    let start = self.pos;
                    self.pos += len;
                    return Ok(Some((start, len)));
                }
                Err(FrameLenError::Incomplete) => {
                    if !self.refill()? {
                        return Ok(None);
                    }
                }
                Err(FrameLenError::Invalid(e)) => {
                    return Err(format!("第 {} 字节处帧结构异常: {e}", self.base + self.pos as u64));
                }
            }
        }
    }
}

/// 生成结果（二进制编码，见 `encode`）
pub struct CloudResult {
    pub points: Vec<[f32; 4]>,
    pub dem: Vec<f32>,
    pub dem_w: usize,
    pub dem_h: usize,
    pub dem_x0: f32,
    pub dem_y0: f32,
    pub dem_cell: f32,
    pub z_min: f32,
    pub z_max: f32,
    pub surface_points: u32,
    pub bottom_points: u32,
    /// 自动解算的窗口时间偏移（样点）
    pub t0: f64,
    /// 处理帧数
    pub frame_count: u64,
    /// 点数超过上限被截断
    pub truncated: bool,
}

impl CloudResult {
    /// 二进制布局（小端）：
    /// magic "CLD1"(4) + point_count(u32) + dem_w(u32) + dem_h(u32) +
    /// dem_x0(f32) + dem_y0(f32) + dem_cell(f32) + z_min(f32) + z_max(f32) +
    /// surface_points(u32) + bottom_points(u32) + t0(f32) + frame_count(u32) +
    /// truncated(u32) +
    /// points[n][x,y,z,intensity](f32) + dem[w*h](f32, NaN 表示无数据)
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64 + self.points.len() * 16 + self.dem.len() * 4);
        buf.extend_from_slice(b"CLD1");
        buf.extend_from_slice(&(self.points.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(self.dem_w as u32).to_le_bytes());
        buf.extend_from_slice(&(self.dem_h as u32).to_le_bytes());
        for v in [self.dem_x0, self.dem_y0, self.dem_cell, self.z_min, self.z_max] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for v in [self.surface_points, self.bottom_points] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&(self.t0 as f32).to_le_bytes());
        buf.extend_from_slice(&(self.frame_count as u32).to_le_bytes());
        buf.extend_from_slice(&(self.truncated as u32).to_le_bytes());
        for p in &self.points {
            for v in p {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in &self.dem {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }
}

/// 进度回调：已处理帧数 / 总帧数
pub type ProgressFn<'a> = &'a dyn Fn(u64, u64);

/// 生成点云
pub fn build(path: &Path, params: &CloudParams, progress: ProgressFn<'_>) -> Result<CloudResult, String> {
    if !path.exists() {
        return Err(format!("文件不存在: {}", path.display()));
    }
    let phi = (params.scan_angle_deg as f64).to_radians().clamp(0.0, 45.0);
    let voxel = (params.voxel as f64).clamp(0.05, 5.0);
    let dem_cell = (params.dem_cell as f64).clamp(0.2, 10.0);

    // ---------- 第一遍：抽样水面回波，解算窗口固定偏移 T0 ----------
    let mut reader = FrameReader::new(path)?;
    let mut frame_no: u64 = 0;
    let mut offsets = Vec::with_capacity(4096);
    let mut alt_sum = 0.0f64;
    let mut alt_n = 0u32;
    let sample_step: u64 = 13; // 稀疏抽样，足够求中位数

    while let Some((start, len)) = reader.next_frame()? {
        frame_no += 1;
        if frame_no % sample_step != 0 {
            continue;
        }
        if offsets.len() >= 2000 {
            break;
        }
        let data = &reader.buf[start..start + len];
        if let Some(e) = analyze_frame(data) {
            if let (Some((s, _)), true) = (e.surface, e.altitude.is_finite()) {
                offsets.push(s);
                alt_sum += e.altitude;
                alt_n += 1;
            }
        }
    }
    if offsets.is_empty() {
        return Err("未在文件中检测到水面回波".into());
    }
    offsets.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_surface = offsets[offsets.len() / 2];
    let median_alt = alt_sum / alt_n.max(1) as f64;
    // 斜距 = 高度 / cos(偏离天底角)
    let surface_range = median_alt / phi.cos();
    let t0 = surface_range / METERS_PER_SAMPLE - median_surface;
    log::info!(
        "点云标定: 高度 {:.2} m, 水面样点 {:.2}, T0 {:.1} 样点",
        median_alt,
        median_surface,
        t0
    );

    // ---------- 第二遍：逐帧生成点 ----------
    let mut reader = FrameReader::new(path)?;
    let mut frame_no: u64 = 0;
    let mut processed: u64 = 0;
    let mut points: Vec<[f32; 4]> = Vec::new();
    let mut voxels: HashMap<(i32, i32, i32), ()> = HashMap::new();
    let mut bottom_xy: Vec<(f64, f64, f64)> = Vec::new(); // (x, y, z) 用于 DEM

    let mut prev_enc: Option<u32> = None;
    let mut theta = 0.0f64;
    let mut lat0: Option<(f64, f64, f64)> = None;
    let mut surface_points = 0u32;
    let mut bottom_points = 0u32;
    let mut z_min = f32::INFINITY;
    let mut z_max = f32::NEG_INFINITY;
    let mut truncated = false;
    let total_hint = if params.frame_count == 0 {
        0
    } else {
        params.frame_count
    };

    while let Some((start, len)) = reader.next_frame()? {
        let index = frame_no;
        frame_no += 1;
        if index < params.start_frame {
            continue;
        }
        if params.frame_count > 0 && processed >= params.frame_count {
            break;
        }
        processed += 1;
        if processed % 2000 == 0 {
            progress(processed, total_hint);
        }

        let data = &reader.buf[start..start + len];
        let Some(e) = analyze_frame(data) else {
            continue;
        };
        let origin = *lat0.get_or_insert((e.lat, e.lon, e.altitude));

        // 角度解缠
        let d_enc = match prev_enc {
            Some(p) => ((e.enc as i64 - p as i64).rem_euclid(ENCODER_PER_REV as i64)) as f64,
            None => 0.0,
        };
        prev_enc = Some(e.enc);
        theta += d_enc / ENCODER_PER_REV * std::f64::consts::TAU;

        // 平台位置（相对首帧的局部平面坐标，米）
        let x0 = (e.lon - origin.1) * 111_320.0 * origin.0.to_radians().cos();
        let y0 = (e.lat - origin.0) * 110_540.0;
        let z0 = e.altitude - origin.2;

        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        let dir = (sin_phi * theta.cos(), sin_phi * theta.sin(), -cos_phi);

        // 水面点
        if let Some((s_idx, amp)) = e.surface {
            let r = (s_idx + t0) * METERS_PER_SAMPLE;
            let p = (x0 + dir.0 * r, y0 + dir.1 * r, z0 + dir.2 * r);
            surface_points += 1;
            push_point(
                &mut points,
                &mut voxels,
                p,
                amp / 4095.0,
                voxel,
                &mut z_min,
                &mut z_max,
                &mut truncated,
            );
        }

        // 水底点（可选折射改正）
        if let Some((b_idx, amp)) = e.bottom {
            let rb = (b_idx + t0) * METERS_PER_SAMPLE;
            let p = if params.refraction {
                if let Some((s_idx, _)) = e.surface {
                    let rs = (s_idx + t0) * METERS_PER_SAMPLE;
                    let p_s = (x0 + dir.0 * rs, y0 + dir.1 * rs, z0 + dir.2 * rs);
                    let path = (rb - rs).max(0.0);
                    let n = 1.334f64;
                    let sin_w = (sin_phi / n).min(1.0);
                    let cos_w = (1.0 - sin_w * sin_w).sqrt();
                    let dw = (sin_w * theta.cos(), sin_w * theta.sin(), -cos_w);
                    (
                        p_s.0 + dw.0 * path,
                        p_s.1 + dw.1 * path,
                        p_s.2 + dw.2 * path,
                    )
                } else {
                    (x0 + dir.0 * rb, y0 + dir.1 * rb, z0 + dir.2 * rb)
                }
            } else {
                (x0 + dir.0 * rb, y0 + dir.1 * rb, z0 + dir.2 * rb)
            };
            bottom_points += 1;
            if push_point(
                &mut points,
                &mut voxels,
                p,
                amp / 4095.0,
                voxel,
                &mut z_min,
                &mut z_max,
                &mut truncated,
            ) {
                bottom_xy.push(p);
            }
        }
    }
    progress(processed, total_hint.max(processed));

    if points.is_empty() {
        return Err("选定范围内没有生成有效点".into());
    }

    // ---------- DEM：按格网取最深值 ----------
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y, _) in &bottom_xy {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    // 单张 DEM 最多 400 万格：超大面积时自动放大格网，保证仍覆盖全部范围
    let mut dem_cell_eff = dem_cell;
    let (mut dem_w, mut dem_h) = (0usize, 0usize);
    if !bottom_xy.is_empty() {
        loop {
            dem_w = ((max_x - min_x) / dem_cell_eff).floor() as usize + 1;
            dem_h = ((max_y - min_y) / dem_cell_eff).floor() as usize + 1;
            if dem_w * dem_h <= 4_000_000 || dem_cell_eff > 50.0 {
                break;
            }
            dem_cell_eff *= 1.5;
        }
    }
    let mut dem = vec![f32::NAN; dem_w * dem_h];
    for &(x, y, z) in &bottom_xy {
        let cx = (((x - min_x) / dem_cell_eff).floor() as isize).clamp(0, dem_w as isize - 1) as usize;
        let cy = (((y - min_y) / dem_cell_eff).floor() as isize).clamp(0, dem_h as isize - 1) as usize;
        let cell = &mut dem[cy * dem_w + cx];
        // 取最深（z 最小）的底点作为该格网高程
        if cell.is_nan() || z < *cell as f64 {
            *cell = z as f32;
        }
    }

    Ok(CloudResult {
        points,
        dem,
        dem_w,
        dem_h,
        dem_x0: if bottom_xy.is_empty() { 0.0 } else { min_x as f32 },
        dem_y0: if bottom_xy.is_empty() { 0.0 } else { min_y as f32 },
        dem_cell: dem_cell_eff as f32,
        z_min,
        z_max,
        surface_points,
        bottom_points,
        t0,
        frame_count: processed,
        truncated,
    })
}

/// 体素去重后追加点；返回是否真的新增
fn push_point(
    points: &mut Vec<[f32; 4]>,
    voxels: &mut HashMap<(i32, i32, i32), ()>,
    p: (f64, f64, f64),
    intensity: f32,
    voxel: f64,
    z_min: &mut f32,
    z_max: &mut f32,
    truncated: &mut bool,
) -> bool {
    let key = (
        (p.0 / voxel).floor() as i32,
        (p.1 / voxel).floor() as i32,
        (p.2 / voxel).floor() as i32,
    );
    if voxels.contains_key(&key) {
        return false;
    }
    if points.len() >= MAX_POINTS {
        *truncated = true;
        return false;
    }
    voxels.insert(key, ());
    *z_min = z_min.min(p.2 as f32);
    *z_max = z_max.max(p.2 as f32);
    points.push([p.0 as f32, p.1 as f32, p.2 as f32, intensity]);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample() -> Option<PathBuf> {
        let p = PathBuf::from("/home/wj/Desktop/15_294557_294591_Ocean.bin");
        p.exists().then_some(p)
    }

    #[test]
    fn test_build_cloud_from_sample() {
        let Some(path) = sample() else {
            eprintln!("样本文件不存在，跳过");
            return;
        };
        let params = CloudParams {
            start_frame: 0,
            frame_count: 20_000,
            voxel: 0.2,
            dem_cell: 1.0,
            scan_angle_deg: 15.0,
            refraction: true,
        };
        let started = std::time::Instant::now();
        let r = build(&path, &params, &|_, _| {}).expect("生成点云失败");
        eprintln!(
            "点云: {} 帧, {} 点 (水面 {} / 水底 {}), T0={:.1}, z∈[{:.1}, {:.1}], DEM {}x{} @{:.1} m, 耗时 {:?}",
            r.frame_count,
            r.points.len(),
            r.surface_points,
            r.bottom_points,
            r.t0,
            r.z_min,
            r.z_max,
            r.dem_w,
            r.dem_h,
            r.dem_cell,
            started.elapsed()
        );
        // T0 应接近 573 样点（86 m 窗口偏移）
        assert!((r.t0 - 573.0).abs() < 30.0, "T0 = {}", r.t0);
        assert_eq!(r.surface_points, 20_000);
        assert!(r.bottom_points > 19_000);
        assert!(!r.points.is_empty());
        assert!(r.dem_w > 0 && r.dem_h > 0);
        // 深度分布诊断：底点应明显低于水面
        let mut depths: Vec<f32> = r
            .points
            .iter()
            .filter(|p| p[2] < -120.0)
            .map(|p| -p[2])
            .collect();
        depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if !depths.is_empty() {
            let q = |f: f64| depths[((depths.len() - 1) as f64 * f) as usize];
            eprintln!(
                "底点高程分位: p1={:.1} p50={:.1} p99={:.1}",
                q(0.01),
                q(0.5),
                q(0.99)
            );
        }
        assert!(depths.len() > 15_000);
    }
}
