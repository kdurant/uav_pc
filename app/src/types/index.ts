/** 发现的设备信息 */
export interface DiscoveredDevice {
    ip: string;
    port: number;
}

/** 系统状态（对应 Rust SystemStatus 结构体） */
export interface SystemStatus {
    device_sn: string;
    emmc_sn: string;
    device_type: string;
    device_id: string;
    uptime: number;
    ps_version: string;
    pl_version: string;
    i2c_temp: number;
    trg_cycle: number;
    pps_cycle: number;
    motor_speed: number;
    adc_ch0: number;
    adc_ch1: number;
    adc_ch2: number;
    adc_ch3: number;
    adc_ch4: number;
    adc_ch5: number;
    adc_ch6: number;
    adc_ch7: number;
    sys_status0: number;
    sys_status1: number;
    sys_status2: number;
    sys_status3: number;
    apd_gain: number;
    apd_temp_volt_cof: number;
    preview_enable: number;
    preview_coe: number;
    ref_channal: number;
    save_channal: number;
    wave_len: number;
    first_pos: number;
    first_len: number;
    second_pos: number;
    second_len: number;
    sum_level: number;
    min_level: number;
    pin_threshold: number;
    gps_store_enable: number;
    gps_week: number;
    gps_second: number;
    gps_latitude: number;
    gps_longitude: number;
    gps_altitude: number;
    gps_roll: number;
    gps_pitch: number;
    gps_azimuth: number;
    laser_freq: number;
    trg_mode: number;
    ssd_store_enable: number;
    app_cmd_done: number;
    sata_dev_diag_done: number;
    sata_dev_identify_done: number;
    sata_dev_tot_sec_num: number;
    sata_app_lba_next: number;
    max_write_time: number;
    preview_wr_cnt: number;
    preview_rd_cnt: number;
}

/** 单个通道的波形数据 */
export interface ChannelData {
    channel_id: number;
    seg0_start: number;
    seg0_len: number;
    seg0_data: number[];
    seg1_start: number;
    seg1_len: number;
    seg1_data: number[];
}

/** TCP 预览数据帧 */
export interface PreviewFrame {
    gps_week: number;
    gps_second: number;
    sub_time: number;
    azimuth: number;
    pitch: number;
    roll: number;
    latitude: number;
    longitude: number;
    altitude: number;
    encoder_bits: number;
    encoder_value: number;
    wave_channels: number;
    wave_length: number;
    channels: ChannelData[];
}

/** 雷达/激光器配置（对应 Rust RadarConfig） */
export interface RadarConfig {
    /** 上位机显示名称 */
    display_name: string;
    /** 激光器类型，用于选择通信协议 */
    laser_type: string;
    /** 自定义激光器信息（预留字段，当前不在界面显示） */
    laser_info: string;
}

/** 应用配置 */
export interface AppConfig {
    basic: {
        local_ip: string;
        local_port: number;
    };
}

/** 激光器实时状态（对应 Rust LaserStatus） */
export interface LaserStatus {
    /** 开关状态：0 关闭，其它为开启 */
    status: number;
    error_code: number;
    /** 箱体温度（℃） */
    box_temp: number;
    /** 泵浦激光器 1 温度（℃） */
    pump1_temp: number;
    /** 泵浦激光器 2 温度（℃） */
    pump2_temp: number;
    /** 泵浦激光器 1 电流（mA） */
    pump1_current: number;
    /** 泵浦激光器 2 电流（mA） */
    pump2_current: number;
    /** 输出功率（mW） */
    export_power: number;

    // ===== 5kHz 激光器扩展字段 =====
    laser_number?: number | null;
    freq_detected_hz?: number | null;
    freq_set_hz?: number | null;
    pulse_width_us?: number | null;
    ld_current_ma?: number | null;
    temp1_c?: number | null;
    temp2_c?: number | null;
    /** 振荡级 LD 温控实际温度（℃，0x3C 帧） */
    ld_temp_c?: number | null;
    /** 倍频晶体温控实际温度（℃，0x3D 帧） */
    shg_temp_c?: number | null;
    /** 振荡级 LD 温控错误位：bit0 过温，bit1 过低 */
    ld_temp_error?: number | null;
    /** 倍频晶体温控错误位：bit0 过温，bit1 过低 */
    shg_temp_error?: number | null;
    q_delay_us?: number | null;
    work_time_s?: number | null;
    status_flags?: number | null;
    error_flags?: number | null;
}

/** 电机实时状态（对应 Rust MotorState） */export interface MotorState {
    /** 实际位置（编码器步数） */
    position: number;
    /** 实际速度（r/s） */
    velocity: number;
}

/** 历史温度采样点 */
export interface TemperaturePoint {
    /** 采样时间戳（ms） */
    time: number;
    /** 温度（℃） */
    temp: number;
}

/** 温度历史配置 */
export interface TemperatureConfig {
    /** 历史文件最多保存的点数 */
    save_points: number;
    /** 曲线最多显示的点数 */
    display_points: number;
}

/** SSD 已存储文件（对应 Rust SsdFile） */
export interface SsdFile {
    name: string;
    /** 起始地址（单位：扇区，512 字节） */
    start: number;
    /** 结束地址（单位：扇区，512 字节） */
    end: number;
    /** 文件大小（字节） */
    size: number;
}

/** SD 卡 GPS 文件（SFTP） */
export interface SdFile {
    name: string;
    /** 文件大小（字节） */
    size: number;
    /** Unix 时间戳（秒） */
    mtime: number;
}

/** SSD 下载进度（对应 Rust SsdDownloadProgress） */
export interface SsdDownloadProgress {
    /** 已传输字节数 */
    transferred: number;
    /** 文件总字节数 */
    total: number;
    /** 0~100 */
    percent: number;
    /** 实时速度（字节/秒） */
    speed_bps: number;
    /** 当前分块大小（MB） */
    chunk_mb: number;
}

/** 前端下载状态 */
export interface SsdDownloadState extends SsdDownloadProgress {
    /** 正在下载的文件名 */
    name: string;
    /** 用户已点击取消，等待当前块结束 */
    cancelling: boolean;
}

/** 连接状态 */
export enum ConnectionState {
    Disconnected = "disconnected",
    Searching = "searching",
    /** 已下发查询命令，等待第一帧设备状态 */
    Connecting = "connecting",
    Connected = "connected",
}

/** 离线回放文件摘要（对应 Rust ReplayInfo） */
export interface ReplayInfo {
    /** 文件路径 */
    path: string;
    /** 文件字节数 */
    file_size: number;
    /** 可完整解析的帧数 */
    frame_count: number;
    /** GPS 周（取首帧） */
    gps_week: number;
    /** 首帧 GPS 周内秒 */
    first_gps_second: number;
    /** 末帧 GPS 周内秒 */
    last_gps_second: number;
    /** 文件末尾未能解析的字节数（损坏或未写完），0 表示完整 */
    skipped_bytes: number;
}

/** 回放帧的 GPS / 姿态信息，供设备状态面板显示 */
export interface ReplayGps {
    /** 当前帧号（0 基） */
    frameIndex: number;
    /** 总帧数 */
    frameCount: number;
    gpsWeek: number;
    gpsSecond: number;
    /** 细分时间（100ns 单位） */
    subTime: number;
    latitude: number;
    longitude: number;
    altitude: number;
    azimuth: number;
    pitch: number;
    roll: number;
}
