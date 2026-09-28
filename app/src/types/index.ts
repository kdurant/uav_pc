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

/** 连接状态 */
export enum ConnectionState {
    Disconnected = "disconnected",
    Searching = "searching",
    Connected = "connected",
}
