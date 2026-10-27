//! SD 卡文件访问（SFTP）。
//!
//! 设备端运行 Dropbear + sftp-server，上位机通过 SSH/SFTP 直接读写
//! `/run/media/mmcblk1p1/` 下的 GPS 数据文件。
//!
//! 注意：当前按测试阶段要求硬编码 root 账号密码，且不校验 host key。
//! 后续应改为配置文件/系统 keyring + SSH 密钥，并固定设备指纹。

use crate::commands::{sanitize_filename, unique_path, SsdDownloadProgress};
use serde::Serialize;
use ssh2::Session;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;

const SD_SSH_PORT: u16 = 22;
const SD_SSH_USER: &str = "root";
// TODO: 测试阶段硬编码，后续移到配置/系统 keyring
const SD_SSH_PASSWORD: &str = "siom_uav";
const SD_READ_CHUNK: usize = 256 * 1024;
const SD_MISSING_MSG: &str = "未检测到 SD 卡，请插入 SD 卡后重试";

/// SD 卡下载取消标记：在 256KB 块边界检查
static SD_DOWNLOAD_CANCEL: AtomicBool = AtomicBool::new(false);

/// SD 卡文件信息
#[derive(Debug, Clone, Serialize)]
pub struct SdFile {
    pub name: String,
    pub size: u64,
    /// Unix 时间戳（秒）
    pub mtime: u64,
}

fn map_ssh_error(context: &str, err: ssh2::Error) -> String {
    format!("{}: {}", context, err)
}

/// 建立 SSH/SFTP 会话
fn connect(device_ip: &str) -> Result<Session, String> {
    let addr = format!("{}:{}", device_ip, SD_SSH_PORT);
    let socket_addr = addr
        .to_socket_addrs()
        .map_err(|e| format!("解析设备地址 {} 失败: {}", addr, e))?
        .next()
        .ok_or_else(|| format!("无法解析设备地址 {}", addr))?;

    let tcp = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5))
        .map_err(|e| format!("连接 SSH {} 失败: {}", addr, e))?;

    let mut sess = Session::new().map_err(|e| map_ssh_error("创建 SSH 会话失败", e))?;
    sess.set_tcp_stream(tcp);
    sess.set_timeout(10_000);
    sess.handshake()
        .map_err(|e| map_ssh_error("SSH 握手失败", e))?;
    sess.userauth_password(SD_SSH_USER, SD_SSH_PASSWORD)
        .map_err(|e| map_ssh_error("SSH 认证失败", e))?;
    if !sess.authenticated() {
        return Err("SSH 认证失败".into());
    }
    Ok(sess)
}

/// 读取 SD 卡目录，返回文件列表（不包含目录）
pub fn list_sd_files(device_ip: &str, dir: &str) -> Result<Vec<SdFile>, String> {
    let sess = connect(device_ip)?;
    let sftp = sess
        .sftp()
        .map_err(|e| map_ssh_error("启动 SFTP 失败", e))?;

    let dir_path = Path::new(dir);
    let dir_stat = sftp
        .stat(dir_path)
        .map_err(|_| SD_MISSING_MSG.to_string())?;
    if !dir_stat.is_dir() {
        return Err(format!("GPS 路径不是目录: {}", dir));
    }

    let entries = sftp
        .readdir(dir_path)
        .map_err(|e| map_ssh_error("读取 SD 卡目录失败", e))?;

    let mut files = Vec::new();
    for (path, stat) in entries {
        if stat.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) if !name.is_empty() && name != "." && name != ".." => name.to_string(),
            _ => continue,
        };
        files.push(SdFile {
            name,
            size: stat.size.unwrap_or(0),
            mtime: stat.mtime.unwrap_or(0),
        });
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

/// 下载一个 SD 卡文件到本机。
///
/// 不支持断点续传；取消/失败会删除 `.part` 临时文件。
pub fn download_sd_file(
    device_ip: &str,
    remote_dir: &str,
    filename: &str,
    local_dir: &str,
    channel: &Channel<SsdDownloadProgress>,
) -> Result<String, String> {
    let local_dir = Path::new(local_dir);
    if !local_dir.is_dir() {
        return Err(format!("保存目录不存在: {}", local_dir.display()));
    }

    let sess = connect(device_ip)?;
    let sftp = sess
        .sftp()
        .map_err(|e| map_ssh_error("启动 SFTP 失败", e))?;
    let remote_path = Path::new(remote_dir).join(filename);
    let mut remote = sftp
        .open(&remote_path)
        .map_err(|e| map_ssh_error("打开 SD 卡文件失败", e))?;

    let stat = remote
        .stat()
        .map_err(|e| map_ssh_error("读取 SD 卡文件信息失败", e))?;
    let total = stat
        .size
        .ok_or_else(|| "无法获取 SD 卡文件大小".to_string())?;

    let safe_name = sanitize_filename(filename);
    let part_path = local_dir.join(format!("{}.part", safe_name));
    let final_path = unique_path(local_dir, &safe_name);

    let result = download_inner(&mut remote, total, &part_path, &final_path, channel);
    if result.is_err() {
        let _ = std::fs::remove_file(&part_path);
    }
    result
}

fn download_inner(
    remote: &mut ssh2::File,
    total: u64,
    part_path: &Path,
    final_path: &Path,
    channel: &Channel<SsdDownloadProgress>,
) -> Result<String, String> {
    let mut local = File::create(part_path)
        .map_err(|e| format!("创建临时文件失败（{}）: {}", part_path.display(), e))?;

    let mut buf = vec![0u8; SD_READ_CHUNK];
    let mut transferred: u64 = 0;
    let mut speed_bps = 0.0f64;
    let mut cancelled = false;
    let mut last_emit = Instant::now();
    let mut last_time = Instant::now();
    let mut last_bytes: u64 = 0;

    channel
        .send(SsdDownloadProgress {
            transferred: 0,
            total,
            percent: 0.0,
            speed_bps: 0.0,
            chunk_mb: 0,
        })
        .map_err(|e| format!("发送下载进度失败: {}", e))?;

    loop {
        if SD_DOWNLOAD_CANCEL.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }

        let n = remote
            .read(&mut buf)
            .map_err(|e| format!("读取 SD 卡文件失败: {}", e))?;
        if n == 0 {
            break;
        }

        local
            .write_all(&buf[..n])
            .map_err(|e| format!("写入文件失败: {}", e))?;
        transferred += n as u64;

        let now = Instant::now();
        if now.duration_since(last_emit) >= Duration::from_millis(100) || transferred == total {
            let dt = now.duration_since(last_time).as_secs_f64();
            let instant = if dt > 0.0 {
                (transferred - last_bytes) as f64 / dt
            } else {
                0.0
            };
            speed_bps = if speed_bps > 0.0 {
                speed_bps * 0.7 + instant * 0.3
            } else {
                instant
            };
            last_emit = now;
            last_time = now;
            last_bytes = transferred;

            let percent = if total > 0 {
                transferred as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            channel
                .send(SsdDownloadProgress {
                    transferred,
                    total,
                    percent,
                    speed_bps,
                    chunk_mb: 0,
                })
                .map_err(|e| format!("发送下载进度失败: {}", e))?;
        }
    }

    local
        .flush()
        .map_err(|e| format!("刷新文件失败: {}", e))?;
    drop(local);

    if cancelled {
        return Err("下载已取消".into());
    }
    if total > 0 && transferred != total {
        return Err(format!(
            "文件读取不完整: 已读取 {} / {} 字节",
            transferred, total
        ));
    }

    std::fs::rename(part_path, final_path)
        .map_err(|e| format!("重命名下载文件失败: {}", e))?;
    Ok(final_path.to_string_lossy().to_string())
}

/// 取消当前 SD 卡下载
pub fn cancel() {
    SD_DOWNLOAD_CANCEL.store(true, Ordering::SeqCst);
}

/// 开始新下载前清除取消标记
pub fn reset_cancel() {
    SD_DOWNLOAD_CANCEL.store(false, Ordering::SeqCst);
}
