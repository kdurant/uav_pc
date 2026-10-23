//! 激光器（0x0200）与电机（0x0f00）透传控制。

use super::*;

// ========== 激光器 / 电机（0x0200 / 0x0f00 透传） ==========

/// 激光器控制。
///
/// action:
/// - `open` / `close`：打开/关闭激光器
/// - `mode`：工作模式，value=0 内触发、1 外触发
/// - `current`：设置电流，value 单位 mA
/// - `freq`：设置重频，value 单位 Hz（0x0201，无应答）
/// - `status`：查询实时状态
#[tauri::command]
pub async fn laser_command(
    app: AppHandle,
    device_ip: String,
    action: String,
    value: Option<u32>,
) -> Result<Option<LaserStatus>, String> {
    let _op = acquire_op_lock(&app).await;
    let target = device_target(&app, &device_ip).await?;
    let network = get_or_init_network(&app).await?;

    match action.as_str() {
        "open" | "close" => {
            let (payload, expect) = if action == "open" {
                (laser::build_open(), laser::open_ack())
            } else {
                (laser::build_close(), laser::open_ack())
            };
            let resp = request(
                &app,
                &network,
                target,
                0x0200,
                payload,
                laser::LASER_RESPONSE_TIMEOUT_MS,
            )
            .await?;
            if !laser::matches_ack(&resp, expect) {
                return Err(format!("激光器{}失败，应答: {}", if action == "open" { "打开" } else { "关闭" }, hex_bytes(&resp)));
            }
            log::info!("Laser {} ok", action);
            Ok(None)
        }
        "mode" => {
            let mode = match value {
                Some(0) => laser::OpenMode::InSide,
                Some(1) => laser::OpenMode::OutSide,
                _ => return Err("激光器模式取值必须为 0（内触发）或 1（外触发）".into()),
            };
            let resp = request(
                &app,
                &network,
                target,
                0x0200,
                laser::build_set_mode(mode),
                laser::LASER_RESPONSE_TIMEOUT_MS,
            )
            .await?;
            if !laser::matches_ack(&resp, laser::mode_ack()) {
                return Err(format!("激光器模式设置失败，应答: {}", hex_bytes(&resp)));
            }
            log::info!("Laser mode set to {:?}", mode);
            Ok(None)
        }
        "current" => {
            let current = value.ok_or("缺少电流值（mA）")?;
            if current > 6000 {
                return Err("激光器电流超出范围（0~6000 mA）".into());
            }
            let resp = request(
                &app,
                &network,
                target,
                0x0200,
                laser::build_set_current(current),
                laser::LASER_RESPONSE_TIMEOUT_MS,
            )
            .await?;
            if !laser::matches_ack(&resp, laser::current_ack()) {
                return Err(format!("激光器电流设置失败，应答: {}", hex_bytes(&resp)));
            }
            log::info!(
                "Laser current set to {} mA (ack: {})",
                current,
                hex_bytes(&resp)
            );
            Ok(None)
        }
        "freq" => {
            let freq = value.ok_or("缺少重频值（Hz）")?;
            if freq == 0 || freq > 100_000 {
                return Err("激光重频超出范围（1~100000 Hz）".into());
            }
            send_frame(&app, &network, target, 0x0201, freq.to_be_bytes().to_vec()).await?;
            log::info!("Laser freq set to {} Hz", freq);
            Ok(None)
        }
        "status" => {
            let resp = request(
                &app,
                &network,
                target,
                0x0200,
                laser::build_query_status(),
                laser::LASER_RESPONSE_TIMEOUT_MS,
            )
            .await?;
            let status = laser::parse_status(&resp)?;
            log::debug!("Laser status: {:?}", status);
            Ok(Some(status))
        }
        other => Err(format!("未知的激光器操作: {}", other)),
    }
}

/// 电机控制单步：发送 OpCode → 发送数据 → 取应答 → 结束帧
async fn motor_step1(
    app: &AppHandle,
    network: &UdpNetwork,
    target: SocketAddr,
    frame: &[u8],
) -> Result<(), String> {
    if frame.is_empty() {
        return Err("电机命令为空".into());
    }
    // step1: 先发 OpCode
    let resp = request(
        app,
        network,
        target,
        0x0f00,
        vec![frame[0]],
        motor::MOTOR_RESPONSE_TIMEOUT_MS,
    )
    .await?;
    if !(resp.len() == 1 && resp[0] == 0x4f) {
        return Err(format!("电机未应答 0x4F（OpCode 阶段）: {}", hex_bytes(&resp)));
    }

    // step2: 再发其余数据
    let resp = request(
        app,
        network,
        target,
        0x0f00,
        frame[1..].to_vec(),
        motor::MOTOR_RESPONSE_TIMEOUT_MS,
    )
    .await?;
    if !(resp.len() == 2 && resp[0] == 0x4f && resp[1] == 0x00) {
        return Err(format!("电机未应答 4F 00（数据阶段）: {}", hex_bytes(&resp)));
    }
    Ok(())
}

/// 电机收尾：主动发 0x4F 取应答，再发一次 0x4F 结束
async fn motor_finish(
    app: &AppHandle,
    network: &UdpNetwork,
    target: SocketAddr,
) -> Result<Vec<u8>, String> {
    let resp = request(
        app,
        network,
        target,
        0x0f00,
        vec![0x4f],
        motor::MOTOR_RESPONSE_TIMEOUT_MS,
    )
    .await?;
    // 结束帧：只发不等待
    send_frame(app, network, target, 0x0f00, vec![0x4f]).await?;
    Ok(resp)
}

/// 执行一条电机写命令（应答 7 字节）
async fn motor_write(
    app: &AppHandle,
    network: &UdpNetwork,
    target: SocketAddr,
    frame: &[u8],
) -> Result<(), String> {
    motor_step1(app, network, target, frame).await?;
    let resp = motor_finish(app, network, target).await?;
    if resp.len() != 7 {
        return Err(format!("电机结束应答长度异常: {}（{}）", resp.len(), hex_bytes(&resp)));
    }
    Ok(())
}

/// 执行一条电机读命令，返回 11 字节应答
async fn motor_read(
    app: &AppHandle,
    network: &UdpNetwork,
    target: SocketAddr,
    frame: &[u8],
) -> Result<Vec<u8>, String> {
    motor_step1(app, network, target, frame).await?;
    let resp = motor_finish(app, network, target).await?;
    if resp.len() != 0x0b {
        return Err(format!("电机读取应答长度异常: {}（{}）", resp.len(), hex_bytes(&resp)));
    }
    Ok(resp)
}

/// 电机控制。
///
/// action:
/// - `init`：初始化（故障复位/关机/暂停/速度模式/参数）
/// - `run`：速度模式运行，value 单位 r/s
/// - `stop`：速度清零
/// - `move_to`：绝对定位，value 单位编码器步数
/// - `home`：回零，value 为回零方法（默认 34）
/// - `home_stop`：停止回零
/// - `read`：读取实际位置与速度
#[tauri::command]
pub async fn motor_command(
    app: AppHandle,
    device_ip: String,
    action: String,
    value: Option<i32>,
) -> Result<Option<MotorState>, String> {
    let _op = acquire_op_lock(&app).await;
    let target = device_target(&app, &device_ip).await?;
    let network = get_or_init_network(&app).await?;

    match action.as_str() {
        "init" => {
            for cmd in motor::init_sequence() {
                motor_write(&app, &network, target, &cmd.frame()).await?;
            }
            log::info!("Motor initialized");
            Ok(None)
        }
        "run" => {
            let speed = value.ok_or("缺少目标速度")?;
            if !(0..=3571).contains(&speed) {
                return Err("电机速度超出范围（0~3571 r/s）".into());
            }
            for cmd in motor::run_sequence(speed as u16) {
                motor_write(&app, &network, target, &cmd.frame()).await?;
            }
            log::info!("Motor running at {} r/s", speed);
            Ok(None)
        }
        "stop" => {
            for cmd in motor::stop_sequence() {
                motor_write(&app, &network, target, &cmd.frame()).await?;
            }
            log::info!("Motor stopped");
            Ok(None)
        }
        "move_to" => {
            let position = value.ok_or("缺少目标位置")?;
            for cmd in motor::move_to_sequence(position) {
                motor_write(&app, &network, target, &cmd.frame()).await?;
            }
            log::info!("Motor moving to position {}", position);
            Ok(None)
        }
        "home" => {
            let method = value.unwrap_or(34);
            if !(0..=35).contains(&method) {
                return Err("回零方法超出范围（0~35）".into());
            }
            for cmd in motor::home_sequence(method) {
                motor_write(&app, &network, target, &cmd.frame()).await?;
            }
            log::info!("Motor homing started (method {})", method);
            Ok(None)
        }
        "home_stop" => {
            motor_write(
                &app,
                &network,
                target,
                &motor::MotorCmd::StopHoming.frame(),
            )
            .await?;
            log::info!("Motor homing stopped");
            Ok(None)
        }
        "read" => {
            let vel_resp = motor_read(&app, &network, target, &motor::read_velocity_frame()).await?;
            let pos_resp = motor_read(&app, &network, target, &motor::read_position_frame()).await?;
            let velocity = motor::parse_actual_velocity(&vel_resp)
                .ok_or_else(|| format!("解析电机速度失败: {}", hex_bytes(&vel_resp)))?;
            let position = motor::parse_actual_position(&pos_resp)
                .ok_or_else(|| format!("解析电机位置失败: {}", hex_bytes(&pos_resp)))?;
            Ok(Some(MotorState { position, velocity }))
        }
        other => Err(format!("未知的电机操作: {}", other)),
    }
}
