<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SystemStatus, TemperatureConfig, TemperaturePoint } from "../types";
import TemperatureHistoryDialog from "./TemperatureHistoryDialog.vue";

const props = defineProps<{
  status: SystemStatus | null;
}>();

/** 方便模板安全访问 */
const s = computed(() => props.status);

// ===== 板载温度历史（每 10s 一点，文件持久化） =====
const tempHistory = ref<TemperaturePoint[]>([]);
const tempConfig = ref<TemperatureConfig>({ save_points: 180, display_points: 180 });
const historyVisible = ref(false);
const historyError = ref("");
const TEMP_SAMPLE_MS = 10_000;
let lastTempSampleAt = 0;
let savingHistory = false;

/** 曲线显示最近 display_points 个点 */
const visibleHistory = computed(() => {
  const limit = Math.max(1, tempConfig.value.display_points);
  const len = tempHistory.value.length;
  return len <= limit ? tempHistory.value : tempHistory.value.slice(len - limit);
});

onMounted(async () => {
  try {
    tempConfig.value = await invoke<TemperatureConfig>("get_temperature_config");
    const saved = await invoke<TemperaturePoint[]>("load_temperature_history");
    if (saved.length) {
      tempHistory.value = saved;
      lastTempSampleAt = saved[saved.length - 1].time;
    }
  } catch (e) {
    historyError.value = `温度历史加载失败: ${e}`;
    console.error("load temperature history failed:", e);
  }
});

watch(
  () => props.status,
  async (status) => {
    // 断开连接时保留历史，仅停止采样
    if (!status) return;

    const now = Date.now();
    if (lastTempSampleAt !== 0 && now - lastTempSampleAt < TEMP_SAMPLE_MS) return;
    lastTempSampleAt = now;
    tempHistory.value.push({ time: now, temp: status.i2c_temp });

    const limit = Math.max(1, tempConfig.value.save_points);
    if (tempHistory.value.length > limit) {
      tempHistory.value.splice(0, tempHistory.value.length - limit);
    }

    if (savingHistory) return;
    savingHistory = true;
    try {
      await invoke("save_temperature_history", { points: tempHistory.value });
      historyError.value = "";
    } catch (e) {
      historyError.value = `温度历史保存失败: ${e}`;
      console.error("save temperature history failed:", e);
    } finally {
      savingHistory = false;
    }
  }
);

/** 硬盘容量：单位 GB，按 1GB = 1024MB（1024³ 字节）换算 */
function formatDiskSize(sectors: number | undefined): string {
  if (sectors === undefined) return "-";
  const bytes = sectors * 512;
  return (bytes / 1024 ** 3).toFixed(2) + " GB";
}

/** 固定小数位，无数据时显示 "-" */
function fmtFixed(v: number | undefined, digits: number): string {
  if (v === undefined || v === null) return "-";
  return v.toFixed(digits);
}

/** SATA 连接状态文本 */
function sataText(v: number | undefined): string {
  if (v === undefined) return "-";
  return v === 1 ? "已连接" : "未连接";
}

/** SATA 连接状态标签样式 */
function sataClass(v: number | undefined): string {
  if (v === undefined) return "";
  return v === 1 ? "status-on" : "status-off";
}

/** 状态值格式化，保留 2 位小数 */
function fmt(v: number | undefined, digits = 2): string {
  if (v === undefined || v === null) return "-";
  return Number(v).toFixed(digits);
}

/** 电机转速格式化（后端已按 1e9/(raw*8)*60 换算为 RPM） */
function fmtMotorSpeed(v: number | undefined): string {
  if (v === undefined) return "-";
  return v.toFixed(1);
}

/** 温度通道：ch6/7=板载热敏（外置热敏 ch4/5 不在界面显示） */
const thermistorChannels = [
  { ch: 6, label: "板载热敏1" },
  { ch: 7, label: "板载热敏2" },
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

/** ADC 数字值 → 比例（16 位满量程） */
const ADC_FULL_SCALE = 65536;

/** 热敏电阻通道编号（仅板载热敏 ch6/7） */
const THERMISTOR_CHANNELS = [6, 7];

/** 高温阈值（°C），超过该值固定显示红色 */
const TEMP_HIGH = 65;

/**
 * 温度色阶：0°C 蓝色 → 20°C 绿色 → 40°C 黄绿 → 55°C 橙色 → 65°C 红色。
 * 超过 65°C 固定亮红色。
 */
function tempColor(t: number | null | undefined): string {
  if (t === undefined || t === null || !Number.isFinite(t)) {
    return "var(--text-primary)";
  }
  if (t > TEMP_HIGH) return "#dc2626";
  const clamped = Math.max(0, t);
  // 色相 210°(蓝) → 0°(红)
  const hue = 210 - (clamped / TEMP_HIGH) * 210;
  return `hsl(${hue.toFixed(0)}, 72%, 40%)`;
}

/** 热敏通道温度（开路/超量程返回 null） */
function thermistorTemp(ch: number): number | null {
  const t = thermistorValues(ch);
  return t ? t.temp : null;
}

/** 热敏通道数值颜色：按温度渐变，开路时红色 */
function adcValueColor(ch: number): string | undefined {
  if (!THERMISTOR_CHANNELS.includes(ch)) return undefined;
  const t = thermistorTemp(ch);
  return t === null ? "var(--error)" : tempColor(t);
}

/** 热敏通道进度条颜色：与温度数值保持一致 */
function adcBarColor(ch: number): string | undefined {
  if (!THERMISTOR_CHANNELS.includes(ch)) return undefined;
  const t = thermistorTemp(ch);
  return t === null ? "var(--error)" : tempColor(t);
}

/** 热敏通道显示文本：换算温度，探头开路显示“开路” */
function adcText(ch: number): string {
  if (!props.status) return "-";
  const t = thermistorValues(ch);
  return t ? t.temp.toFixed(2) + " °C" : "开路";
}

/**
 * 热敏电阻换算（外置/板载通用，单位：V/Ω/℃）：
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

/** 热敏通道进度条比例(0~1) */
function adcRatio(ch: number): number {
  if (!props.status) return 0;
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
  <div class="status-panel">
    <div v-if="!s" class="status-offline">设备未连接，以下为占位数据</div>

    <!-- 版本 -->
    <section class="status-group">
      <h4>版本信息</h4>
      <div class="row-list">
        <div class="status-row">
          <span class="label">PS版本</span>
          <span class="value mono">{{ s?.ps_version || "-" }}</span>
        </div>
        <div class="status-row">
          <span class="label">PL版本</span>
          <span class="value mono">{{ s?.pl_version || "-" }}</span>
        </div>
      </div>
    </section>

    <!-- 环境参数 -->
    <section class="status-group">
      <h4>环境参数</h4>
      <div class="metric-grid">
        <div class="metric metric-clickable" title="点击查看最近 30 分钟温度曲线" @click="historyVisible = true">
          <span class="metric-label">板载温度</span>
          <span class="metric-value" :style="{ color: tempColor(s?.i2c_temp) }">
            {{ fmt(s?.i2c_temp, 1) }}<i>°C</i>
          </span>
        </div>
        <div class="metric">
          <span class="metric-label">电机转速</span>
          <span class="metric-value">{{ fmtMotorSpeed(s?.motor_speed) }}<i>RPM</i></span>
        </div>
        <div class="metric">
          <span class="metric-label">触发周期</span>
          <span class="metric-value">{{ fmt(s?.trg_cycle, 1) }}<i>Hz</i></span>
        </div>
        <div class="metric">
          <span class="metric-label">PPS 周期</span>
          <span class="metric-value">{{ fmt(s?.pps_cycle, 1) }}<i>Hz</i></span>
        </div>
      </div>
      <p v-if="historyError" class="history-error">{{ historyError }}</p>
      <div class="row-list">
        <div class="status-row">
          <span class="label">触发模式</span>
          <span class="value">{{ trgModeLabel(s?.trg_mode) }}</span>
        </div>
        <div class="status-row">
          <span class="label">激光频率</span>
          <span class="value mono">{{ s ? s.laser_freq + " Hz" : "-" }}</span>
        </div>
      </div>
    </section>

    <!-- 热敏电阻 -->
    <section class="status-group">
      <h4>热敏电阻</h4>
      <div class="adc-list">
        <div v-for="item in thermistorChannels" :key="item.ch" class="adc-row">
          <span class="adc-label">{{ item.label }}</span>
          <div class="adc-bar-wrap">
            <div
              class="adc-bar"
              :style="{ width: adcRatio(item.ch) * 100 + '%', background: adcBarColor(item.ch) }"
            ></div>
          </div>
          <span class="adc-val" :title="adcTitle(item.ch)" :style="{ color: adcValueColor(item.ch) }">
            {{ adcText(item.ch) }}
          </span>
        </div>
      </div>
    </section>

    <!-- 采集参数 -->
    <section class="status-group">
      <h4>采集</h4>
      <div class="row-list">
        <div class="status-row">
          <span class="label">采集状态</span>
          <span class="badge" :class="statusLabel(s?.preview_enable).cls">
            {{ statusLabel(s?.preview_enable).text }}
          </span>
        </div>
        <div class="status-row">
          <span class="label">抽样率</span>
          <span class="value mono">{{ s?.preview_coe ?? "-" }}</span>
        </div>
        <div class="status-row">
          <span class="label">采集长度</span>
          <span class="value mono">{{ s?.wave_len ?? "-" }}</span>
        </div>
        <div class="status-row">
          <span class="label">第一段</span>
          <span class="value mono">pos {{ s?.first_pos ?? "-" }} · len {{ s?.first_len ?? "-" }}</span>
        </div>
        <div class="status-row">
          <span class="label">第二段</span>
          <span class="value mono">pos {{ s?.second_pos ?? "-" }} · len {{ s?.second_len ?? "-" }}</span>
        </div>
      </div>
    </section>

    <!-- 存储状态 -->
    <section class="status-group">
      <h4>存储状态</h4>
      <div class="row-list">
        <div class="status-row">
          <span class="label">存储状态</span>
          <span class="badge" :class="statusLabel(s?.ssd_store_enable).cls">
            {{ statusLabel(s?.ssd_store_enable).text }}
          </span>
        </div>
        <div class="status-row">
          <span class="label">SATA 连接</span>
          <span class="badge" :class="sataClass(s?.sata_dev_identify_done)">
            {{ sataText(s?.sata_dev_identify_done) }}
          </span>
        </div>
        <div class="status-row">
          <span class="label">硬盘容量</span>
          <span class="value mono">{{ formatDiskSize(s?.sata_dev_tot_sec_num) }}</span>
        </div>
      </div>
    </section>

    <!-- sys_status 标志 -->
    <section class="status-group" v-if="sys0Flags && sys0Flags.length > 0">
      <h4>FIFO 状态</h4>
      <div v-for="flag in sys0Flags" :key="flag" class="fifo-warn">
        <span class="fifo-dot"></span>{{ flag }}
      </div>
    </section>

    <!-- GPS -->
    <section class="status-group">
      <h4>GPS 数据</h4>
      <div class="row-list">
        <div class="status-row">
          <span class="label">经度</span>
          <span class="value mono">{{ fmtFixed(s?.gps_longitude, 6) }}°</span>
        </div>
        <div class="status-row">
          <span class="label">纬度</span>
          <span class="value mono">{{ fmtFixed(s?.gps_latitude, 6) }}°</span>
        </div>
        <div class="status-row">
          <span class="label">高度</span>
          <span class="value mono">{{ fmtFixed(s?.gps_altitude, 2) }} m</span>
        </div>
      </div>
      <div class="attitude-grid">
        <div class="attitude">
          <span class="metric-label">方位角</span>
          <span class="attitude-value">{{ fmt(s?.gps_azimuth, 2) }}<i>°</i></span>
        </div>
        <div class="attitude">
          <span class="metric-label">俯仰角</span>
          <span class="attitude-value">{{ fmt(s?.gps_pitch, 2) }}<i>°</i></span>
        </div>
        <div class="attitude">
          <span class="metric-label">横滚角</span>
          <span class="attitude-value">{{ fmt(s?.gps_roll, 2) }}<i>°</i></span>
        </div>
      </div>
    </section>

    <TemperatureHistoryDialog
      :visible="historyVisible"
      :points="visibleHistory"
      @close="historyVisible = false"
    />
  </div>
</template>

<style scoped>
.status-panel {
  font-size: 13px;
}

.status-offline {
  margin-bottom: 14px;
  padding: 8px 10px;
  background: var(--bg-inset);
  border: 1px dashed var(--border-color);
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  font-size: 11.5px;
  text-align: center;
}

.status-group {
  margin-bottom: 14px;
}

.status-group:last-child {
  margin-bottom: 4px;
}

.status-group h4 {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 11px;
  font-weight: 650;
  color: var(--text-tertiary);
  letter-spacing: 0.09em;
  text-transform: uppercase;
  margin-bottom: 8px;
}

.status-group h4::after {
  content: "";
  flex: 1;
  height: 1px;
  background: var(--border-color);
}

.row-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.status-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  padding: 5px 7px;
  margin: 0 -7px;
  border-radius: var(--radius-xs);
  transition: background 0.14s ease;
}

.status-row:hover {
  background: var(--bg-inset);
}

.label {
  color: var(--text-secondary);
  flex-shrink: 0;
  font-size: 12.5px;
}

.value {
  color: var(--text-primary);
  text-align: right;
  word-break: break-all;
  font-size: 12.5px;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}

.mono {
  font-family: var(--font-mono);
  font-size: 12px;
}

/* 指标磁贴 */
.metric-grid {  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-bottom: 6px;
}

.metric {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 8px 10px;
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.metric-clickable {
  cursor: pointer;
  transition: border-color 0.16s ease, box-shadow 0.16s ease;
}

.metric-clickable:hover {
  border-color: #bcd0f8;
  box-shadow: 0 2px 8px rgba(37, 99, 235, 0.08);
}

.history-error {
  margin-top: 6px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  background: var(--error-soft);
  color: #b42318;
  font-size: 11.5px;
  word-break: break-all;
}

.metric-label {
  font-size: 11px;
  color: var(--text-tertiary);
  font-weight: 500;
  letter-spacing: 0.01em;
}

.metric-value {
  font-size: 16px;
  font-weight: 650;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
  line-height: 1.2;
  white-space: nowrap;
}

.metric-value i {
  font-style: normal;
  font-size: 10.5px;
  font-weight: 500;
  color: var(--text-tertiary);
  margin-left: 3px;
}

/* ADC */
.adc-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.adc-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.adc-label {
  color: var(--text-secondary);
  font-size: 11.5px;
  width: 74px;
  flex-shrink: 0;
}

.adc-bar-wrap {
  flex: 1;
  height: 6px;
  background: #eef1f6;
  border-radius: 999px;
  overflow: hidden;
  min-width: 0;
}

.adc-bar {
  height: 100%;
  background: linear-gradient(90deg, #6ea0ff, var(--accent));
  border-radius: 999px;
  transition: width 0.24s ease-out;
}

.adc-val {
  color: var(--text-primary);
  font-size: 11.5px;
  width: 80px;
  text-align: right;
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

/* 状态徽章 */
.badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 40px;
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 650;
  letter-spacing: 0.02em;
  line-height: 1.7;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
}

.status-on {
  background: var(--success-soft);
  color: #0e6b3c;
}

.status-off {
  background: var(--bg-tertiary);
  color: var(--text-tertiary);
}

/* 姿态角 */
.attitude-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin-top: 8px;
}

.attitude {
  display: flex;
  flex-direction: column;
  gap: 2px;
  align-items: center;
  padding: 7px 4px;
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.attitude-value {
  font-size: 13.5px;
  font-weight: 650;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.attitude-value i {
  font-style: normal;
  font-size: 10px;
  color: var(--text-tertiary);
  margin-left: 1px;
}

/* FIFO 告警 */
.fifo-warn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  margin-bottom: 6px;
  background: var(--error-soft);
  border: 1px solid #f7d4d4;
  border-radius: var(--radius-sm);
  color: #b42318;
  font-size: 12px;
  font-weight: 500;
}

.fifo-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #e5484d;
  flex-shrink: 0;
}
</style>
