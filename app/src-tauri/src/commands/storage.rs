//! SSD（0x0400/0x0401/0x0402/0x0403/0x0406）与 GPS 文件（0x0110）操作。

use super::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::ipc::Channel;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// 解析名称扇区：前 32 字节为文件名（不足用 0x00 填充，其余用 0xee 填充）。
/// 全为填充字节或非可打印字符时视为空条目。
fn parse_ssd_name(sector: &[u8]) -> String {
    let raw = &sector[..32.min(sector.len())];
    let end = raw.iter().position(|&b| b == 0x00).unwrap_or(raw.len());
    let name_bytes = &raw[..end];
    if name_bytes.is_empty() || !name_bytes.iter().all(|&b| (0x20..0x7f).contains(&b)) {
        return String::new();
    }
    String::from_utf8_lossy(name_bytes).trim().to_string()
}

/// 读取指定扇区（0x0400），返回该扇区 512 字节原始数据。
/// 通过共享 socket 发送，避免设备把状态推送目标切换到其他端口。
async fn read_ssd_sector(
    network: &Arc<UdpNetwork>,
    target: &SocketAddr,
    rx: &mut broadcast::Receiver<(Frame, SocketAddr)>,
    sector: u64,
    seq: u16,
) -> Result<Vec<u8>, String> {
    let mut frame = Frame::new(
        0x0400,
        SourceAddr::PC as u16,
        DestAddr::PS as u16,
        sector.to_be_bytes().to_vec(),
    );
    frame.seq = seq;

    for attempt in 0..3 {
        network.send_to(target, &frame).await?;

        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_millis(800);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok((f, addr))) => {
                    if f.cmd == 0x0400
                        && addr.ip() == target.ip()
                        && f.data.len() == SSD_SECTOR_SIZE as usize
                    {
                        return Ok(f.data);
                    }
                    // 扫描期间收到的其他帧（状态/预览等）直接丢弃
                }
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    return Err("帧通道已关闭".into())
                }
                Err(_) => break,
            }
        }

        if attempt == 2 {
            return Err(format!("读取扇区 {} 超时", sector));
        }
    }

    Err(format!("读取扇区 {} 失败", sector))
}

/// 构造文件名扇区：前 32 字节为 ASCII 文件名（不足补 0x00），其余 480 字节补 0xee
fn build_ssd_name_sector(name: &str) -> Result<Vec<u8>, String> {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return Err("文件名不能为空".into());
    }
    if bytes.len() > 32 {
        return Err("文件名不能超过 32 字节".into());
    }
    if !bytes.iter().all(|&b| (0x20..=0x7e).contains(&b)) {
        return Err("文件名仅支持 ASCII 可打印字符".into());
    }

    let mut sector = vec![0xeeu8; SSD_SECTOR_SIZE as usize];
    sector[..32].fill(0);
    sector[..bytes.len()].copy_from_slice(bytes);
    Ok(sector)
}

/// 发送一条 SSD 操作命令
async fn send_ssd_command(
    network: &Arc<UdpNetwork>,
    target: &SocketAddr,
    cmd: u16,
    data: Vec<u8>,
    seq: u16,
) -> Result<(), String> {
    let mut frame = Frame::new(cmd, SourceAddr::PC as u16, DestAddr::PS as u16, data);
    frame.seq = seq;
    network.send_to(target, &frame).await
}

/// SSD 文件表扫描结果
struct SsdDirectory {
    /// 有效文件
    files: Vec<SsdFile>,
    /// 第一个空闲目录槽位（文件名扇区为空）
    first_free: Option<usize>,
    /// 已有文件数据区的最大结束地址
    max_end: u64,
}

/// 扫描 SSD 文件表（0x0400），返回有效文件、空闲槽位与数据区末尾
async fn scan_ssd_directory(
    network: &Arc<UdpNetwork>,
    target: &SocketAddr,
    rx: &mut broadcast::Receiver<(Frame, SocketAddr)>,
) -> Result<SsdDirectory, String> {
    let mut files: Vec<SsdFile> = Vec::new();
    let mut first_free: Option<usize> = None;
    let mut empty_run = 0usize;
    let mut max_end = SSD_DATA_START;
    let mut seq: u16 = 0;

    for i in 0..SSD_MAX_FILES {
        // 文件名在偶数扇区 2i，起止地址在奇数扇区 2i+1
        let name_sector = read_ssd_sector(network, target, rx, (2 * i) as u64, seq).await?;
        seq = seq.wrapping_add(1);

        let name = parse_ssd_name(&name_sector);
        if name.is_empty() {
            if first_free.is_none() {
                first_free = Some(i);
            }
            empty_run += 1;
            if empty_run >= SSD_EMPTY_STOP {
                break;
            }
            continue;
        }
        empty_run = 0;

        let addr_sector =
            read_ssd_sector(network, target, rx, (2 * i + 1) as u64, seq).await?;
        seq = seq.wrapping_add(1);

        let start = u64::from_be_bytes(addr_sector[0..8].try_into().unwrap());
        let end = u64::from_be_bytes(addr_sector[8..16].try_into().unwrap());

        // 地址合法才视为有效文件（数据区从 8192 扇区开始，结束地址须大于起始地址）
        if start >= SSD_DATA_START && end > start {
            max_end = max_end.max(end);
            files.push(SsdFile {
                name,
                start,
                end,
                size: (end - start) * SSD_SECTOR_SIZE,
            });
        } else if start >= SSD_DATA_START {
            // 已创建但尚未写入结束地址的文件：避免新文件覆盖其数据区
            max_end = max_end.max(start);
        }
    }

    Ok(SsdDirectory {
        files,
        first_free,
        max_end,
    })
}

/// 检索 SSD 已存储文件：逐条目读取文件表（0x0400），返回文件名/起始/结束/大小
#[tauri::command]
pub async fn list_ssd_files(app: AppHandle, device_ip: String) -> Result<Vec<SsdFile>, String> {
    let _ssd = acquire_ssd_lock(&app).await;
    let network = get_or_init_network(&app).await?;

    let config = app.state::<AppConfig>();
    let target: SocketAddr = format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))?;

    // 先订阅广播通道，避免设备响应早于订阅而丢失
    let mut rx = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.frame_tx.subscribe()
    };

    let dir = scan_ssd_directory(&network, &target, &mut rx).await?;
    log::info!("Listed {} SSD file(s) from {}", dir.files.len(), device_ip);
    Ok(dir.files)
}

/// 开始/停止 SSD 存储。
///
/// 开始存储需要按协议顺序执行：
///   0x0402(文件名扇区) → 0x0403(文件名) → 0x0402(数据起始 LBA) → 0x0406(开始)
/// 仅发送 0x0406 时设备没有文件名和写入地址，文件表不会产生记录。
#[tauri::command]
pub async fn set_ssd_store(
    app: AppHandle,
    device_ip: String,
    enable: bool,
    filename: Option<String>,
) -> Result<(), String> {
    let _ssd = acquire_ssd_lock(&app).await;
    let network = get_or_init_network(&app).await?;

    let config = app.state::<AppConfig>();
    let target: SocketAddr = format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))?;

    let mut seq = next_seq_inner(&app).await;

    if !enable {
        send_ssd_command(&network, &target, 0x0406, 0u32.to_be_bytes().to_vec(), seq).await?;
        log::info!("Stop SSD store on {}", device_ip);
        return Ok(());
    }

    let name = filename.unwrap_or_default();
    let name_sector = build_ssd_name_sector(&name)?;

    // 先订阅广播通道，扫描文件表时才能收到 0x0400 响应
    let mut rx = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.frame_tx.subscribe()
    };

    let dir = scan_ssd_directory(&network, &target, &mut rx).await?;
    let slot = dir
        .first_free
        .ok_or_else(|| "SSD 文件表已满（最多 1024 个文件）".to_string())?;

    // 1) 设置文件名存放扇区
    send_ssd_command(
        &network,
        &target,
        0x0402,
        ((2 * slot) as u64).to_be_bytes().to_vec(),
        seq,
    )
    .await?;
    seq = seq.wrapping_add(1);

    // 2) 写入文件名扇区
    send_ssd_command(&network, &target, 0x0403, name_sector, seq).await?;
    seq = seq.wrapping_add(1);

    // 3) 设置采集数据起始扇区
    send_ssd_command(&network, &target, 0x0402, dir.max_end.to_be_bytes().to_vec(), seq).await?;
    seq = seq.wrapping_add(1);

    // 4) 开始存储
    send_ssd_command(&network, &target, 0x0406, 1u32.to_be_bytes().to_vec(), seq).await?;

    log::info!(
        "Start SSD store '{}' (slot={}, data_lba={}) on {}",
        name,
        slot,
        dir.max_end,
        device_ip
    );
    Ok(())
}

/// 系统指令（0x0110）收到最后一个包后的静默判定窗口
const SYS_CMD_QUIET: std::time::Duration = std::time::Duration::from_millis(350);
/// 系统指令（0x0110）最长等待时间
const SYS_CMD_TOTAL: std::time::Duration = std::time::Duration::from_millis(3000);

/// 通过 0x0110 执行设备端系统命令，返回逐行输出。
///
/// 设备把输出的每一行单独发一个 UDP 包（每包 `总包数` 均为 1），
/// 因此需要按“静默窗口”持续收集，直到一段时间不再收到新包。
async fn system_cmd_lines(
    network: &Arc<UdpNetwork>,
    target: &SocketAddr,
    rx: &mut broadcast::Receiver<(Frame, SocketAddr)>,
    command: &str,
    seq: u16,
) -> Result<Vec<String>, String> {
    let mut frame = Frame::new(
        0x0110,
        SourceAddr::PC as u16,
        DestAddr::PS as u16,
        command.as_bytes().to_vec(),
    );
    frame.seq = seq;

    network.send_to(target, &frame).await?;

    let mut lines: Vec<String> = Vec::new();
    let mut started = false;
    let deadline = tokio::time::Instant::now() + SYS_CMD_TOTAL;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        // 首包最多等到总超时；已开始接收后，静默 SYS_CMD_QUIET 即认为结束
        let wait = if started { remaining.min(SYS_CMD_QUIET) } else { remaining };

        match tokio::time::timeout(wait, rx.recv()).await {
            Ok(Ok((f, addr))) => {
                if f.cmd == 0x0110 && addr.ip() == target.ip() {
                    started = true;
                    for line in String::from_utf8_lossy(&f.data).lines() {
                        let line = line.trim().to_string();
                        if !line.is_empty() {
                            lines.push(line);
                        }
                    }
                }
                // 其他帧（状态推送等）忽略
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                return Err("帧通道已关闭".into())
            }
            Err(_) => break, // 静默窗口内无新包，响应结束
        }
    }

    Ok(lines)
}

/// GPS 目录不存在时，由设备 shell 返回的标记
const GPS_NO_SD_MARKER: &str = "__UAV_GPS_NO_SD__";

/// 单引号包裹 shell 参数，内部的单引号按 `'\''` 转义
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// 构造 GPS 目录检索命令：目录存在时执行 `ls`，否则输出无 SD 卡标记
fn build_gps_list_command(dir: &str) -> String {
    let dir = shell_quote(dir);
    format!(
        "if [ -d {dir} ]; then ls {dir}; else echo {marker}; fi",
        dir = dir,
        marker = GPS_NO_SD_MARKER
    )
}

/// 解析 `ls` 输出：识别无 SD 卡标记，并过滤设备返回的错误行
fn parse_gps_files(lines: Vec<String>) -> Result<Vec<String>, String> {
    if lines.iter().any(|line| line == GPS_NO_SD_MARKER) {
        return Err("设备未插入 SD 卡，请插入 SD 卡后重试".into());
    }
    if let Some(err) = lines.iter().find(|line| line.starts_with("ls:")) {
        return Err(format!("读取 GPS 目录失败：{}", err));
    }
    Ok(lines
        .into_iter()
        .filter(|line| line != GPS_NO_SD_MARKER)
        .collect())
}

/// GPS 数据文件列表：使用 0x0110 执行 `ls <gps.dir>`
#[tauri::command]
pub async fn query_gps_files(app: AppHandle, device_ip: String) -> Result<Vec<String>, String> {
    let _op = acquire_op_lock(&app).await;
    let network = get_or_init_network(&app).await?;

    let config = app.state::<AppConfig>();
    let target: SocketAddr = format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))?;
    let command = build_gps_list_command(&config.gps.dir);

    // 先订阅广播通道，避免设备响应早于订阅而丢失
    let mut rx = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.frame_tx.subscribe()
    };

    let seq = next_seq_inner(&app).await;
    let lines = system_cmd_lines(&network, &target, &mut rx, &command, seq).await?;
    let files = parse_gps_files(lines)?;

    log::info!(
        "GPS file query '{}' from {}: {} entr(ies)",
        command,
        device_ip,
        files.len()
    );
    Ok(files)
}

// ========== SSD 文件 TCP 下载（0x0401） ==========

/// 单块 TCP 读取超时
const SSD_TCP_READ_TIMEOUT: Duration = Duration::from_secs(10);
/// TCP 连接后等待设备 accept 的时间
const SSD_TCP_ACCEPT_WAIT: Duration = Duration::from_millis(300);
/// 取消标记：只在块边界检查，避免设备正在 write 时强断 TCP
static SSD_DOWNLOAD_CANCEL: AtomicBool = AtomicBool::new(false);

/// SSD 下载进度
#[derive(Clone, serde::Serialize)]
pub struct SsdDownloadProgress {
    pub transferred: u64,
    pub total: u64,
    pub percent: f64,
    /// 实时速度（字节/秒）
    pub speed_bps: f64,
    /// 当前分块大小（MB）
    pub chunk_mb: u32,
}

/// 清理文件名中的路径分隔符和 Windows 非法字符
pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = sanitized.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "ssd_file".to_string()
    } else {
        trimmed
    }
}

/// 配置的分块大小（MB）转换为扇区数；仅支持 1/2/4 MB
fn chunk_sectors_from_mb(chunk_mb: u32) -> Result<u64, String> {
    match chunk_mb {
        1 => Ok(2048),
        2 => Ok(4096),
        4 => Ok(8192),
        other => Err(format!(
            "config.toml [ssd].chunk_mb 只能设置为 1、2 或 4，当前为 {}",
            other
        )),
    }
}

/// 目标目录下生成不重名的文件路径
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("ssd_file");
    let ext = path.extension().and_then(|s| s.to_str());
    for i in 1..10_000 {
        let file = match ext {
            Some(ext) => format!("{}_{}.{}", stem, i, ext),
            None => format!("{}_{}", stem, i),
        };
        let candidate = dir.join(file);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{}_{}", stem, std::process::id()))
}

/// 建立 TCP 连接，并等待设备完成 accept
async fn connect_tcp(ip: std::net::IpAddr, port: u16) -> Result<TcpStream, String> {
    let stream = TcpStream::connect((ip, port))
        .await
        .map_err(|e| format!("TCP 连接 {}:{} 失败: {}", ip, port, e))?;
    let _ = stream.set_nodelay(true);
    tokio::time::sleep(SSD_TCP_ACCEPT_WAIT).await;
    Ok(stream)
}

/// 执行下载：按配置的分块大小发送 0x0401，并逐块读取 TCP 数据
async fn download_ssd_file_inner(
    app: &AppHandle,
    network: &Arc<UdpNetwork>,
    target: SocketAddr,
    start_lba: u64,
    total_sectors: u64,
    total_bytes: u64,
    chunk_sectors: u64,
    part_path: &Path,
    final_path: &Path,
    channel: &Channel<SsdDownloadProgress>,
) -> Result<String, String> {
    let mut file = tokio::fs::File::create(part_path)
        .await
        .map_err(|e| format!("创建临时文件失败（{}）: {}", part_path.display(), e))?;

    let mut stream: Option<TcpStream> = None;
    let mut transferred: u64 = 0;
    let mut speed_bps: f64 = 0.0;
    let mut cancelled = false;
    let chunk_mb = (chunk_sectors / 2048) as u32;

    channel
        .send(SsdDownloadProgress {
            transferred: 0,
            total: total_bytes,
            percent: 0.0,
            speed_bps: 0.0,
            chunk_mb,
        })
        .map_err(|e| format!("发送下载进度失败: {}", e))?;

    while transferred < total_bytes {
        if SSD_DOWNLOAD_CANCEL.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }

        let done_sectors = transferred / SSD_SECTOR_SIZE;
        let remaining = total_sectors.saturating_sub(done_sectors);
        if remaining == 0 {
            break;
        }
        let sectors = remaining.min(chunk_sectors);
        let chunk_bytes = (sectors * SSD_SECTOR_SIZE) as usize;
        let mut chunk_start = std::time::Instant::now();
        let mut buf = vec![0u8; chunk_bytes];
        let mut attempt = 0u32;

        loop {
            attempt += 1;
            if stream.is_none() {
                stream = Some(connect_tcp(target.ip(), target.port()).await?);
            }
            // 只统计实际传输时间，不把 TCP accept 等待和重试时间算进速度
            chunk_start = std::time::Instant::now();

            let mut data = Vec::with_capacity(19);
            data.extend_from_slice(&(start_lba + done_sectors).to_be_bytes());
            data.extend_from_slice(&sectors.to_be_bytes());
            // 单次 DMA 扇区数保持文档默认的 2048（1MB），大块由 SATA 核分多次搬运
            data.extend_from_slice(&(sectors.min(2048) as u16).to_be_bytes());
            data.push(0x01); // TCP 返回
            send_frame(app, network, target, 0x0401, data).await?;

            let result = tokio::time::timeout(
                SSD_TCP_READ_TIMEOUT,
                stream.as_mut().unwrap().read_exact(&mut buf),
            )
            .await;

            match result {
                Ok(Ok(_)) => break,
                Ok(Err(e)) => log::warn!(
                    "读取 SSD 扇区 {} 失败（第 {} 次）: {}",
                    start_lba + done_sectors,
                    attempt,
                    e
                ),
                Err(_) => log::warn!(
                    "读取 SSD 扇区 {} 超时（第 {} 次）",
                    start_lba + done_sectors,
                    attempt
                ),
            }

            // 连接上可能残留半包数据，重连后从同一扇区重读
            stream = None;
            if attempt >= 3 {
                return Err(format!(
                    "读取扇区 {} 失败（已重试 {} 次）",
                    start_lba + done_sectors,
                    attempt
                ));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        file.write_all(&buf)
            .await
            .map_err(|e| format!("写入文件失败: {}", e))?;
        transferred += chunk_bytes as u64;

        // 当前块速度做指数平滑，避免进度条数值跳动过大
        let elapsed = chunk_start.elapsed().as_secs_f64();
        let instant_speed = if elapsed > 0.0 {
            chunk_bytes as f64 / elapsed
        } else {
            0.0
        };
        speed_bps = if speed_bps > 0.0 {
            speed_bps * 0.7 + instant_speed * 0.3
        } else {
            instant_speed
        };

        let percent = transferred as f64 / total_bytes as f64 * 100.0;
        channel
            .send(SsdDownloadProgress {
                transferred,
                total: total_bytes,
                percent,
                speed_bps,
                chunk_mb,
            })
            .map_err(|e| format!("发送下载进度失败: {}", e))?;
    }

    file.flush()
        .await
        .map_err(|e| format!("刷新文件失败: {}", e))?;
    drop(file);

    if cancelled {
        return Err("下载已取消".into());
    }

    tokio::fs::rename(part_path, final_path)
        .await
        .map_err(|e| format!("重命名下载文件失败: {}", e))?;
    Ok(final_path.to_string_lossy().to_string())
}

/// 通过 TCP 下载 SSD 文件（分块大小由 config.toml [ssd].chunk_mb 指定）
#[tauri::command]
pub async fn download_ssd_file(
    app: AppHandle,
    device_ip: String,
    start_lba: u64,
    total_sectors: u64,
    filename: String,
    local_dir: String,
    channel: Channel<SsdDownloadProgress>,
) -> Result<String, String> {
    let _download = try_acquire_download_lock(&app).await?;
    let _ssd = try_acquire_ssd_lock(&app).await?;

    // 分块大小由 config.toml 的 [ssd].chunk_mb 指定，仅支持 1/2/4 MB
    let chunk_mb = app.state::<AppConfig>().ssd.chunk_mb;
    let chunk_sectors = chunk_sectors_from_mb(chunk_mb)?;

    if total_sectors == 0 {
        return Err("文件大小为 0，无法下载".into());
    }
    let total_bytes = total_sectors
        .checked_mul(SSD_SECTOR_SIZE)
        .ok_or_else(|| "文件大小溢出".to_string())?;
    if start_lba < SSD_DATA_START {
        return Err("文件起始扇区不在 SSD 数据区".into());
    }
    let end_lba = start_lba
        .checked_add(total_sectors)
        .ok_or_else(|| "文件扇区范围溢出".to_string())?;

    // 下载前检查最近一次状态：存储中或 SATA 未连接时拒绝
    let status = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.last_status.clone()
    };
    let Some(st) = status.as_ref() else {
        return Err("未获取到设备状态，请先开启设备状态读取".into());
    };
    if st.ssd_store_enable == 1 {
        return Err("设备正在存储，请先停止存储后再下载".into());
    }
    if st.sata_dev_identify_done != 1 {
        return Err("SATA 未连接，无法下载".into());
    }
    if st.sata_dev_tot_sec_num > 0 && end_lba > st.sata_dev_tot_sec_num {
        return Err("文件范围超出硬盘容量".into());
    }

    let dir = Path::new(&local_dir);
    if !dir.is_dir() {
        return Err(format!("保存目录不存在: {}", local_dir));
    }

    let safe_name = sanitize_filename(&filename);
    let part_path = dir.join(format!("{}.part", safe_name));
    let final_path = unique_path(dir, &safe_name);

    let port = app.state::<AppConfig>().basic.remote_port;
    let ip: std::net::IpAddr = device_ip
        .parse()
        .map_err(|e| format!("设备 IP 非法 {}: {}", device_ip, e))?;
    let target = SocketAddr::new(ip, port);
    let network = get_or_init_network(&app).await?;

    // 必须在获取到 SSD 锁之后再重置，避免取消正在运行的其他下载
    SSD_DOWNLOAD_CANCEL.store(false, Ordering::SeqCst);

    let result = download_ssd_file_inner(
        &app,
        &network,
        target,
        start_lba,
        total_sectors,
        total_bytes,
        chunk_sectors,
        &part_path,
        &final_path,
        &channel,
    )
    .await;

    // 取消或失败时删除临时文件，不保留断点
    if result.is_err() {
        let _ = tokio::fs::remove_file(&part_path).await;
    }
    result
}

/// 取消当前 SSD 下载：在下一个 1MB 块边界生效
#[tauri::command]
pub fn cancel_ssd_download() {
    SSD_DOWNLOAD_CANCEL.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::{
        build_gps_list_command, build_ssd_name_sector, chunk_sectors_from_mb, parse_gps_files,
        parse_ssd_name, sanitize_filename, shell_quote, GPS_NO_SD_MARKER,
    };

    #[test]
    fn test_parse_name_normal() {
        let mut sec = vec![0u8; 512];
        sec[..19].copy_from_slice(b"2026-07-14 14-04-10");
        for b in &mut sec[32..] {
            *b = 0xee;
        }
        assert_eq!(parse_ssd_name(&sec), "2026-07-14 14-04-10");
    }

    #[test]
    fn test_parse_name_erased() {
        assert_eq!(parse_ssd_name(&vec![0xeeu8; 512]), "");
        assert_eq!(parse_ssd_name(&vec![0xddu8; 512]), "");
        assert_eq!(parse_ssd_name(&vec![0x00u8; 512]), "");
    }

    #[test]
    fn test_build_name_sector() {
        let sec = build_ssd_name_sector("uav_20261023_120000").unwrap();
        assert_eq!(sec.len(), 512);
        assert_eq!(&sec[..19], b"uav_20261023_120000");
        assert_eq!(sec[19], 0x00);
        assert_eq!(sec[31], 0x00);
        assert_eq!(sec[32], 0xee);
        assert_eq!(sec[511], 0xee);
        assert_eq!(parse_ssd_name(&sec), "uav_20261023_120000");

        assert!(build_ssd_name_sector("").is_err());
        assert!(build_ssd_name_sector("文件名").is_err());
        assert!(build_ssd_name_sector(&"a".repeat(33)).is_err());
    }

    #[test]
    fn test_shell_quote() {
        assert_eq!(shell_quote("/run/media/mmcblk1p1/"), "'/run/media/mmcblk1p1/'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn test_build_gps_list_command() {
        let cmd = build_gps_list_command("/run/media/mmcblk1p1/");
        assert_eq!(
            cmd,
            format!(
                "if [ -d '/run/media/mmcblk1p1/' ]; then ls '/run/media/mmcblk1p1/'; else echo {}; fi",
                GPS_NO_SD_MARKER
            )
        );
    }

    #[test]
    fn test_parse_gps_files_no_sd() {
        let err = parse_gps_files(vec![GPS_NO_SD_MARKER.to_string()]).unwrap_err();
        assert!(err.contains("SD 卡"), "unexpected error: {}", err);
    }

    #[test]
    fn test_parse_gps_files_normal() {
        let files = parse_gps_files(vec![
            "2026-05-13_1.gps".to_string(),
            "2026-05-13_2.gps".to_string(),
        ])
        .unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_parse_gps_files_ls_error() {
        let err = parse_gps_files(vec![
            "ls: cannot access '/run/media/mmcblk1p1/': No such file or directory".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("读取 GPS 目录失败"));
    }

    #[test]
    fn test_chunk_sectors_from_mb() {
        assert_eq!(chunk_sectors_from_mb(1).unwrap(), 2048);
        assert_eq!(chunk_sectors_from_mb(2).unwrap(), 4096);
        assert_eq!(chunk_sectors_from_mb(4).unwrap(), 8192);
        assert!(chunk_sectors_from_mb(3).is_err());
        assert!(chunk_sectors_from_mb(0).is_err());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("uav_20261023_085416"), "uav_20261023_085416");
        assert_eq!(sanitize_filename("a/b:c*?\"<>|"), "a_b_c______");
        assert_eq!(sanitize_filename(".."), "ssd_file");
        assert_eq!(sanitize_filename("   "), "ssd_file");
    }
}
