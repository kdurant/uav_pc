//! 激光器（0x0200）与电机（0x0f00）透传控制。

use super::*;
use crate::laser_5khz;

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
    // 根据 config.toml [radar].laser_type 选择激光器通信协议。
    // 目前只有 laser_connet；未知类型直接拒绝，避免用错协议操作激光器。
    let laser_type = app.state::<AppConfig>().radar.laser_type.trim().to_string();
    if laser_type.is_empty() {
        return Err("未配置激光器类型（config.toml [radar].laser_type）".into());
    }
    if laser_type == "laser_5kHz" {
        return laser_5khz_command(app, device_ip, action, value).await;
    }
    if !laser::is_supported_type(&laser_type) {
        return Err(format!("暂不支持激光器类型: {}", laser_type));
    }

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

/// 5kHz / 25uJ 激光器主控板控制。
///
/// 命令帧和状态帧格式见 doc/laser/5khz/。
async fn laser_5khz_command(
    app: AppHandle,
    device_ip: String,
    action: String,
    value: Option<u32>,
) -> Result<Option<LaserStatus>, String> {
    let _op = acquire_op_lock(&app).await;
    let target = device_target(&app, &device_ip).await?;
    let network = get_or_init_network(&app).await?;

    let (frame, require_status) = match action.as_str() {
        "open" => (laser_5khz::build_open(), false),
        "close" => (laser_5khz::build_close(), false),
        "mode" => {
            let mode = value.ok_or("缺少激光器模式")?;
            if mode > 1 {
                return Err("激光器模式取值必须为 0（内触发）或 1（外触发）".into());
            }
            let mode = if mode == 0 {
                laser::OpenMode::InSide
            } else {
                laser::OpenMode::OutSide
            };
            (laser_5khz::build_mode(mode), false)
        }
        "freq" => {
            let hz = value.ok_or("缺少重频值（Hz）")?;
            if !(1..=5000).contains(&hz) {
                return Err("5kHz 激光器重频范围为 1~5000 Hz".into());
            }
            (laser_5khz::build_freq(hz), false)
        }
        "current" => {
            let ma = value.ok_or("缺少电流值（mA）")?;
            if ma > 6000 {
                return Err("5kHz 激光器电流范围为 0~6000 mA".into());
            }
            (laser_5khz::build_current_ma(ma), false)
        }
        "pulse_width" => {
            let us = value.ok_or("缺少脉宽值（µs）")?;
            if !(50..=80).contains(&us) {
                return Err("5kHz 激光器脉宽范围为 50~80 µs".into());
            }
            (laser_5khz::build_pulse_width(us as u16), false)
        }
        "clear_error" => (laser_5khz::build_clear_error(), false),
        "status" => (laser_5khz::build_query_status(), true),
        other => return Err(format!("未知的激光器操作: {}", other)),
    };

    if require_status {
        return laser_5khz_query_status(&app, &network, target)
            .await
            .map(Some);
    }

    // 文档只定义了状态查询(0xFF)的应答帧，开光/关光/设频率/设电流/设脉宽
    // 等设置类命令没有 ACK。这里发送成功即视为完成，不再等待 0x0200 应答，
    // 否则每次操作都会白等 1 秒后误报超时。
    send_frame(&app, &network, target, 0x0200, frame).await?;
    Ok(None)
}

/// 查询 5kHz 主控板状态。
///
/// 激光器串口是总线结构，一次查询会返回多帧：主控第一帧(0x00)、主控
/// 第二帧(0x99)、驱动板(0x0A)、温控板(0x3C/0x3D)。设备端按串口接收
/// 分段透传，主控帧不一定在第一段里。这里在超时周期内多次重发查询
/// （每次重发都会把设备的 UDP 回包目标重新指回本机，抵消其它客户端抢占），
/// 收集 0x00/0x99/0x3C/0x3D，合并成一份完整状态。
async fn laser_5khz_query_status(
    app: &AppHandle,
    network: &UdpNetwork,
    target: SocketAddr,
) -> Result<LaserStatus, String> {
    /// 两次查询之间的间隔；协议要求主控查询指令间隔 >= 150ms
    const POLL_INTERVAL_MS: u64 = 200;
    /// 拿到主控第一帧后，再等其它板卡帧的时间
    const SIBLING_GRACE_MS: u64 = 200;

    let mut rx = subscribe_frames(app).await;
    let timeout = std::time::Duration::from_millis(laser::LASER_RESPONSE_TIMEOUT_MS);
    let deadline = tokio::time::Instant::now() + timeout;
    let mut next_poll = tokio::time::Instant::now();
    let mut sibling_deadline: Option<tokio::time::Instant> = None;
    let mut buf: Vec<u8> = Vec::new();
    let mut received: Vec<u8> = Vec::new();
    let mut main_status: Option<LaserStatus> = None;
    let mut aux_status: Option<LaserStatus> = None;
    let mut ld_temp: Option<LaserStatus> = None;
    let mut shg_temp: Option<LaserStatus> = None;

    loop {
        for (addr, status) in laser_5khz::take_status_frames(&mut buf) {
            match addr {
                laser_5khz::MAIN_ADDR if main_status.is_none() => {
                    main_status = Some(status);
                    sibling_deadline = Some(
                        tokio::time::Instant::now()
                            + std::time::Duration::from_millis(SIBLING_GRACE_MS),
                    );
                }
                laser_5khz::MAIN_ADDR_2 if aux_status.is_none() => aux_status = Some(status),
                laser_5khz::TEMP_ADDR_LD if ld_temp.is_none() => ld_temp = Some(status),
                laser_5khz::TEMP_ADDR_SHG if shg_temp.is_none() => shg_temp = Some(status),
                _ => {}
            }
        }

        if main_status.is_some() {
            let all_ready =
                aux_status.is_some() && ld_temp.is_some() && shg_temp.is_some();
            let grace_expired = sibling_deadline
                .map(|d| tokio::time::Instant::now() >= d)
                .unwrap_or(false);
            if all_ready || grace_expired {
                return Ok(merge_status(
                    main_status.take().unwrap(),
                    aux_status.take(),
                    ld_temp.take(),
                    shg_temp.take(),
                ));
            }
        }

        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }

        if now >= next_poll {
            send_frame(
                app,
                network,
                target,
                0x0200,
                laser_5khz::build_query_status(),
            )
            .await?;
            next_poll = now + std::time::Duration::from_millis(POLL_INTERVAL_MS);
        }

        let wait_until = sibling_deadline
            .map(|d| d.min(next_poll))
            .unwrap_or(next_poll)
            .min(deadline);
        let wait = wait_until.saturating_duration_since(tokio::time::Instant::now());
        if wait.is_zero() {
            continue;
        }

        match tokio::time::timeout(wait, rx.recv()).await {
            Ok(Ok((frame, addr))) if frame.cmd == 0x0200 && addr.ip() == target.ip() => {
                received.extend_from_slice(&frame.data);
                buf.extend_from_slice(&frame.data);
            }
            Ok(Ok(_)) => continue,
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(_)) => break,
            Err(_) => continue, // 到下一次重发或等待其它板卡帧的时间点
        }
    }

    if let Some(main) = main_status.take() {
        return Ok(merge_status(
            main,
            aux_status.take(),
            ld_temp.take(),
            shg_temp.take(),
        ));
    }

    Err(format!(
        "5kHz 激光器未返回主控板状态帧（已收到: {}）",
        hex_bytes(&received)
    ))
}

/// 合并主控第一帧、第二帧和两块温控板的数据
fn merge_status(
    mut main: LaserStatus,
    aux: Option<LaserStatus>,
    ld_temp: Option<LaserStatus>,
    shg_temp: Option<LaserStatus>,
) -> LaserStatus {
    if let Some(aux) = aux {
        main.ld_current_ma = aux.ld_current_ma;
    }
    if let Some(temp) = ld_temp {
        main.ld_temp_c = temp.ld_temp_c;
        main.ld_temp_error = temp.ld_temp_error;
    }
    if let Some(temp) = shg_temp {
        main.shg_temp_c = temp.shg_temp_c;
        main.shg_temp_error = temp.shg_temp_error;
    }
    main
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
