# 系统状态参数

| 数据类型    | offset | label                  | 作用                                                                                  |
| ----------- | -----: | ---------------------- | ------------------------------------------------------------------------------------- |
| uint8_t[16] |      0 | device_sn              | 出厂设置                                                                              |
| uint8_t[16] |     16 | emmc_sn                | 使用emmc芯片的序列号，不可变，不对用户显示                                            |
| uint8_t[16] |     32 | device_type            | 设备类型：按字符串解释                                                                |
| uint8_t[4]  |     48 | device_ID              | 设备编号：按字符串解释                                                                |
| uint32_t    |     52 | uptime                 | 启动后的秒数                                                                          |
| uint8_t[32] |     56 | ps_version             | 按照ascii解释                                                                         |
| uint8_t[32] |     88 | pl_version             | 按照ascii解释（1.23）                                                                 |
| uint32_t    |    120 | i2c_temp               | 将数据当作12位有符号数*0.0625                                                         |
| uint32_t[4] |    124 | reverse                |                                                                                       |
| uint32_t    |    140 | trg_cycle              | 1e9/(trg_cycle*8)                                                                     |
| uint32_t    |    144 | pps_cycle              | 1e9/(pps_cycle*8)                                                                     |
| uint32_t    |    148 | motor_speed            | 当值是32'hFFFF_FFFF，motor_speed = 0<br>否则，motor_speed = 1e9 / (uint32_t * 8) * 60 |
| uint32_t    |    152 | adc_ch0                | ch0: PMT1反馈                                                                         |
| uint32_t    |    156 | adc_ch1                | ch1: PMT2反馈                                                                         |
| uint32_t    |    160 | adc_ch2                | ch2: PMT3反馈                                                                         |
| uint32_t    |    164 | adc_ch3                | ch3: APD高压反馈                                                                      |
| uint32_t    |    168 | adc_ch4                | ch4: 外置热敏电阻1                                                                    |
| uint32_t    |    172 | adc_ch5                | ch5: 外置热敏电阻2                                                                    |
| uint32_t    |    176 | adc_ch6                | ch6: 板载热敏电阻1                                                                    |
| uint32_t    |    180 | adc_ch7                | ch7: 板载热敏电阻2                                                                    |
| uint32_t    |    184 | sys_status0            |                                                                                       |
| uint32_t    |    188 | sys_status1            | 预览FIFO空间不够时，本次预览数据不写入FIFO的次数                                      |
| uint32_t    |    192 | sys_status2            | sata数据FIFO空间不够时，本次采集数据不写入FIFO的次数                                  |
| uint32_t    |    196 | sys_status3            | bit[15:00], DDR3缓冲区读写指针差值<br>bit[31:16], DDR3缓冲区写满的次数                |
| uint32_t    |    200 | apd_gain               | APD增益                                                                               |
| uint32_t    |    204 | apd_temp_volt_cof      | APD温度高压系数                                                                       |
| uint32_t    |    208 | preivew_enable         | 上传预览数据。1, 上传；0, 不上传                                                      |
| uint32_t    |    212 | preivew_coe            | 采集帧抽样率                                                                          |
| uint32_t    |    216 | ref_channal            |                                                                                       |
| uint32_t    |    220 | save_channal           |                                                                                       |
| uint32_t    |    224 | wave_len               | 采集长度                                                                              |
| uint32_t    |    228 | first_pos              | 第一段起始位置                                                                        |
| uint32_t    |    232 | first_len              | 第一段采集长度                                                                        |
| uint32_t    |    236 | second_pos             | 第二段起始位置                                                                        |
| uint32_t    |    240 | second_len             | 第二段采集长度                                                                        |
| uint32_t    |    244 | sum_level              | 和阈值                                                                                |
| uint32_t    |    248 | min_level              | 值阈值                                                                                |
| uint32_t    |    252 | pin_threshold          |                                                                                       |
| uint32_t    |    256 | gps_store_enable       |                                                                                       |
| uint64_t    |    264 | gps_week               | 和doubel内存对齐, 才用了8字节                                                         |
| double      |    272 | gps_second             |                                                                                       |
| double      |    280 | gps_latitude           |                                                                                       |
| double      |    288 | gps_longitude          |                                                                                       |
| double      |    296 | gps_altitude           |                                                                                       |
| double      |    304 | gps_roll               |                                                                                       |
| double      |    312 | gps_pitch              |                                                                                       |
| double      |    320 | gps_azimuth            |                                                                                       |
| uint32_t    |    328 | laser_freq             | 上位机设置的激光器频率                                                                |
| uint32_t    |    332 | trg_mode               | 0x01, 外触发;<br>0x00, 内触发                                                         |
| uint32_t    |    336 | ssd_store_enable       | 1, 正在存储采集数据；0, 没有存储                                                      |
| uint32_t    |    340 | app_cmd_done           | 1, 上次sata命令成功; 0 失败                                                           |
| uint32_t    |    344 | sata_dev_diag_done     | 不需要显示                                                                            |
| uint32_t    |    348 | sata_dev_identify_done | sata连接状态。1,已连接；0,未连接                                                      |
| uint64_t    |    352 | sata_dev_tot_sec_num   | 硬盘容量：单位512字节                                                                 |
| uint64_t    |    360 | sata_app_lba_next      | 正在写的sata扇区。可用于计算sata剩余空间                                              |
| uint32_t    |    368 | max_write_time         | 单位：8ns                                                                             |
| uint32_t    |    372 | preview_wr_cnt         | Preview: PL通知PS自己已经写入了多少block                                              |
| uint32_t    |    376 | preview_rd_cnt         | Preview: PS通知PL自己已经读取了多少block                                              |

> 偏移为字段在小端结构体中的起始字节偏移。`gps_store_enable` 之后有 4 字节填充（260~263），`gps_week` 与 `gps_second` 之间也有 4 字节填充（268~271），整个状态包共 **380 字节**。

## sys_status0
- bit[8]: 一键启动按键电平值
- bit[7]: 1, 通道3帧封装FIFO溢出.
- bit[6]: 1, 通道2帧封装FIFO溢出.
- bit[5]: 1, 通道1帧封装FIFO溢出.
- bit[4]: 1, 通道0 帧封装FIFO溢出.
- bit[3]: 1, 通道3截取FIFO溢出.
- bit[2]: 1, 通道2截取FIFO溢出.
- bit[1]: 1, 通道1截取FIFO溢出.
- bit[0]: 1, 通道0截取FIFO溢出.
