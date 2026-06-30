<script setup lang="ts">
import { computed } from "vue";
import type { SystemStatus } from "../types";

const props = defineProps<{
  status: SystemStatus | null;
}>();

function formatUptime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

function formatDiskSize(sectors: number): string {
  const bytes = sectors * 512;
  if (bytes >= 1e12) return (bytes / 1e12).toFixed(2) + " TB";
  if (bytes >= 1e9) return (bytes / 1e9).toFixed(2) + " GB";
  if (bytes >= 1e6) return (bytes / 1e6).toFixed(1) + " MB";
  return bytes + " B";
}

/** 状态值格式化，保留 2 位小数 */
function fmt(v: number | undefined, digits = 2): string {
  if (v === undefined || v === null) return "-";
  return Number(v).toFixed(digits);
}

/** 电机转速格式化 */
function fmtMotorSpeed(v: number | undefined): string {
  if (v === undefined) return "-";
  return (v / 60).toFixed(1) + " Hz";
}

/** ADC 通道标签 */
const adcChannelLabels = [
  "PMT1反馈",
  "PMT2反馈",
  "PMT3反馈",
  "APD高压反馈",
  "外置热敏1",
  "外置热敏2",
  "板载热敏1",
  "板载热敏2",
];

/** 获取 ADC 通道值 */
function adcValue(ch: number): number {
  if (!props.status) return 0;
  const values = [
    props.status.adc_ch0, props.status.adc_ch1, props.status.adc_ch2, props.status.adc_ch3,
    props.status.adc_ch4, props.status.adc_ch5, props.status.adc_ch6, props.status.adc_ch7,
  ];
  return values[ch] ?? 0;
}

/** sys_status0 错误标记 */
const sys0Flags = computed(() => {
  if (!props.status) return [];
  const v = props.status.sys_status0;
  const flags: string[] = [];
  const names = [
    "通道0截取FIFO溢出", "通道1截取FIFO溢出", "通道2截取FIFO溢出", "通道3截取FIFO溢出",
    "通道0封装FIFO溢出", "通道1封装FIFO溢出", "通道2封装FIFO溢出", "通道3封装FIFO溢出",
  ];
  for (let i = 0; i < 8; i++) {
    if (v & (1 << i)) flags.push(names[i]);
  }
  return flags;
});

/** 触发模式文本 */
function trgModeLabel(v: number | undefined): string {
  if (v === 1) return "外触发";
  if (v === 0) return "内触发";
  return "-";
}

/** 状态标签样式 */
function statusLabel(v: number | undefined): { text: string; cls: string } {
  if (v === 1) return { text: "ON", cls: "status-on" };
  if (v === 0) return { text: "OFF", cls: "status-off" };
  return { text: "-", cls: "" };
}
</script>

<template>
  <div class="status-panel" v-if="props.status">
    <!-- 设备信息 -->
    <section class="status-group">
      <h4>设备信息</h4>
      <div class="status-row">
        <span class="label">设备SN</span>
        <span class="value">{{ props.status.device_sn || "-" }}</span>
      </div>
      <div class="status-row">
        <span class="label">设备类型</span>
        <span class="value">{{ props.status.device_type || "-" }}</span>
      </div>
      <div class="status-row">
        <span class="label">运行时间</span>
        <span class="value">{{ formatUptime(props.status.uptime) }}</span>
      </div>
    </section>

    <!-- 版本 -->
    <section class="status-group">
      <h4>版本信息</h4>
      <div class="status-row">
        <span class="label">PS版本</span>
        <span class="value mono">{{ status.ps_version || "-" }}</span>
      </div>
      <div class="status-row">
        <span class="label">PL版本</span>
        <span class="value mono">{{ status.pl_version || "-" }}</span>
      </div>
    </section>

    <!-- 环境参数 -->
    <section class="status-group">
      <h4>环境参数</h4>
      <div class="status-row">
        <span class="label">板载温度</span>
        <span class="value">{{ fmt(status.i2c_temp, 1) }} °C</span>
      </div>
      <div class="status-row">
        <span class="label">电机转速</span>
        <span class="value">{{ fmtMotorSpeed(status.motor_speed) }}</span>
      </div>
      <div class="status-row">
        <span class="label">触发周期</span>
        <span class="value">{{ fmt(status.trg_cycle, 1) }} Hz</span>
      </div>
      <div class="status-row">
        <span class="label">PPS周期</span>
        <span class="value">{{ fmt(status.pps_cycle, 1) }} Hz</span>
      </div>
      <div class="status-row">
        <span class="label">触发模式</span>
        <span class="value">{{ trgModeLabel(status.trg_mode) }}</span>
      </div>
      <div class="status-row">
        <span class="label">激光频率</span>
        <span class="value">{{ status.laser_freq || 0 }} Hz</span>
      </div>
    </section>

    <!-- ADC 通道 -->
    <section class="status-group">
      <h4>ADC 通道</h4>
      <div v-for="(label, i) in adcChannelLabels" :key="i" class="adc-row">
        <span class="adc-label">{{ label }}</span>
        <div class="adc-bar-wrap">
          <div class="adc-bar" :style="{ width: Math.min((adcValue(i) / 4096) * 100, 100) + '%' }"></div>
        </div>
        <span class="adc-val">{{ adcValue(i) }}</span>
      </div>
    </section>

    <!-- 采集参数 -->
    <section class="status-group">
      <h4>采集参数</h4>
      <div class="status-row">
        <span class="label">采集状态</span>
        <span :class="statusLabel(props.status.preview_enable).cls">
          {{ statusLabel(props.status.preview_enable).text }}
        </span>
      </div>
      <div class="status-row">
        <span class="label">抽样率</span>
        <span class="value">{{ status.preview_coe || 0 }}</span>
      </div>
      <div class="status-row">
        <span class="label">采集长度</span>
        <span class="value">{{ status.wave_len || 0 }}</span>
      </div>
      <div class="status-row">
        <span class="label">第一段</span>
        <span class="value">pos={{ status.first_pos }} len={{ status.first_len }}</span>
      </div>
      <div class="status-row">
        <span class="label">第二段</span>
        <span class="value">pos={{ status.second_pos }} len={{ status.second_len }}</span>
      </div>
    </section>

    <!-- 存储状态 -->
    <section class="status-group">
      <h4>存储状态</h4>
      <div class="status-row">
        <span class="label">存储状态</span>
        <span :class="statusLabel(props.status.ssd_store_enable).cls">
          {{ statusLabel(props.status.ssd_store_enable).text }}
        </span>
      </div>
      <div class="status-row">
        <span class="label">SATA连接</span>
        <span :class="statusLabel(props.status.sata_dev_identify_done).cls">
          {{ props.status.sata_dev_identify_done === 1 ? "已连接" : "未连接" }}
        </span>
      </div>
      <div class="status-row">
        <span class="label">硬盘容量</span>
        <span class="value">{{ formatDiskSize(status.sata_dev_tot_sec_num) }}</span>
      </div>
      <div class="status-row">
        <span class="label">写入位置</span>
        <span class="value">{{ formatDiskSize(status.sata_app_lba_next) }}</span>
      </div>
      <div class="status-row">
        <span class="label">写耗时(8ns)</span>
        <span class="value">{{ status.max_write_time }}</span>
      </div>
      <div class="status-row">
        <span class="label">预览写/读</span>
        <span class="value">{{ status.preview_wr_cnt }} / {{ status.preview_rd_cnt }}</span>
      </div>
    </section>

    <!-- sys_status 标志 -->
    <section class="status-group" v-if="sys0Flags && sys0Flags.length > 0">
      <h4>FIFO 状态</h4>
      <div v-for="flag in sys0Flags" :key="flag" class="fifo-warn">
        ⚠ {{ flag }}
      </div>
    </section>

    <!-- GPS -->
    <section class="status-group">
      <h4>GPS 数据</h4>
      <div class="status-row">
        <span class="label">经度</span>
        <span class="value">{{ status.gps_longitude.toFixed(6) }}°</span>
      </div>
      <div class="status-row">
        <span class="label">纬度</span>
        <span class="value">{{ status.gps_latitude.toFixed(6) }}°</span>
      </div>
      <div class="status-row">
        <span class="label">高度</span>
        <span class="value">{{ status.gps_altitude.toFixed(2) }} m</span>
      </div>
      <div class="status-row">
        <span class="label">方位角</span>
        <span class="value">{{ fmt(status.gps_azimuth, 2) }}°</span>
      </div>
      <div class="status-row">
        <span class="label">俯仰角</span>
        <span class="value">{{ fmt(status.gps_pitch, 2) }}°</span>
      </div>
      <div class="status-row">
        <span class="label">横滚角</span>
        <span class="value">{{ fmt(status.gps_roll, 2) }}°</span>
      </div>
    </section>
  </div>
  <div v-else class="status-empty">
    <p>等待设备连接...</p>
  </div>
</template>

<style scoped>
.status-panel {
  font-size: 12px;
}

.status-group {
  margin-bottom: 14px;
}

.status-group h4 {
  font-size: 11px;
  font-weight: 700;
  color: var(--accent);
  text-transform: uppercase;
  letter-spacing: 1px;
  margin-bottom: 6px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--border-color);
}

.status-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 2px 0;
  gap: 8px;
}

.label {
  color: var(--text-secondary);
  flex-shrink: 0;
}

.value {
  color: var(--text-primary);
  text-align: right;
  word-break: break-all;
}

.mono {
  font-family: "Cascadia Code", "Fira Code", monospace;
  font-size: 11px;
}

/* ADC bar */
.adc-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 1px 0;
}
.adc-label {
  color: var(--text-secondary);
  font-size: 10px;
  width: 64px;
  flex-shrink: 0;
  text-align: right;
}
.adc-bar-wrap {
  flex: 1;
  height: 8px;
  background: var(--bg-primary);
  border-radius: 4px;
  overflow: hidden;
}
.adc-bar {
  height: 100%;
  background: linear-gradient(90deg, var(--accent-dim), var(--accent));
  border-radius: 4px;
  transition: width 0.3s;
}
.adc-val {
  color: var(--text-primary);
  font-size: 10px;
  width: 40px;
  text-align: right;
  font-family: monospace;
}

/* Status indicators */
.status-on {
  color: var(--success);
  font-weight: 600;
}
.status-off {
  color: var(--text-secondary);
}

/* FIFO warnings */
.fifo-warn {
  color: var(--error);
  font-size: 11px;
  padding: 1px 0;
}

/* Empty state */
.status-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--text-secondary);
}
</style>
