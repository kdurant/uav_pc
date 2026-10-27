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
