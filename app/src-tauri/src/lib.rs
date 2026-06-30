use commands::NetworkState;
use std::sync::Arc;
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::{broadcast, Mutex};

mod commands;
mod config;
mod network;
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
                // 0x0102 系统状态直接解析并推送前端
                if frame.cmd == 0x0102 {
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
                    log::trace!("Received cmd 0x{:04x} from {}", frame.cmd, addr);
                }

                // 所有帧都转发到 broadcast channel，供命令函数订阅
                let _ = frame_tx.send((frame, addr));
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
