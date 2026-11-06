use crate::config::AppConfig;
use crate::laser::{self, LaserStatus};
use crate::motor::{self, MotorState};
use crate::network::UdpNetwork;
use crate::protocol::{Frame, SourceAddr, DestAddr};
use serde::{Deserialize, Serialize};
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
pub struct NetworkState {    pub network: Option<Arc<UdpNetwork>>,
    pub connected_device: Option<SocketAddr>,
    pub seq_counter: u16,
    pub preview_enabled: bool,
    /// 预览数据推送通道（二进制 IPC）；未注册时丢弃预览帧
    pub preview_channel: Option<tauri::ipc::Channel<Vec<u8>>>,
    /// 广播通道：后台监听器将所有收到的帧转发至此，供命令订阅
    pub frame_tx: broadcast::Sender<(Frame, SocketAddr)>,
    /// 请求-响应类命令（激光器/电机透传）的串行化锁。
    /// 这些命令需要独占 UDP 应答通道，避免两个操作互相收到对方的响应。
    pub op_lock: Arc<tokio::sync::Mutex<()>>,
    /// 最近一次系统状态：SSD 下载前用于检查存储/SATA 状态
    pub last_status: Option<crate::status::SystemStatus>,
    /// SSD 相关操作（文件列表/存储/下载）的串行化锁
    pub ssd_lock: Arc<tokio::sync::Mutex<()>>,
    /// 文件下载锁：SSD 下载和 SD 卡下载不能同时进行
    pub download_lock: Arc<tokio::sync::Mutex<()>>,
}

/// 运行期可写的配置状态：保存 config.toml 后同步更新，
/// 使不依赖启动时绑定的配置项立即生效，无需重启上位机。
pub struct ConfigState(pub std::sync::RwLock<AppConfig>);

/// 获取当前配置的快照（克隆），避免跨 await 持有读锁
pub fn current_config(app: &AppHandle) -> AppConfig {
    app.state::<ConfigState>()
        .0
        .read()
        .expect("config lock poisoned")
        .clone()
}

/// 保存配置的结果
#[derive(Debug, Clone, Serialize)]
pub struct SaveConfigResult {
    /// true：本机 IP / 端口发生变化，需要重启上位机后重新绑定 socket
    pub restart_required: bool,
}

/// 是否需要重启上位机：只有本机绑定 IP / 端口这类启动期参数需要
fn config_restart_required(old: &AppConfig, new: &AppConfig) -> bool {
    old.basic.local_ip.trim() != new.basic.local_ip.trim()
        || old.basic.local_port != new.basic.local_port
}

/// 获取或初始化 NetworkState
async fn get_or_init_network(app: &AppHandle) -> Result<Arc<UdpNetwork>, String> {
    let config = current_config(app);
    let state = app.state::<Arc<Mutex<NetworkState>>>();

    let mut guard = state.lock().await;
    if guard.network.is_none() {
        let network = UdpNetwork::bind(&config.basic.local_ip, config.basic.local_port).await?;
        guard.network = Some(Arc::new(network));
    }
    Ok(guard.network.as_ref().unwrap().clone())
}

// ========== Tauri Commands ==========

/// 历史温度采样点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperaturePoint {
    /// 毫秒时间戳
    pub time: u64,
    /// 温度（℃）
    pub temp: f64,
}

/// 温度历史文件名（优先放在可执行文件目录）
const TEMP_HISTORY_FILE: &str = "temperature_history.json";

/// 温度历史完整路径只解析一次：可执行文件目录不可写时回退到应用数据目录
static TEMP_HISTORY_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// 通过实际创建/删除临时文件探测目录是否可写
fn dir_is_writable(dir: &std::path::Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".uav_pc_write_test");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 温度历史文件完整路径
fn temperature_history_path(app: &AppHandle) -> std::path::PathBuf {
    TEMP_HISTORY_PATH
        .get_or_init(|| {
            let exe_dir = crate::config::app_dir();
            let dir = if dir_is_writable(&exe_dir) {
                exe_dir
            } else {
                let fallback = app
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| crate::config::app_dir());
                log::warn!(
                    "Executable directory is not writable, using {} for temperature history",
                    fallback.display()
                );
                fallback
            };
            dir.join(TEMP_HISTORY_FILE)
        })
        .clone()
}

/// 读取历史温度点；文件不存在或损坏时返回空列表
#[tauri::command]
pub async fn load_temperature_history(app: AppHandle) -> Result<Vec<TemperaturePoint>, String> {
    let save_points = current_config(&app).temperature.save_points.max(1);
    let path = temperature_history_path(&app);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let mut points: Vec<TemperaturePoint> = serde_json::from_str(&text).unwrap_or_default();
    let start = points.len().saturating_sub(save_points);
    points.drain(..start);
    log::info!(
        "Loaded {} temperature history points from {}",
        points.len(),
        path.display()
    );
    Ok(points)
}

/// 保存历史温度点（按 save_points 裁剪后整体写入）
#[tauri::command]
pub async fn save_temperature_history(
    app: AppHandle,
    points: Vec<TemperaturePoint>,
) -> Result<(), String> {
    let save_points = current_config(&app).temperature.save_points.max(1);
    let start = points.len().saturating_sub(save_points);
    let kept = &points[start..];
    let text = serde_json::to_string(kept).map_err(|e| format!("序列化温度历史失败: {}", e))?;
    let path = temperature_history_path(&app);
    std::fs::write(&path, text)
        .map_err(|e| format!("写入温度历史失败（{}）: {}", path.display(), e))
}

/// 读取温度历史配置
#[tauri::command]
pub fn get_temperature_config(app: AppHandle) -> crate::config::TemperatureConfig {
    current_config(&app).temperature
}

/// 读取雷达显示名称、激光器类型和自定义信息
#[tauri::command]
pub fn get_radar_config(app: AppHandle) -> crate::config::RadarConfig {
    current_config(&app).radar
}

/// 读取完整配置（供设置界面展示）
#[tauri::command]
pub fn get_app_config(app: AppHandle) -> AppConfig {
    current_config(&app)
}

/// 上位机网络端点：连接异常时用于排查本机 IP 配置
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalEndpoint {
    /// config.toml 中配置的本机 IP
    pub configured_ip: String,
    /// config.toml 中配置的本机端口
    pub configured_port: u16,
    /// 实际绑定成功的本地地址（未初始化或绑定失败时为空）
    pub bound_addr: String,
}

/// 获取上位机网络端点：配置值与实际绑定地址
#[tauri::command]
pub async fn get_local_endpoint(app: AppHandle) -> LocalEndpoint {
    let config = current_config(&app);
    let bound_addr = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard
            .network
            .as_ref()
            .map(|n| n.local_addr().to_string())
            .unwrap_or_default()
    };

    LocalEndpoint {
        configured_ip: config.basic.local_ip,
        configured_port: config.basic.local_port,
        bound_addr,
    }
}

/// config.toml 的实际路径，界面上展示给用户
#[tauri::command]
pub fn get_config_path() -> String {
    crate::config::config_path().display().to_string()
}

/// 保存配置到 config.toml，并同步更新内存中的配置：
/// 除本机 IP / 端口外，其余配置项立即生效。
#[tauri::command]
pub fn save_app_config(app: AppHandle, config: AppConfig) -> Result<SaveConfigResult, String> {
    config.validate()?;
    let path = crate::config::config_path();

    let state = app.state::<ConfigState>();
    let restart_required = {
        let old = state
            .0
            .read()
            .map_err(|_| "配置锁已损坏，请重启上位机".to_string())?;
        config_restart_required(&old, &config)
    };

    config
        .save(&path)
        .map_err(|e| format!("保存配置失败（{}）: {}", path.display(), e))?;

    *state
        .0
        .write()
        .map_err(|_| "配置锁已损坏，请重启上位机".to_string())? = config;

    log::info!("Config saved and applied: {}", path.display());
    Ok(SaveConfigResult { restart_required })
}

/// 发现局域网内设备：发送 0x0100 广播，300ms 内收集响应设备；
/// `[basic].broadcast_search = false` 时不广播，直接返回 `device_ip`
#[tauri::command]
pub async fn discover_devices(app: AppHandle) -> Result<Vec<DiscoveredDevice>, String> {
    // broadcast_search = false：不广播，直接使用配置中的设备 IP
    {
        let config = current_config(&app);
        if !config.basic.broadcast_search {
            let ip_str = config.basic.device_ip.trim();
            let ip: std::net::IpAddr = ip_str.parse().map_err(|_| {
                format!("config.toml [basic].device_ip 未配置或非法: {:?}", ip_str)
            })?;

            let devices = vec![DiscoveredDevice {
                ip: ip.to_string(),
                port: config.basic.remote_port,
            }];
            let _ = app.emit("devices-found", devices.clone());
            log::info!("Broadcast search disabled, using configured device {}", ip);
            return Ok(devices);
        }
    }

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

        let config = current_config(&app);

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
    let config = current_config(&app);
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

/// 注册预览二进制推送通道（前端连接设备时调用）
#[tauri::command]
pub async fn register_preview_channel(
    app: AppHandle,
    channel: tauri::ipc::Channel<Vec<u8>>,
) -> Result<(), String> {
    let state = app.state::<Arc<Mutex<NetworkState>>>();
    let mut guard = state.lock().await;
    guard.preview_channel = Some(channel);
    log::info!("Preview channel registered");
    Ok(())
}

/// 注销预览推送通道（断开连接时调用）
#[tauri::command]
pub async fn unregister_preview_channel(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Arc<Mutex<NetworkState>>>();
    let mut guard = state.lock().await;
    guard.preview_channel = None;
    log::info!("Preview channel unregistered");
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

/// 设备地址（IP:remote_port）
async fn device_target(app: &AppHandle, device_ip: &str) -> Result<SocketAddr, String> {
    let config = current_config(&app);
    format!("{}:{}", device_ip, config.basic.remote_port)
        .parse()
        .map_err(|e| format!("Invalid device address: {}", e))
}

/// 获取请求-响应串行锁：激光器/电机透传命令期间独占应答通道
async fn acquire_op_lock(app: &AppHandle) -> tokio::sync::OwnedMutexGuard<()> {
    let lock = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.op_lock.clone()
    };
    lock.lock_owned().await
}

/// 获取 SSD 操作串行锁（文件列表/存储/下载）
async fn acquire_ssd_lock(app: &AppHandle) -> tokio::sync::OwnedMutexGuard<()> {
    let lock = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.ssd_lock.clone()
    };
    lock.lock_owned().await
}

/// 尝试获取 SSD 操作锁；已有操作时直接返回错误，避免下载排队
async fn try_acquire_ssd_lock(
    app: &AppHandle,
) -> Result<tokio::sync::OwnedMutexGuard<()>, String> {
    let lock = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.ssd_lock.clone()
    };
    lock.try_lock_owned()
        .map_err(|_| "已有 SSD 操作进行中，请稍后再试".to_string())
}

/// 尝试获取下载锁：SSD 下载和 SD 卡下载互斥
async fn try_acquire_download_lock(
    app: &AppHandle,
) -> Result<tokio::sync::OwnedMutexGuard<()>, String> {
    let lock = {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        guard.download_lock.clone()
    };
    lock.try_lock_owned()
        .map_err(|_| "已有下载任务进行中，请稍后再试".to_string())
}

/// 订阅后台监听器转发的帧
async fn subscribe_frames(app: &AppHandle) -> broadcast::Receiver<(Frame, SocketAddr)> {
    let state = app.state::<Arc<Mutex<NetworkState>>>();
    let guard = state.lock().await;
    guard.frame_tx.subscribe()
}

/// 打包并发送一帧（不等待应答）
async fn send_frame(
    app: &AppHandle,
    network: &UdpNetwork,
    target: SocketAddr,
    cmd: u16,
    data: Vec<u8>,
) -> Result<(), String> {
    let seq = next_seq_inner(app).await;
    let mut frame = Frame::new(cmd, SourceAddr::PC as u16, DestAddr::PS as u16, data);
    frame.seq = seq;
    network.send_to(&target, &frame).await
}

/// 发送一帧并等待来自同一 IP、同一命令码的应答。
/// 必须先订阅再发送，避免设备应答早于订阅而丢失。
async fn request(
    app: &AppHandle,
    network: &UdpNetwork,
    target: SocketAddr,
    cmd: u16,
    data: Vec<u8>,
    timeout_ms: u64,
) -> Result<Vec<u8>, String> {
    let mut rx = subscribe_frames(app).await;
    send_frame(app, network, target, cmd, data).await?;

    let deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_millis(timeout_ms);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(format!("等待 0x{:04x} 应答超时（{}ms）", cmd, timeout_ms));
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok((frame, addr))) => {
                // 只接受目标设备的应答，忽略其他设备与广播回环
                if frame.cmd == cmd && addr.ip() == target.ip() {
                    return Ok(frame.data);
                }
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                return Err("UDP 帧通道已关闭".into())
            }
            Err(_) => return Err(format!("等待 0x{:04x} 应答超时（{}ms）", cmd, timeout_ms)),
        }
    }
}

/// 字节序列格式化为十六进制，用于错误提示
fn hex_bytes(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 查询系统状态
#[tauri::command]
pub async fn query_sys_status(
    app: AppHandle,
    device_ip: String,
    enable: bool,
) -> Result<(), String> {
    let network = get_or_init_network(&app).await?;

    let config = current_config(&app);
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
        } else {
            guard.connected_device = None;
            guard.last_status = None;
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

    let config = current_config(&app);
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

    let config = current_config(&app);
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

/// 设置预览波形刷新间隔（ms），由波形图下方控件调用
#[tauri::command]
pub fn set_preview_refresh_ms(ms: u64) -> Result<(), String> {
    if !(crate::PREVIEW_REFRESH_MIN_MS..=crate::PREVIEW_REFRESH_MAX_MS).contains(&ms) {
        return Err(format!(
            "刷新间隔必须在 {}~{} ms 之间",
            crate::PREVIEW_REFRESH_MIN_MS, crate::PREVIEW_REFRESH_MAX_MS
        ));
    }
    crate::PREVIEW_REFRESH_MS.store(ms, std::sync::atomic::Ordering::Relaxed);
    log::info!("Preview refresh interval set to {} ms", ms);
    Ok(())
}

/// 设置信号采集模式（0x0f03）。
///
/// PIN 触发（mode=2）时，先用 0x0321 下发 pin 通道触发阈值，
/// 阈值取自 config.toml 的 `[capture].pin_threshold`。
#[tauri::command]
pub async fn set_trg_mode(
    app: AppHandle,
    device_ip: String,
    mode: u32,
) -> Result<(), String> {
    if mode > 2 {
        return Err(format!("无效的信号采集模式: {mode}"));
    }

    let network = get_or_init_network(&app).await?;
    let target = device_target(&app, &device_ip).await?;

    if mode == 2 {
        let pin_threshold = current_config(&app).capture.pin_threshold;
        send_frame(
            &app,
            &network,
            target,
            0x0321,
            pin_threshold.to_be_bytes().to_vec(),
        )
        .await?;
        log::info!("PIN 触发阈值已下发: {}", pin_threshold);
    }

    send_frame(
        &app,
        &network,
        target,
        0x0f03,
        mode.to_be_bytes().to_vec(),
    )
    .await?;
    log::info!("信号采集模式已设置: {}", mode);
    Ok(())
}

mod cloud;
mod control;
mod replay;
mod sdcard;
mod storage;

pub use cloud::*;
pub use control::*;
pub use replay::*;
pub use sdcard::*;
pub use storage::*;

#[cfg(test)]
mod tests {
    use super::{config_restart_required, resolve_device_ip};
    use crate::config::AppConfig;
    use std::net::SocketAddr;

    #[test]
    fn test_resolve_device_ip_fallback_to_source() {
        let addr: SocketAddr = "192.168.1.10:6666".parse().unwrap();
        assert_eq!(resolve_device_ip(b"", &addr), "192.168.1.10");
        assert_eq!(resolve_device_ip(b"\0\0\0\0", &addr), "192.168.1.10");
        assert_eq!(resolve_device_ip(b"192.168.1.20\0", &addr), "192.168.1.20");
    }

    #[test]
    fn test_restart_required_only_for_local_binding() {
        let old = AppConfig::default();

        let mut new = old.clone();
        new.radar.display_name = "新名称".into();
        new.basic.remote_port = 7000;
        assert!(!config_restart_required(&old, &new));

        let mut new = old.clone();
        new.basic.local_port = 23456;
        assert!(config_restart_required(&old, &new));

        let mut new = old.clone();
        new.basic.local_ip = "127.0.0.1".into();
        assert!(config_restart_required(&old, &new));
    }
}
