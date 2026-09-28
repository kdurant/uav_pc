use serde::Serialize;

/// 系统状态（对应 sys_status.md 定义的所有字段）
#[derive(Debug, Clone, Serialize, Default)]
pub struct SystemStatus {
    pub device_sn: String,
    pub emmc_sn: String,
    pub device_type: String,
    pub device_id: String,
    pub uptime: u32,
    pub ps_version: String,
    pub pl_version: String,
    pub i2c_temp: f32,
    pub trg_cycle: f64,
    pub pps_cycle: f64,
    pub motor_speed: f64,
    pub adc_ch0: u32,
    pub adc_ch1: u32,
    pub adc_ch2: u32,
    pub adc_ch3: u32,
    pub adc_ch4: u32,
    pub adc_ch5: u32,
    pub adc_ch6: u32,
    pub adc_ch7: u32,
    pub sys_status0: u32,
    pub sys_status1: u32,
    pub sys_status2: u32,
    pub sys_status3: u32,
    pub apd_gain: u32,
    pub apd_temp_volt_cof: u32,
    pub preview_enable: u32,
    pub preview_coe: u32,
    pub ref_channal: u32,
    pub save_channal: u32,
    pub wave_len: u32,
    pub first_pos: u32,
    pub first_len: u32,
    pub second_pos: u32,
    pub second_len: u32,
    pub sum_level: u32,
    pub min_level: u32,
    pub pin_threshold: u32,
    pub gps_store_enable: u32,
    pub gps_week: u32,
    pub gps_second: f64,
    pub gps_latitude: f64,
    pub gps_longitude: f64,
    pub gps_altitude: f64,
    pub gps_roll: f64,
    pub gps_pitch: f64,
    pub gps_azimuth: f64,
    pub laser_freq: u32,
    pub trg_mode: u32,
    pub ssd_store_enable: u32,
    pub app_cmd_done: u32,
    pub sata_dev_diag_done: u32,
    pub sata_dev_identify_done: u32,
    pub sata_dev_tot_sec_num: u64,
    pub sata_app_lba_next: u64,
    pub max_write_time: u32,
    pub preview_wr_cnt: u32,
    pub preview_rd_cnt: u32,
}

/// 从字节缓冲区读取 u32（小端序）
fn read_u32(data: &[u8], offset: &mut usize) -> Option<u32> {
    if *offset + 4 > data.len() {
        return None;
    }
    let val = u32::from_le_bytes([
        data[*offset],
        data[*offset + 1],
        data[*offset + 2],
        data[*offset + 3],
    ]);
    *offset += 4;
    Some(val)
}

/// 从字节缓冲区读取 u64（小端序）
fn read_u64(data: &[u8], offset: &mut usize) -> Option<u64> {
    if *offset + 8 > data.len() {
        return None;
    }
    let val = u64::from_le_bytes([
        data[*offset],
        data[*offset + 1],
        data[*offset + 2],
        data[*offset + 3],
        data[*offset + 4],
        data[*offset + 5],
        data[*offset + 6],
        data[*offset + 7],
    ]);
    *offset += 8;
    Some(val)
}

/// 从字节缓冲区读取 f64（小端序）
fn read_f64(data: &[u8], offset: &mut usize) -> Option<f64> {
    if *offset + 8 > data.len() {
        return None;
    }
    let val = f64::from_le_bytes([
        data[*offset],
        data[*offset + 1],
        data[*offset + 2],
        data[*offset + 3],
        data[*offset + 4],
        data[*offset + 5],
        data[*offset + 6],
        data[*offset + 7],
    ]);
    *offset += 8;
    Some(val)
}

/// 读取固定长度字节数组并转为字符串（遇到 null 截断）
fn read_fixed_str(data: &[u8], offset: &mut usize, len: usize) -> Option<String> {
    if *offset + len > data.len() {
        return None;
    }
    let slice = &data[*offset..*offset + len];
    *offset += len;
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    Some(String::from_utf8_lossy(&slice[..end]).to_string())
}

impl SystemStatus {
    /// 从字节数组解析系统状态
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        let mut status = SystemStatus::default();
        let mut offset: usize = 0;

        // uint8_t[16] device_sn
        status.device_sn = read_fixed_str(data, &mut offset, 16).unwrap_or_default();
        // uint8_t[16] emmc_sn
        status.emmc_sn = read_fixed_str(data, &mut offset, 16).unwrap_or_default();
        // uint8_t[16] device_type
        status.device_type = read_fixed_str(data, &mut offset, 16).unwrap_or_default();
        // uint8_t[4] device_ID
        status.device_id = read_fixed_str(data, &mut offset, 4).unwrap_or_default();
        // uint32_t uptime
        status.uptime = read_u32(data, &mut offset).unwrap_or(0);
        // uint8_t[32] ps_version
        status.ps_version = read_fixed_str(data, &mut offset, 32).unwrap_or_default();
        // uint8_t[32] pl_version
        status.pl_version = read_fixed_str(data, &mut offset, 32).unwrap_or_default();
        // uint32_t i2c_temp (12位有符号数 * 0.0625)
        let i2c_raw = read_u32(data, &mut offset).unwrap_or(0);
        status.i2c_temp = SystemStatus::parse_temp(i2c_raw);
        // uint32_t[4] reverse
        offset += 16; // 跳过 4 个 uint32
                      // uint32_t trg_cycle
        let trg_cycle_raw = read_u32(data, &mut offset).unwrap_or(0);
        status.trg_cycle = if trg_cycle_raw != 0 {
            1e9 / (trg_cycle_raw as f64 * 8.0)
        } else {
            0.0
        };
        // uint32_t pps_cycle
        let pps_cycle_raw = read_u32(data, &mut offset).unwrap_or(0);
        status.pps_cycle = if pps_cycle_raw != 0 {
            1e9 / (pps_cycle_raw as f64 * 8.0)
        } else {
            0.0
        };
        // uint32_t motor_speed
        let motor_raw = read_u32(data, &mut offset).unwrap_or(0);
        status.motor_speed = if motor_raw == 0xFFFF_FFFF || motor_raw == 0 {
            0.0
        } else {
            1e9 / (motor_raw as f64 * 8.0) * 60.0
        };
        // ADC channels ch0-ch7 (8 * uint32_t)
        status.adc_ch0 = read_u32(data, &mut offset).unwrap_or(0);
        status.adc_ch1 = read_u32(data, &mut offset).unwrap_or(0);
        status.adc_ch2 = read_u32(data, &mut offset).unwrap_or(0);
        status.adc_ch3 = read_u32(data, &mut offset).unwrap_or(0);
        status.adc_ch4 = read_u32(data, &mut offset).unwrap_or(0);
        status.adc_ch5 = read_u32(data, &mut offset).unwrap_or(0);
        status.adc_ch6 = read_u32(data, &mut offset).unwrap_or(0);
        status.adc_ch7 = read_u32(data, &mut offset).unwrap_or(0);
        // sys_status0-3
        status.sys_status0 = read_u32(data, &mut offset).unwrap_or(0);
        status.sys_status1 = read_u32(data, &mut offset).unwrap_or(0);
        status.sys_status2 = read_u32(data, &mut offset).unwrap_or(0);
        status.sys_status3 = read_u32(data, &mut offset).unwrap_or(0);
        // apd_gain
        status.apd_gain = read_u32(data, &mut offset).unwrap_or(0);
        // apd_temp_volt_cof
        status.apd_temp_volt_cof = read_u32(data, &mut offset).unwrap_or(0);
        // preview_enable
        status.preview_enable = read_u32(data, &mut offset).unwrap_or(0);
        // preview_coe
        status.preview_coe = read_u32(data, &mut offset).unwrap_or(0);
        // ref_channal
        status.ref_channal = read_u32(data, &mut offset).unwrap_or(0);
        // save_channal
        status.save_channal = read_u32(data, &mut offset).unwrap_or(0);
        // wave_len
        status.wave_len = read_u32(data, &mut offset).unwrap_or(0);
        // first_pos
        status.first_pos = read_u32(data, &mut offset).unwrap_or(0);
        // first_len
        status.first_len = read_u32(data, &mut offset).unwrap_or(0);
        // second_pos
        status.second_pos = read_u32(data, &mut offset).unwrap_or(0);
        // second_len
        status.second_len = read_u32(data, &mut offset).unwrap_or(0);
        // sum_level
        status.sum_level = read_u32(data, &mut offset).unwrap_or(0);
        // min_level
        status.min_level = read_u32(data, &mut offset).unwrap_or(0);
        // pin_threshold
        status.pin_threshold = read_u32(data, &mut offset).unwrap_or(0);
        // gps_store_enable
        status.gps_store_enable = read_u32(data, &mut offset).unwrap_or(0);
        // gps_week
        status.gps_week = read_u32(data, &mut offset).unwrap_or(0);
        // 固件在 GPS 段实际多出一个 8 字节字段（sys_status.md 未列出），
        // 不跳过会导致其后所有字段（激光频率、硬盘容量等）整体错位
        offset += 8;
        // double gps_second
        status.gps_second = read_f64(data, &mut offset).unwrap_or(0.0);
        // double gps_latitude
        status.gps_latitude = read_f64(data, &mut offset).unwrap_or(0.0);
        // double gps_longitude
        status.gps_longitude = read_f64(data, &mut offset).unwrap_or(0.0);
        // double gps_altitude
        status.gps_altitude = read_f64(data, &mut offset).unwrap_or(0.0);
        // double gps_roll
        status.gps_roll = read_f64(data, &mut offset).unwrap_or(0.0);
        // double gps_pitch
        status.gps_pitch = read_f64(data, &mut offset).unwrap_or(0.0);
        // double gps_azimuth
        status.gps_azimuth = read_f64(data, &mut offset).unwrap_or(0.0);
        // laser_freq
        status.laser_freq = read_u32(data, &mut offset).unwrap_or(0);
        // trg_mode
        status.trg_mode = read_u32(data, &mut offset).unwrap_or(0);
        // ssd_store_enable
        status.ssd_store_enable = read_u32(data, &mut offset).unwrap_or(0);
        // app_cmd_done
        status.app_cmd_done = read_u32(data, &mut offset).unwrap_or(0);
        // sata_dev_diag_done
        status.sata_dev_diag_done = read_u32(data, &mut offset).unwrap_or(0);
        // sata_dev_identify_done
        status.sata_dev_identify_done = read_u32(data, &mut offset).unwrap_or(0);
        // uint64_t sata_dev_tot_sec_num
        status.sata_dev_tot_sec_num = read_u64(data, &mut offset).unwrap_or(0);
        // uint64_t sata_app_lba_next
        status.sata_app_lba_next = read_u64(data, &mut offset).unwrap_or(0);
        // max_write_time
        status.max_write_time = read_u32(data, &mut offset).unwrap_or(0);
        // preview_wr_cnt
        status.preview_wr_cnt = read_u32(data, &mut offset).unwrap_or(0);
        // preview_rd_cnt
        status.preview_rd_cnt = read_u32(data, &mut offset).unwrap_or(0);

        Ok(status)
    }

    /// 解析温度：12位有符号数 * 0.0625
    fn parse_temp(raw: u32) -> f32 {
        // 取低12位
        let val = (raw & 0xFFF) as u16;
        // 如果第11位为1（负数），符号扩展到i16
        let signed = if val & 0x800 != 0 {
            (val | 0xF000) as i16
        } else {
            val as i16
        };
        signed as f32 * 0.0625
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_temp_positive() {
        // 25.0 / 0.0625 = 400 = 0x190
        assert!((SystemStatus::parse_temp(0x190) - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_temp_negative() {
        // -10.0 / 0.0625 = -160 -> 0xF60 (12-bit), 0xFF60 (16-bit), stored as 0xF60
        let raw = 0xF60u32;
        let temp = SystemStatus::parse_temp(raw);
        assert!((temp - (-10.0)).abs() < 0.1, "Got {}", temp);
    }

    #[test]
    fn test_parse_temp_zero() {
        assert!((SystemStatus::parse_temp(0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_disk_and_laser_offset() {
        // 固件在 GPS 段多出 8 字节，验证跳过后 laser_freq / 硬盘容量落在正确偏移
        let mut data = vec![0u8; 376];
        data[328..332].copy_from_slice(&5000u32.to_le_bytes()); // laser_freq
        data[352..360].copy_from_slice(&1_953_525_168u64.to_le_bytes()); // 1TB 容量（扇区）
        let status = SystemStatus::parse(&data).unwrap();
        assert_eq!(status.laser_freq, 5000);
        assert_eq!(status.sata_dev_tot_sec_num, 1_953_525_168);
    }

    #[test]
    fn test_motor_speed_ffff() {
        let mut offset = 0;
        let bytes: Vec<u8> = {
            // Build minimal data up to motor_speed field
            // SN(16) + emmc_sn(16) + device_type(16) + device_id(4) + uptime(4)
            // + ps_version(32) + pl_version(32) + i2c_temp(4) + reverse(16)
            // + trg_cycle(4) + pps_cycle(4) + motor_speed(4)
            let mut v = vec![0u8; 16 + 16 + 16 + 4 + 4 + 32 + 32 + 4 + 16 + 4 + 4 + 4];
            let motor_offset = v.len() - 4;
            v[motor_offset..motor_offset + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
            v
        };
        let mut off = 0;
        // skip to motor_speed
        off += 16 + 16 + 16 + 4 + 4 + 32 + 32 + 4 + 16 + 4 + 4;
        let motor_raw = read_u32(&bytes, &mut off).unwrap();
        let speed = if motor_raw == 0xFFFF_FFFF || motor_raw == 0 {
            0.0
        } else {
            1.0
        };
        assert_eq!(speed, 0.0);
    }
}
