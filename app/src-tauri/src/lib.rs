use commands::NetworkState;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::{broadcast, Mutex};

mod commands;
mod config;
mod laser;
mod motor;
mod network;
mod preview;
mod protocol;
mod status;

/// 预览数据推送到前端的最大频率（30Hz）。
/// Tauri v2 的事件通过 eval 注入 JS，每帧都要序列化 + 编译执行，
/// 波形帧数据量大，按设备原始上传频率（可达 100Hz）推送会占满 WebKit 主线程，
/// 导致按钮点击等交互明显变慢；画面只需保留最新一帧，因此这里限制推送频率。
const PREVIEW_EMIT_INTERVAL: Duration = Duration::from_millis(33);

/// 后台 UDP 监听任务：唯一 socket 消费者，将所有帧分发到 broadcast channel
async fn background_udp_listener(
    app: tauri::AppHandle,
    frame_tx: broadcast::Sender<(crate::protocol::Frame, std::net::SocketAddr)>,
) {
    // 等待网络初始化：网络在首个命令下发时创建，可能晚于窗口加载，
    // 因此一直等待（此前限制 10 秒会在前端加载慢时放弃，导致收不到状态帧）
    let network = loop {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        if let Some(net) = guard.network.clone() {
            drop(guard);
            break net;
        }
        drop(guard);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    };

    let mut last_preview_emit: Option<Instant> = None;

    loop {
        match network.recv_from().await {
            Ok((frame, addr)) => {
                // 检查是否有已连接的设备，如果有则只处理来自该设备的帧
                let connected = {
                    let state = app.state::<Arc<Mutex<NetworkState>>>();
                    let guard = state.lock().await;
                    guard.connected_device.clone()
                };

                let from_connected = match &connected {
                    Some(conn) => addr.ip() == conn.ip(),
                    None => false,
                };

                // 0x0100 设备发现响应始终允许（发现阶段可能未连接）
                // 0x0102 和 0x0300 只处理来自已连接设备的数据
                if frame.cmd == 0x0102 {
                    if from_connected {
                        match status::SystemStatus::parse(&frame.data) {
                            Ok(sys_status) => {
                                log::debug!("Received system status from {}", addr);
                                let _ = app.emit("sys-status-update", sys_status);
                            }
                            Err(e) => {
                                log::warn!("Failed to parse system status: {}", e);
                            }
                        }
                    } else {
                        log::trace!("Ignored 0x0102 from non-connected device {}", addr);
                    }
                } else if frame.cmd == 0x0300 {
                    if from_connected {
                        // 限频：只解析并推送最新一帧，丢弃间隔内的旧帧
                        let due = last_preview_emit
                            .map(|t| t.elapsed() >= PREVIEW_EMIT_INTERVAL)
                            .unwrap_or(true);
                        if due {
                            last_preview_emit = Some(Instant::now());
                            match preview::PreviewFrame::parse(&frame.data) {
                                Ok(preview_frame) => {
                                    let _ = app.emit("preview-data", preview_frame);
                                }
                                Err(e) => {
                                    log::warn!("Failed to parse preview data: {}", e);
                                }
                            }
                        }
                    } else {
                        log::trace!("Ignored 0x0300 from non-connected device {}", addr);
                    }
                } else {
                    log::trace!("Received cmd 0x{:04x} from {}", frame.cmd, addr);
                }

                // 0x0100 和 0x0102 转发到 broadcast channel（0x0100 供 discover 使用；0x0102 仅已连接的）
                if frame.cmd == 0x0100 || (frame.cmd == 0x0102 && from_connected) || (frame.cmd != 0x0100 && frame.cmd != 0x0102 && frame.cmd != 0x0300) {
                    let _ = frame_tx.send((frame, addr));
                }
            }
            Err(e) => {
                log::warn!("UDP receive error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
    }
}

pub fn run() {
    env_logger::init();

    let app_config = config::AppConfig::load_or_create("config.toml")
        .expect("Failed to load or create config.toml");

    let (frame_tx, _) = broadcast::channel::<(crate::protocol::Frame, std::net::SocketAddr)>(256);
    let frame_tx_setup = frame_tx.clone();

    tauri::Builder::default()
        .manage(app_config)
        .manage(Arc::new(Mutex::new(NetworkState {
            network: None,
            connected_device: None,
            seq_counter: 0,
            preview_enabled: false,
            frame_tx: frame_tx.clone(),
            op_lock: Arc::new(tokio::sync::Mutex::new(())),
        })))
        .setup(move |app| {
            log::info!("UAV PC application started");

            // 启动后台 UDP 监听任务（唯一 socket 消费者）
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                background_udp_listener(handle, frame_tx_setup).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::discover_devices,
            commands::query_sys_status,
            commands::send_ref_channel,
            commands::send_command,
            commands::laser_command,
            commands::motor_command,
            commands::start_preview,
            commands::list_ssd_files,
            commands::set_ssd_store,
            commands::query_gps_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
