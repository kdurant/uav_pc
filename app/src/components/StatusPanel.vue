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

/** 硬盘容量：单位 GB，按 1GB = 1024MB（1024³ 字节）换算 */
function formatDiskSize(sectors: number): string {
  const bytes = sectors * 512;
  return (bytes / 1024 ** 3).toFixed(2) + " GB";
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

/** 获取 ADC 通道原始值 */
function adcValue(ch: number): number {
  if (!props.status) return 0;
  const values = [
    props.status.adc_ch0, props.status.adc_ch1, props.status.adc_ch2, props.status.adc_ch3,
    props.status.adc_ch4, props.status.adc_ch5, props.status.adc_ch6, props.status.adc_ch7,
  ];
  return values[ch] ?? 0;
}

/** ADC 数字值 → 模拟电压：16 位，满量程 5V */
const ADC_FULL_SCALE = 65536;
const ADC_VREF = 5.0;

function adcVoltage(ch: number): number {
  return (adcValue(ch) / ADC_FULL_SCALE) * ADC_VREF;
}

/** APD 高压反馈换算系数：Vhv = Vadc * 293 * 0.798 */
const APD_HV_COEFF = 293 * 0.798;
/** APD 高压反馈满量程电压（用于进度条归一化） */
const APD_HV_FULL = ADC_VREF * APD_HV_COEFF;

/** 板载热敏电阻通道编号（ch6=板载热敏1, ch7=板载热敏2） */
const THERMISTOR_CHANNELS = [6, 7];

/** ADC 通道显示文本：电压反馈显示电压，热敏电阻换算温度 */
function adcText(ch: number): string {
  if (!props.status) return "-";
  if (ch <= 2) return adcVoltage(ch).toFixed(3) + " V"; // PMT1~3 反馈
  if (ch === 3) return (adcVoltage(ch) * APD_HV_COEFF).toFixed(1) + " V"; // APD 高压反馈
  if (THERMISTOR_CHANNELS.includes(ch)) {
    const t = thermistorValues(ch);
    return t ? t.temp.toFixed(2) + " °C" : "开路";
  }
  return String(adcValue(ch)); // 外置热敏1/2（未连接）：保留原始值
}

/**
 * 板载热敏电阻换算（单位：V/Ω/℃）：
 *   Vres = value * 0.0000625 * 1.051
 *   Rres = 10000 / (3.3 - Vres) * Vres
 *   Temp = 1 / (ln(Rres/10000)/3380 + 1/(273.15+25)) - 273.15 + 0.5
 * 电压达到/超过 3.3V（探头开路）时返回 null。
 */
function thermistorValues(ch: number): { vres: number; rres: number; temp: number } | null {
  const value = adcValue(ch);
  const vres = value * 0.0000625 * 1.051;
  const denom = 3.3 - vres;
  if (vres <= 0 || denom <= 0) return null;
  const rres = (10000 / denom) * vres;
  if (!(rres > 0)) return null;
  const temp = 1 / (Math.log(rres / 10000) / 3380 + 1 / (273.15 + 25)) - 273.15 + 0.5;
  if (!Number.isFinite(temp)) return null;
  return { vres, rres, temp };
}

/** 热敏电阻通道悬浮提示：显示 Vres / Rres / Temp */
function adcTitle(ch: number): string | undefined {
  if (!props.status || !THERMISTOR_CHANNELS.includes(ch)) return undefined;
  const value = adcValue(ch);
  const vres = value * 0.0000625 * 1.051;
  const t = thermistorValues(ch);
  if (!t) return `Vres=${vres.toFixed(4)} V（超量程/探头开路）`;
  return `Vres=${t.vres.toFixed(4)} V, Rres=${t.rres.toFixed(0)} Ω, Temp=${t.temp.toFixed(2)} °C`;
}

/** ADC 通道进度条比例(0~1) */
function adcRatio(ch: number): number {
  if (!props.status) return 0;
  if (ch === 3) return Math.min((adcVoltage(ch) * APD_HV_COEFF) / APD_HV_FULL, 1);
  return Math.min(adcValue(ch) / ADC_FULL_SCALE, 1);
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
        <span class="value mono">{{ props.status.ps_version || "-" }}</span>
      </div>
      <div class="status-row">
        <span class="label">PL版本</span>
        <span class="value mono">{{ props.status.pl_version || "-" }}</span>
      </div>
    </section>

    <!-- 环境参数 -->
    <section class="status-group">
      <h4>环境参数</h4>
      <div class="status-row">
        <span class="label">板载温度</span>
        <span class="value">{{ fmt(props.status.i2c_temp, 1) }} °C</span>
      </div>
      <div class="status-row">
        <span class="label">电机转速</span>
        <span class="value">{{ fmtMotorSpeed(props.status.motor_speed) }}</span>
      </div>
      <div class="status-row">
        <span class="label">触发周期</span>
        <span class="value">{{ fmt(props.status.trg_cycle, 1) }} Hz</span>
      </div>
      <div class="status-row">
        <span class="label">PPS周期</span>
        <span class="value">{{ fmt(props.status.pps_cycle, 1) }} Hz</span>
      </div>
      <div class="status-row">
        <span class="label">触发模式</span>
        <span class="value">{{ trgModeLabel(props.status.trg_mode) }}</span>
      </div>
      <div class="status-row">
        <span class="label">激光频率</span>
        <span class="value">{{ props.status.laser_freq || 0 }} Hz</span>
      </div>
    </section>

    <!-- ADC 通道 -->
    <section class="status-group">
      <h4>ADC 通道</h4>
      <div v-for="(label, i) in adcChannelLabels" :key="i" class="adc-row">
        <span class="adc-label">{{ label }}</span>
        <div class="adc-bar-wrap">
          <div class="adc-bar" :style="{ width: adcRatio(i) * 100 + '%' }"></div>
        </div>
        <span class="adc-val" :title="adcTitle(i)">{{ adcText(i) }}</span>
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
        <span class="value">{{ props.status.preview_coe || 0 }}</span>
      </div>
      <div class="status-row">
        <span class="label">采集长度</span>
        <span class="value">{{ props.status.wave_len || 0 }}</span>
      </div>
      <div class="status-row">
        <span class="label">第一段</span>
        <span class="value">pos={{ props.status.first_pos }} len={{ props.status.first_len }}</span>
      </div>
      <div class="status-row">
        <span class="label">第二段</span>
        <span class="value">pos={{ props.status.second_pos }} len={{ props.status.second_len }}</span>
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
        <span class="value">{{ formatDiskSize(props.status.sata_dev_tot_sec_num) }}</span>
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
        <span class="value">{{ props.status.gps_longitude.toFixed(6) }}°</span>
      </div>
      <div class="status-row">
        <span class="label">纬度</span>
        <span class="value">{{ props.status.gps_latitude.toFixed(6) }}°</span>
      </div>
      <div class="status-row">
        <span class="label">高度</span>
        <span class="value">{{ props.status.gps_altitude.toFixed(2) }} m</span>
      </div>
      <div class="status-row">
        <span class="label">方位角</span>
        <span class="value">{{ fmt(props.status.gps_azimuth, 2) }}°</span>
      </div>
      <div class="status-row">
        <span class="label">俯仰角</span>
        <span class="value">{{ fmt(props.status.gps_pitch, 2) }}°</span>
      </div>
      <div class="status-row">
        <span class="label">横滚角</span>
        <span class="value">{{ fmt(props.status.gps_roll, 2) }}°</span>
      </div>
    </section>
  </div>
  <div v-else class="status-empty">
    <p>等待设备连接...</p>
  </div>
</template>

<style scoped>
.status-panel {
  font-size: 14px;
}

.status-group {
  margin-bottom: 16px;
}

.status-group h4 {
  font-size: 12px;
  font-weight: 700;
  color: var(--accent);
  text-transform: uppercase;
  letter-spacing: 1.2px;
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
  font-size: 13px;
}

.value {
  color: var(--text-primary);
  text-align: right;
  word-break: break-all;
  font-variant-numeric: tabular-nums;
}

.mono {
  font-family: "Cascadia Code", "Fira Code", monospace;
  font-size: 13px;
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
  font-size: 12px;
  width: 78px;
  flex-shrink: 0;
  text-align: right;
}
.adc-bar-wrap {
  flex: 1;
  height: 6px;
  background: var(--bg-primary);
  border-radius: 3px;
  overflow: hidden;
}
.adc-bar {
  height: 100%;
  background: linear-gradient(90deg, var(--accent-dim), var(--accent));
  border-radius: 3px;
  transition: width 0.2s ease-out;
}
.adc-val {
  color: var(--text-primary);
  font-size: 12px;
  width: 74px;
  text-align: right;
  font-family: "Cascadia Code", "Fira Code", monospace;
  font-variant-numeric: tabular-nums;
}

/* Status indicators */
.status-on {
  color: var(--success);
  font-weight: 600;
}
.status-off {
  color: var(--text-secondary);
  opacity: 0.7;
}

/* FIFO warnings */
.fifo-warn {
  color: var(--error);
  font-size: 13px;
  padding: 2px 0;
}

/* Empty state */
.status-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--text-secondary);
  font-size: 14px;
  opacity: 0.6;
}
</style>
