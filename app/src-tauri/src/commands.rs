use crate::config::AppConfig;
use crate::network::UdpNetwork;
use crate::protocol::{Frame, SourceAddr, DestAddr};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{broadcast, Mutex};

/// 发现的设备信息
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredDevice {
    pub ip: String,
    pub port: u16,
}

/// 解析设备发现响应中的 IP：优先用数据区里的 IP 字符串；
/// 若为空或非法（部分固件 0x0100 响应不带数据）则回退到报文源 IP。
fn resolve_device_ip(data: &[u8], src: &SocketAddr) -> String {
    let from_data = String::from_utf8_lossy(data)
        .trim_end_matches('\0')
        .trim()
        .to_string();
    if from_data.parse::<std::net::IpAddr>().is_ok() {
        from_data
    } else {
        src.ip().to_string()
    }
}

/// SSD 已存储文件信息
#[derive(Debug, Clone, Serialize)]
pub struct SsdFile {
    /// 文件名（最长 32 字节）
    pub name: String,
    /// 起始地址（单位：扇区，512 字节）
    pub start: u64,
    /// 结束地址（单位：扇区，512 字节）
    pub end: u64,
    /// 文件大小（字节）
    pub size: u64,
}

/// SSD 扇区大小（字节）
const SSD_SECTOR_SIZE: u64 = 512;
/// 文件表最大条目数（fs.md：扇区 0~2047，每个文件占 2 个扇区）
const SSD_MAX_FILES: usize = 1024;
/// 连续多少个空条目后认为文件表结束
const SSD_EMPTY_STOP: usize = 16;
/// 数据区起始扇区
const SSD_DATA_START: u64 = 8192;

/// 共享的网络状态
pub struct NetworkState {
    pub network: Option<Arc<UdpNetwork>>,
    pub connected_device: Option<SocketAddr>,
    pub seq_counter: u16,
    pub preview_enabled: bool,
    /// 广播通道：后台监听器将所有收到的帧转发至此，供命令订阅
    pub frame_tx: broadcast::Sender<(Frame, SocketAddr)>,
}

/// 获取或初始化 NetworkState
async fn get_or_init_network(app: &AppHandle) -> Result<Arc<UdpNetwork>, String> {
    let config = app.state::<AppConfig>();
    let state = app.state::<Arc<Mutex<NetworkState>>>();

    let mut guard = state.lock().await;
    if guard.network.is_none() {
        let network = UdpNetwork::bind(&config.basic.local_ip, config.basic.local_port).await?;
        guard.network = Some(Arc::new(network));
    }
    Ok(guard.network.as_ref().unwrap().clone())
}

// ========== Tauri Commands ==========

/// 发现局域网内设备：发送 0x0100 广播，100ms 内收集响应设备
#[tauri::command]
pub async fn discover_devices(app: AppHandle) -> Result<Vec<DiscoveredDevice>, String> {
    let network = get_or_init_network(&app).await?;
    let state = app.state::<Arc<Mutex<NetworkState>>>();

    // 先订阅广播通道（后台监听器转发所有收到的帧），避免设备响应早于订阅而丢失
    let mut rx = {
        let guard = state.lock().await;
        guard.frame_tx.subscribe()
    };

    // 构建并发送广播帧
    {
        let mut guard = state.lock().await;
        let seq = guard.seq_counter;
        guard.seq_counter = guard.seq_counter.wrapping_add(1);

        let config = app.state::<AppConfig>();

        let mut frame = Frame::new(
            0x0100,
            SourceAddr::PC as u16,
            DestAddr::PS as u16,
            vec![0u8; 4],
        );
        frame.seq = seq;

        network.send_broadcast(&frame, config.basic.remote_port).await?;
        log::info!("Sent broadcast for device discovery");
    }

    // 收集响应（300ms 窗口，覆盖后台监听器首次启动的等待时间）
    let mut devices: Vec<DiscoveredDevice> = Vec::new();
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_millis(300);

    loop {
        let timeout = deadline.saturating_duration_since(tokio::time::Instant::now());
        if timeout.is_zero() {
            break;
        }

        match tokio::time::timeout(timeout, rx.recv()).await {
            Ok(Ok((frame, addr))) => {
                if frame.cmd == 0x0100 {
                    let ip_str = resolve_device_ip(&frame.data, &addr);
                    if !devices.iter().any(|d| d.ip == ip_str) {
                        devices.push(DiscoveredDevice {
                            ip: ip_str,
                            port: addr.port(),
                        });
                    }
                }
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => break,
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Err(_) => break,
        }
    }

    let _ = app.emit("devices-found", devices.clone());
    Ok(devices)
}

/// 启动/停止预览数据采集
/// enable=true: 发送 0x0300(0x00000001) + 启动 TCP 监听
/// enable=false: 发送 0x0300(0x00000000) + 停止 TCP 监听
#[tauri::command]
pub async fn start_preview(
    app: AppHandle,
    device_ip: String,
    enable: bool,
) -> Result<(), String> {
    let config = app.state::<AppConfig>();
    let network = get_or_init_network(&app).await?;
    let target: SocketAddr = format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))?;

    let seq = next_seq_inner(&app).await;

    let enable_val: u32 = if enable { 1 } else { 0 };
    let data = enable_val.to_be_bytes().to_vec();

    let mut frame = Frame::new(0x0300, SourceAddr::PC as u16, DestAddr::PS as u16, data);
    frame.seq = seq;

    network.send_to(&target, &frame).await?;

    // 更新 TCP 预览状态
    {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let mut guard = state.lock().await;
        guard.preview_enabled = enable;
    }

    let _ = app.emit("preview-state-changed", enable);
    log::info!("Preview {} for {}", if enable { "enabled" } else { "disabled" }, device_ip);
    Ok(())
}

/// 内部获取帧序号
async fn next_seq_inner(app: &AppHandle) -> u16 {
    let state = app.state::<Arc<Mutex<NetworkState>>>();
    let mut guard = state.lock().await;
    let seq = guard.seq_counter;
    guard.seq_counter = guard.seq_counter.wrapping_add(1);
    seq
}

/// 查询系统状态
#[tauri::command]
pub async fn query_sys_status(
    app: AppHandle,
    device_ip: String,
    enable: bool,
) -> Result<(), String> {
    let network = get_or_init_network(&app).await?;

    let config = app.state::<AppConfig>();
    let target: SocketAddr = format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))?;

    let seq = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let mut guard = state.lock().await;
        let seq = guard.seq_counter;
        guard.seq_counter = guard.seq_counter.wrapping_add(1);
        if enable {
            guard.connected_device = Some(target);
        }
        seq
    };

    let enable_val: u32 = if enable { 1 } else { 0 };
    let data = enable_val.to_be_bytes().to_vec();

    let mut frame = Frame::new(0x0102, SourceAddr::PC as u16, DestAddr::PS as u16, data);
    frame.seq = seq;

    network.send_to(&target, &frame).await?;
    log::info!("Sent 0x0102 to {} (enable={})", device_ip, enable);
    Ok(())
}

/// 下发参考通道（0x0302），值取自 config.toml 的 [capture].ref_channal，界面不显示
#[tauri::command]
pub async fn send_ref_channel(app: AppHandle, device_ip: String) -> Result<(), String> {
    let network = get_or_init_network(&app).await?;

    let config = app.state::<AppConfig>();
    let target: SocketAddr = format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))?;

    let seq = next_seq_inner(&app).await;
    let ref_channal = config.capture.ref_channal;

    let mut frame = Frame::new(
        0x0302,
        SourceAddr::PC as u16,
        DestAddr::PS as u16,
        ref_channal.to_be_bytes().to_vec(),
    );
    frame.seq = seq;

    network.send_to(&target, &frame).await?;
    log::info!("Sent ref_channel={} (from config) to {}", ref_channal, device_ip);
    Ok(())
}

/// 通用命令发送接口
#[tauri::command]
pub async fn send_command(
    app: AppHandle,
    device_ip: String,
    cmd: u16,
    data: Vec<u8>,
) -> Result<(), String> {
    let network = get_or_init_network(&app).await?;

    let config = app.state::<AppConfig>();
    let target: SocketAddr = format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))?;

    let seq = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let mut guard = state.lock().await;
        let seq = guard.seq_counter;
        guard.seq_counter = guard.seq_counter.wrapping_add(1);
        seq
    };

    let mut frame = Frame::new(cmd, SourceAddr::PC as u16, DestAddr::PS as u16, data);
    frame.seq = seq;

    network.send_to(&target, &frame).await?;
    log::info!("Sent command 0x{:04x} to {}", cmd, device_ip);
    Ok(())
}

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

/// 检索 SSD 已存储文件：逐条目读取文件表（0x0400），返回文件名/起始/结束/大小
#[tauri::command]
pub async fn list_ssd_files(app: AppHandle, device_ip: String) -> Result<Vec<SsdFile>, String> {
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

    let mut files: Vec<SsdFile> = Vec::new();
    let mut empty_run = 0usize;
    let mut seq: u16 = 0;

    for i in 0..SSD_MAX_FILES {
        // 文件名在偶数扇区 2i，起止地址在奇数扇区 2i+1
        let name_sector = read_ssd_sector(&network, &target, &mut rx, (2 * i) as u64, seq).await?;
        seq = seq.wrapping_add(1);

        let name = parse_ssd_name(&name_sector);
        if name.is_empty() {
            empty_run += 1;
            if empty_run >= SSD_EMPTY_STOP {
                break;
            }
            continue;
        }
        empty_run = 0;

        let addr_sector =
            read_ssd_sector(&network, &target, &mut rx, (2 * i + 1) as u64, seq).await?;
        seq = seq.wrapping_add(1);

        let start = u64::from_be_bytes(addr_sector[0..8].try_into().unwrap());
        let end = u64::from_be_bytes(addr_sector[8..16].try_into().unwrap());

        // 地址合法才视为有效文件（数据区从 8192 扇区开始，结束地址须大于起始地址）
        if start >= SSD_DATA_START && end > start {
            files.push(SsdFile {
                name,
                start,
                end,
                size: (end - start) * SSD_SECTOR_SIZE,
            });
        }
    }

    log::info!("Listed {} SSD file(s) from {}", files.len(), device_ip);
    Ok(files)
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

/// GPS 数据文件列表：使用 0x0110 执行 `ls <gps.dir>`
#[tauri::command]
pub async fn query_gps_files(app: AppHandle, device_ip: String) -> Result<Vec<String>, String> {
    let network = get_or_init_network(&app).await?;

    let config = app.state::<AppConfig>();
    let target: SocketAddr = format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))?;
    let command = format!("ls {}", config.gps.dir);

    // 先订阅广播通道，避免设备响应早于订阅而丢失
    let mut rx = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.frame_tx.subscribe()
    };

    let seq = next_seq_inner(&app).await;
    let files = system_cmd_lines(&network, &target, &mut rx, &command, seq).await?;

    log::info!(
        "GPS file query '{}' from {}: {} entr(ies)",
        command,
        device_ip,
        files.len()
    );
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::{parse_ssd_name, resolve_device_ip};
    use std::net::SocketAddr;

    #[test]
    fn test_resolve_device_ip_fallback_to_source() {
        let addr: SocketAddr = "192.168.1.10:6666".parse().unwrap();
        // 数据区为空 / 全 0：回退到源 IP（实测该固件 0x0100 响应不带数据）
        assert_eq!(resolve_device_ip(b"", &addr), "192.168.1.10");
        assert_eq!(resolve_device_ip(b"\0\0\0\0", &addr), "192.168.1.10");
        // 数据区带合法 IP：优先使用
        assert_eq!(resolve_device_ip(b"192.168.1.20\0", &addr), "192.168.1.20");
    }

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
        // 全 0xee（已擦除）与全 0xdd 都应视为空条目
        assert_eq!(parse_ssd_name(&vec![0xeeu8; 512]), "");
        assert_eq!(parse_ssd_name(&vec![0xddu8; 512]), "");
        assert_eq!(parse_ssd_name(&vec![0x00u8; 512]), "");
    }
}
