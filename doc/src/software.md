# 软件设计

基于 **Tauri v2 + Rust + Vue** 构建的跨平台桌面应用，通过UDP+TCP于FPGA采集卡通信, 获取采集卡状态，控制采集卡工作模式, 显示采集卡上传数据等

- [通信协议格式](./protocol_format.md)
- [具体通信指令](./pc-ps.md)
- [存储数据格式](./storge_data_format.md)


# 配置文件
- 使用config.toml作为配置文件
## 配置文件选项
```toml
[basic]
local_ip = "192.168.93.44"
local_port = 12345

remote_port = 6666

[capture]
preview_coe = 1000
ref_channal = 1
save_channal = 0xff
wave_len = 6000
first_pos = 100
first_len = 200
second_pos = 400
second_len = 400
sum_value = 0
max_value = 0
pin_threshold = 1000
```


# 界面设计

- 左侧用于控制功能。电机，激光器，采集参数等
- 右侧显示设备状态参数
- 中间用于显示数据折线图, 要满足至少100Hz，4000数据点的刷新要求


# 实现功能

## 局域网设备搜寻功能
1. 每次启动上位机后, 使用**local_ip**， **remote_port**，主动发送局域网广播
2. 设备会使用0x0100命令码上传设备ip地址
3. 100ms内，如果有多个设备上传ip地址，上位机会弹出选择界面

##  查询设备状态
使用一个checkbox来决定是否读取设备状态
- 发送的数据是0x01时，通知设备发送设备状态
- 发送的数据是0x00时，通知设备停止发送设备状态

## 解析，显示设备状态
1. 上位发送0x0102命令
2. 采集卡会响应0x0102命令, 参考./sys_status.md，解析并显示系统状态


## 数据采集，预览
1. 上位机上使用按键设置capture参数
2. 按键发送0x0300命令（0x00， 0x00, 0x00, 0x01)，通知设备上传[波形数据](./storge_data_format.md)
3. 上位机解析并显示
4. 可以通过checkbox独立选择是否显示那个通道的数据
5. 可以使用鼠标滚轮缩放预览数据
