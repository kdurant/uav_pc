# 系统状态参数

| 数据类型    | label                  | 作用                                                                                  |
| ----------- | ---------------------- | ------------------------------------------------------------------------------------- |
| uint8_t[16] | device_sn              | 出厂设置                                                                              |
| uint8_t[16] | emmc_sn                | 使用emmc芯片的序列号，不可变，不对用户显示                                            |
| uint8_t[16] | device_type            | 设备类型：按字符串解释                                                                |
| uint8_t[4]  | device_ID              | 设备编号：按字符串解释                                                                |
| uint32_t    | uptime                 | 启动后的秒数                                                                          |
| uint8_t[32] | ps_version             | 按照ascii解释                                                                         |
| uint8_t[32] | pl_version             | 按照ascii解释（1.23）                                                                 |
| uint32_t    | i2c_temp               | 将数据当作12位有符号数*0.0625                                                         |
| uint32_t[4] | reverse                |                                                                                       |
| uint32_t    | trg_cycle              | 1e9/(trg_cycle*8)                                                                     |
| uint32_t    | pps_cycle              | 1e9/(pps_cycle*8)                                                                     |
| uint32_t    | motor_speed            | 当值是32'hFFFF_FFFF，motor_speed = 0<br>否则，motor_speed = 1e9 / (uint32_t * 8) * 60 |
| uint32_t    | adc_ch0                | ch0: PMT1反馈                                                                         |
| uint32_t    | adc_ch1                | ch1: PMT2反馈                                                                         |
| uint32_t    | adc_ch2                | ch2: PMT3反馈                                                                         |
| uint32_t    | adc_ch3                | ch3: APD高压反馈                                                                      |
| uint32_t    | adc_ch4                | ch4: 外置热敏电阻1                                                                    |
| uint32_t    | adc_ch5                | ch5: 外置热敏电阻2                                                                    |
| uint32_t    | adc_ch6                | ch6: 板载热敏电阻1                                                                    |
| uint32_t    | adc_ch7                | ch7: 板载热敏电阻2                                                                    |
| uint32_t    | sys_status0            |                                                                                       |
| uint32_t    | sys_status1            | 预览FIFO空间不够时，本次预览数据不写入FIFO的次数                                      |
| uint32_t    | sys_status2            | sata数据FIFO空间不够时，本次采集数据不写入FIFO的次数                                  |
| uint32_t    | sys_status3            | bit[15:00], DDR3缓冲区读写指针差值<br>bit[31:16], DDR3缓冲区写满的次数                |
| uint32_t    | apd_gain               | APD增益                                                                               |
| uint32_t    | apd_temp_volt_cof      | APD温度高压系数                                                                       |
| uint32_t    | preivew_enable         | 上传预览数据。1, 上传；0, 不上传                                                      |
| uint32_t    | preivew_coe            | 采集帧抽样率                                                                          |
| uint32_t    | ref_channal            |                                                                                       |
| uint32_t    | save_channal           |                                                                                       |
| uint32_t    | wave_len               | 采集长度                                                                              |
| uint32_t    | first_pos              | 第一段起始位置                                                                        |
| uint32_t    | first_len              | 第一段采集长度                                                                        |
| uint32_t    | second_pos             | 第二段起始位置                                                                        |
| uint32_t    | second_len             | 第二段采集长度                                                                        |
| uint32_t    | sum_level              | 和阈值                                                                                |
| uint32_t    | min_level              | 值阈值                                                                                |
| uint32_t    | pin_threshold          |                                                                                       |
| uint32_t    | gps_store_enable       |                                                                                       |
| uint32_t    | gps_week               |                                                                                       |
| double      | gps_second             |                                                                                       |
| double      | gps_latitude           |                                                                                       |
| double      | gps_longitude          |                                                                                       |
| double      | gps_altitude           |                                                                                       |
| double      | gps_roll               |                                                                                       |
| double      | gps_pitch              |                                                                                       |
| double      | gps_azimuth            |                                                                                       |
| uint32_t    | laser_freq             |                                                                                       |
| uint32_t    | trg_mode               | 0x01, 外触发;<br>0x00, 内触发                                                         |
| uint32_t    | ssd_store_enable       | 1, 正在存储采集数据；0, 没有存储                                                      |
| uint32_t    | app_cmd_done           | 1, 上次sata命令成功; 0 失败                                                           |
| uint32_t    | sata_dev_diag_done     | 不需要显示                                                                            |
| uint32_t    | sata_dev_identify_done | sata连接状态。1,已连接；0,未连接                                                      |
| uint64_t    | sata_dev_tot_sec_num   | 硬盘容量：单位512字节                                                                 |
| uint64_t    | sata_app_lba_next      | 正在写的sata扇区。可用于计算sata剩余空间                                              |
| uint32_t    | max_write_time         | 单位：8ns                                                                             |
| uint32_t    | preview_wr_cnt         | Preview: PL通知PS自己已经写入了多少block                                              |
| uint32_t    | preview_rd_cnt         | Preview: PS通知PL自己已经读取了多少block                                              |



## sys_status0
- bit[7]: 1, 通道3帧封装FIFO溢出.
- bit[6]: 1, 通道2帧封装FIFO溢出.
- bit[5]: 1, 通道1帧封装FIFO溢出.
- bit[4]: 1, 通道0 帧封装FIFO溢出.
- bit[3]: 1, 通道3截取FIFO溢出.
- bit[2]: 1, 通道2截取FIFO溢出.
- bit[1]: 1, 通道1截取FIFO溢出.
- bit[0]: 1, 通道0截取FIFO溢出.
