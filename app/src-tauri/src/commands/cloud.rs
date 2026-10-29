//! 点云 / DEM 生成命令

use crate::pointcloud::{self, CloudParams};
use serde::Serialize;
use std::cell::Cell;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 进度事件负载
#[derive(Debug, Clone, Serialize)]
pub struct CloudProgress {
    /// 已处理帧数
    pub processed: u64,
    /// 总帧数（0 表示未知）
    pub total: u64,
    /// 0~100
    pub percent: f64,
}

/// 把采集数据转换成点云与 DEM。
///
/// 结果为二进制 `CLD1` 结构（见 `pointcloud::CloudResult::encode`），
/// 生成过程在阻塞线程池执行，并通过 `cloud-progress` 事件上报进度。
#[tauri::command]
pub async fn build_point_cloud(
    app: AppHandle,
    path: String,
    params: CloudParams,
) -> Result<tauri::ipc::Response, String> {
    let last_emit = Cell::new(Instant::now() - Duration::from_secs(1));

    let result = tauri::async_runtime::spawn_blocking(move || {
        let progress = |processed: u64, total: u64| {
            // 限频 200ms，避免大量小事件阻塞 IPC
            if last_emit.get().elapsed() < Duration::from_millis(200) {
                return;
            }
            last_emit.set(Instant::now());
            let percent = if total > 0 {
                (processed as f64 / total as f64 * 100.0).min(100.0)
            } else {
                0.0
            };
            let _ = app.emit(
                "cloud-progress",
                CloudProgress {
                    processed,
                    total,
                    percent,
                },
            );
        };
        pointcloud::build(std::path::Path::new(&path), &params, &progress)
    })
    .await
    .map_err(|e| format!("点云任务执行失败: {e}"))??;

    Ok(tauri::ipc::Response::new(result.encode()))
}
