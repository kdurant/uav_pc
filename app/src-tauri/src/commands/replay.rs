//! 离线数据回放相关命令

use crate::replay::{ReplayInfo, ReplaySession};
use std::sync::Mutex;
use tauri::State;

/// 回放会话状态（同一时间只允许打开一个文件）
#[derive(Default)]
pub struct ReplayState {
    pub session: Mutex<Option<ReplaySession>>,
}

/// 打开回放文件，返回帧数等摘要信息。
///
/// 建索引需要顺序扫一遍文件（示例 612MB 文件约 1 秒），放到阻塞线程池执行，
/// 避免卡住 IPC 主线程。
#[tauri::command]
pub async fn open_replay_file(
    path: String,
    state: State<'_, ReplayState>,
) -> Result<ReplayInfo, String> {
    let session = tauri::async_runtime::spawn_blocking(move || {
        ReplaySession::open(std::path::Path::new(&path))
    })
    .await
    .map_err(|e| format!("回放任务执行失败: {e}"))??;

    let info = session.info().clone();
    let mut guard = state.session.lock().map_err(|_| "回放状态锁失败")?;
    *guard = Some(session);
    Ok(info)
}

/// 读取指定帧，返回与实时预览一致的 PRV1 二进制
#[tauri::command]
pub fn read_replay_frame(
    index: u64,
    state: State<'_, ReplayState>,
) -> Result<tauri::ipc::Response, String> {
    let mut guard = state.session.lock().map_err(|_| "回放状态锁失败")?;
    let session = guard.as_mut().ok_or("尚未打开回放文件")?;
    let frame = session.read_frame(index)?;
    Ok(tauri::ipc::Response::new(frame.to_binary()))
}

/// 关闭回放文件，释放句柄
#[tauri::command]
pub fn close_replay_file(state: State<'_, ReplayState>) {
    if let Ok(mut guard) = state.session.lock() {
        *guard = None;
    }
}
