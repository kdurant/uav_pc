use commands::NetworkState;
use std::sync::Arc;
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::{broadcast, Mutex};

mod commands;
mod config;
mod network;
mod preview;
mod protocol;
mod status;

/// 后台 UDP 监听任务：唯一 socket 消费者，将所有帧分发到 broadcast channel
async fn background_udp_listener(
    app: tauri::AppHandle,
    frame_tx: broadcast::Sender<(crate::protocol::Frame, std::net::SocketAddr)>,
) {
    // 等待网络初始化（最多等 10 秒）
    let mut attempts = 0;
    let network = loop {
        let state = app.state::<Arc<Mutex<NetworkState>>>();
        let guard = state.lock().await;
        if let Some(net) = guard.network.clone() {
            drop(guard);
            break net;
        }
        drop(guard);

        attempts += 1;
        if attempts > 100 {
            log::error!("Network not initialized after 10s, giving up");
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    };

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
                        match preview::PreviewFrame::parse(&frame.data) {
                            Ok(preview_frame) => {
                                let _ = app.emit("preview-data", preview_frame);
                            }
                            Err(e) => {
                                log::warn!("Failed to parse preview data: {}", e);
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

    let app_config = config::AppConfig::load("config.toml")
        .expect("Failed to load config.toml");

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
            commands::send_command,
            commands::start_preview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
