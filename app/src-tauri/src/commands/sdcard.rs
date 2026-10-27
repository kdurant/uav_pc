//! SD 卡文件（SFTP）命令。

use super::*;
use crate::device_sftp;
use tauri::ipc::Channel;

/// 通过 SFTP 读取 SD 卡文件列表（含大小/修改时间）
#[tauri::command]
pub async fn list_sd_files(
    app: AppHandle,
    device_ip: String,
) -> Result<Vec<device_sftp::SdFile>, String> {
    let dir = app.state::<AppConfig>().gps.dir.clone();
    tokio::task::spawn_blocking(move || device_sftp::list_sd_files(&device_ip, &dir))
        .await
        .map_err(|e| format!("SD 卡文件检索任务失败: {}", e))?
}

/// 通过 SFTP 下载 SD 卡文件（不支持断点续传）
#[tauri::command]
pub async fn download_sd_file(
    app: AppHandle,
    device_ip: String,
    filename: String,
    local_dir: String,
    channel: Channel<SsdDownloadProgress>,
) -> Result<String, String> {
    let _download = try_acquire_download_lock(&app).await?;
    let dir = app.state::<AppConfig>().gps.dir.clone();
    device_sftp::reset_cancel();

    tokio::task::spawn_blocking(move || {
        device_sftp::download_sd_file(&device_ip, &dir, &filename, &local_dir, &channel)
    })
    .await
    .map_err(|e| format!("SD 卡下载任务失败: {}", e))?
}

/// 取消当前 SD 卡下载：在 256KB 块边界生效
#[tauri::command]
pub fn cancel_sd_download() {
    device_sftp::cancel();
}
