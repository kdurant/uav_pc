<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SsdFile } from "../types";

/** 通用命令按钮 Props */
const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
}>();

// ===== 设备连接 =====
const emit = defineEmits<{
  (e: "search"): void;
  (e: "disconnect"): void;
  (e: "toggle-status", enable: boolean): void;
  (e: "tab-change", tab: string): void;
}>();

const statusPollingEnabled = ref(true);

function toggleStatusPolling() {
  emit("toggle-status", statusPollingEnabled.value);
}

// ===== 激光器控制 =====
const laserFreq = ref(1000);
const laserEnabled = ref(false);

async function setLaserFreq() {
  if (!props.connected || !props.deviceIp) return;
  await sendParam(0x0201, laserFreq.value);
}
async function toggleLaser() {
  if (!props.connected || !props.deviceIp) return;
  await sendParam(0x0202, laserEnabled.value ? 1 : 0);
}

// ===== 采集控制 =====
const previewEnabled = ref(false);
const previewCoe = ref(1000);
const waveLen = ref(6000);
const firstPos = ref(100);
const firstLen = ref(200);
const secondPos = ref(400);
const secondLen = ref(400);
const sumLevel = ref(0);
const minLevel = ref(0);

const settingInProgress = ref(false);

async function setPreviewEnable() {
  if (!props.connected || !props.deviceIp) return;
  try {
    await invoke("start_preview", { deviceIp: props.deviceIp, enable: previewEnabled.value });
  } catch (e) {
    console.error("start_preview failed:", e);
  }
}

/** 采集开关按键：切换后立即下发 */
function togglePreview() {
  if (!props.connected || !props.deviceIp) return;
  previewEnabled.value = !previewEnabled.value;
  setPreviewEnable();
}

async function sendAllCaptureParams() {
  if (!props.connected || !props.deviceIp) return;
  settingInProgress.value = true;

  // 参考通道由 config.toml 的 [capture].ref_channal 指定，界面不显示，由后端下发
  try {
    await invoke("send_ref_channel", { deviceIp: props.deviceIp });
  } catch (e) {
    console.error("send_ref_channel failed:", e);
  }

  const batch: [number, number][] = [
    [0x0301, previewCoe.value],
    [0x0304, waveLen.value],
    [0x0305, firstPos.value],
    [0x0306, firstLen.value],
    [0x0307, secondPos.value],
    [0x0308, secondLen.value],
    [0x0309, sumLevel.value],
    [0x0310, minLevel.value],
  ];
  for (const [cmd, value] of batch) {
    const buf = new ArrayBuffer(4);
    const view = new DataView(buf);
    view.setUint32(0, value, false);
    const data = Array.from(new Uint8Array(buf));
    try {
      await invoke("send_command", { deviceIp: props.deviceIp, cmd, data });
    } catch (e) {
      console.error(`send_command 0x${cmd.toString(16)} failed:`, e);
    }
  }
  settingInProgress.value = false;
}

async function sendParam(cmd: number, value: number) {
  if (!props.connected || !props.deviceIp) return;
  const buf = new ArrayBuffer(4);
  const view = new DataView(buf);
  view.setUint32(0, value, false);
  const data = Array.from(new Uint8Array(buf));
  try {
    await invoke("send_command", { deviceIp: props.deviceIp, cmd, data });
  } catch (e) {
    console.error(`send_command 0x${cmd.toString(16)} failed:`, e);
  }
}

// ===== 存储控制 =====
const storeEnabled = ref(false);
const storeFilename = ref("");

async function setStoreEnable() {
  if (!props.connected || !props.deviceIp) return;
  await sendParam(0x0406, storeEnabled.value ? 1 : 0);
}

// ===== SSD 已存储文件检索 =====
const ssdFiles = ref<SsdFile[]>([]);
const ssdLoading = ref(false);
const ssdError = ref("");

async function refreshSsdFiles() {
  if (!props.connected || !props.deviceIp) return;
  ssdLoading.value = true;
  ssdError.value = "";
  try {
    ssdFiles.value = await invoke<SsdFile[]>("list_ssd_files", {
      deviceIp: props.deviceIp,
    });
  } catch (e) {
    ssdError.value = String(e);
    console.error("list_ssd_files failed:", e);
  } finally {
    ssdLoading.value = false;
  }
}

/** 字节数格式化为易读单位 */
function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return (bytes / 1024 ** 3).toFixed(2) + " GB";
  if (bytes >= 1024 ** 2) return (bytes / 1024 ** 2).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
  return bytes + " B";
}

// ===== GPS 数据文件列表（0x0110 系统指令 + ls） =====
const gpsFiles = ref<string[]>([]);
const gpsLoading = ref(false);
const gpsError = ref("");
const gpsQueried = ref(false);

async function refreshGpsFiles() {
  if (!props.connected || !props.deviceIp) return;
  gpsLoading.value = true;
  gpsError.value = "";
  try {
    gpsFiles.value = await invoke<string[]>("query_gps_files", {
      deviceIp: props.deviceIp,
    });
    gpsQueried.value = true;
  } catch (e) {
    gpsError.value = String(e);
    console.error("query_gps_files failed:", e);
  } finally {
    gpsLoading.value = false;
  }
}

// ===== 信号采集模式 (0x0f03) =====
const trgMode = ref(0);
const trigModes = [
  { value: 0, label: "内触发" },
  { value: 1, label: "外触发" },
  { value: 2, label: "PIN触发" },
];

async function setTrgMode() {
  if (!props.connected || !props.deviceIp) return;
  await sendParam(0x0f03, trgMode.value);
}

// ===== 扩展板 DA 控制 (0x0f01)：PMT0/1/2 电压、APD 高压 =====
const pmtVoltages = ref<number[]>([0, 0, 0]); // PMT0/PMT1/PMT2，0~5V
const apdVoltage = ref(270); // APD 高压，范围 (140, 300) V
const daSetting = ref<number | null>(null); // 正在下发的通道号，null 表示空闲

/** PMT 电压 → DA 数字值：65535 * V / 5（V ≤ 5） */
function pmtToDigital(v: number): number {
  const volt = Math.min(Math.max(v || 0, 0), 5);
  return Math.round((65535 * volt) / 5);
}

/** APD 高压 → DA 数字值：(V/270)/5 * 65535（V ∈ (140, 300)） */
function apdToDigital(v: number): number {
  const volt = Math.min(Math.max(v || 0, 140), 300);
  return Math.round((volt / 270 / 5) * 65535);
}

/** 下发一路 DA：data[31:16]=通道号(0-7)，data[15:0]=DA 数字值（大端） */
async function sendDaChannel(channel: number, digital: number) {
  if (!props.connected || !props.deviceIp) return;
  const word = (((channel & 0xffff) << 16) | (digital & 0xffff)) >>> 0;
  const buf = new ArrayBuffer(4);
  new DataView(buf).setUint32(0, word, false);
  await invoke("send_command", {
    deviceIp: props.deviceIp,
    cmd: 0x0f01,
    data: Array.from(new Uint8Array(buf)),
  });
}

/** 单独设置一路电压：ch0~ch2 = PMT0~2，ch3 = APD 高压 */
async function setDaChannel(channel: number) {
  if (!props.connected || !props.deviceIp) return;
  daSetting.value = channel;
  try {
    const digital = channel === 3
      ? apdToDigital(apdVoltage.value)
      : pmtToDigital(pmtVoltages.value[channel]);
    await sendDaChannel(channel, digital);
  } catch (e) {
    console.error(`set DA channel ${channel} failed:`, e);
  } finally {
    daSetting.value = null;
  }
}

// ===== Tab =====
const activeTab = ref("capture");
const tabs = [
  { key: "capture", label: "采集参数" },
  { key: "voltage", label: "电压" },
  { key: "laser", label: "激光器" },
  { key: "motor", label: "电机" },
  { key: "storage", label: "存储" },
];

function selectTab(key: string) {
  activeTab.value = key;
  emit("tab-change", key);
}
</script>

<template>
  <div class="control-panel">
    <!-- 设备连接组 — 始终可见 -->
    <section class="ctrl-group">
      <h4 class="group-title">设备连接</h4>
      <button v-if="!connected" class="accent block" @click="emit('search')">搜索设备</button>
      <template v-else>
        <button class="danger block" @click="emit('disconnect')">断开连接</button>
        <div class="switch-row">
          <span class="switch-row-label">读取设备状态</span>
          <label class="switch">
            <input type="checkbox" v-model="statusPollingEnabled" @change="toggleStatusPolling" />
            <span class="switch-track"><span class="switch-thumb"></span></span>
          </label>
        </div>
      </template>
    </section>

    <!-- Tab Bar -->
    <div class="tab-bar">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        class="tab-btn"
        :class="{ active: activeTab === tab.key }"
        @click="selectTab(tab.key)"
      >{{ tab.label }}</button>
    </div>

    <!-- Tab Content: 采集参数 -->
    <div v-show="activeTab === 'capture'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">实时采集参数</h4>

        <div class="field-list">
          <label class="field">
            <span class="field-label">采样长度</span>
            <input type="number" v-model.number="waveLen" min="1" max="65535" />
          </label>
          <label class="field">
            <span class="field-label">抽样率</span>
            <input type="number" v-model.number="previewCoe" min="1" max="65535" />
          </label>
          <label class="field">
            <span class="field-label">第一段位置</span>
            <input type="number" v-model.number="firstPos" min="0" max="65535" />
          </label>
          <label class="field">
            <span class="field-label">第一段长度</span>
            <input type="number" v-model.number="firstLen" min="0" max="65535" />
          </label>
          <label class="field">
            <span class="field-label">第二段位置</span>
            <input type="number" v-model.number="secondPos" min="0" max="65535" />
          </label>
          <label class="field">
            <span class="field-label">第二段长度</span>
            <input type="number" v-model.number="secondLen" min="0" max="65535" />
          </label>
          <label class="field">
            <span class="field-label">和阈值</span>
            <input type="number" v-model.number="sumLevel" />
          </label>
          <label class="field">
            <span class="field-label">值阈值</span>
            <input type="number" v-model.number="minLevel" />
          </label>
        </div>

        <div class="btn-row">
          <button class="accent" :disabled="settingInProgress" @click="sendAllCaptureParams">
            {{ settingInProgress ? '设置中...' : '应用参数' }}
          </button>
          <button :class="{ accent: previewEnabled }" @click="togglePreview">
            {{ previewEnabled ? '停止采集' : '开始采集' }}
          </button>
        </div>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">信号采集模式</h4>
        <div class="mode-grid">
          <label
            v-for="m in trigModes"
            :key="m.value"
            class="mode-pill"
            :class="{ active: trgMode === m.value }"
          >
            <input
              type="radio"
              name="trgMode"
              :value="m.value"
              v-model.number="trgMode"
              @change="setTrgMode"
            />
            <span>{{ m.label }}</span>
          </label>
        </div>
      </section>
    </div>

    <!-- Tab Content: 电压（扩展板 DA 控制 0x0f01） -->
    <div v-show="activeTab === 'voltage'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">PMT / APD 电压</h4>

        <div class="channel-card">
          <div class="channel-head">
            <span class="channel-name"><span class="ch-dot pmt"></span>PMT0</span>
            <span class="channel-range">0 – 5 V</span>
          </div>
          <div class="channel-body">
            <input type="number" v-model.number="pmtVoltages[0]" min="0" max="5" step="0.01" />
            <button class="set-btn" :disabled="daSetting !== null" @click="setDaChannel(0)">
              {{ daSetting === 0 ? "..." : "设置" }}
            </button>
          </div>
        </div>

        <div class="channel-card">
          <div class="channel-head">
            <span class="channel-name"><span class="ch-dot pmt"></span>PMT1</span>
            <span class="channel-range">0 – 5 V</span>
          </div>
          <div class="channel-body">
            <input type="number" v-model.number="pmtVoltages[1]" min="0" max="5" step="0.01" />
            <button class="set-btn" :disabled="daSetting !== null" @click="setDaChannel(1)">
              {{ daSetting === 1 ? "..." : "设置" }}
            </button>
          </div>
        </div>

        <div class="channel-card">
          <div class="channel-head">
            <span class="channel-name"><span class="ch-dot pmt"></span>PMT2</span>
            <span class="channel-range">0 – 5 V</span>
          </div>
          <div class="channel-body">
            <input type="number" v-model.number="pmtVoltages[2]" min="0" max="5" step="0.01" />
            <button class="set-btn" :disabled="daSetting !== null" @click="setDaChannel(2)">
              {{ daSetting === 2 ? "..." : "设置" }}
            </button>
          </div>
        </div>

        <div class="channel-card accent-card">
          <div class="channel-head">
            <span class="channel-name"><span class="ch-dot apd"></span>APD 高压</span>
            <span class="channel-range">140 – 300 V</span>
          </div>
          <div class="channel-body">
            <input type="number" v-model.number="apdVoltage" min="140" max="300" step="1" />
            <button class="set-btn" :disabled="daSetting !== null" @click="setDaChannel(3)">
              {{ daSetting === 3 ? "..." : "设置" }}
            </button>
          </div>
        </div>

        <p class="hint">ch0~ch2 = PMT0~2（0~5V），ch3 = APD 高压（140~300V）</p>
      </section>
    </div>

    <!-- Tab Content: 激光器 -->
    <div v-show="activeTab === 'laser'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">激光器</h4>
        <div class="field-list">
          <div class="field">
            <span class="field-label">重频 (Hz)</span>
            <div class="field-inline">
              <input type="number" v-model.number="laserFreq" min="1" />
              <button class="set-btn" @click="setLaserFreq">设置</button>
            </div>
          </div>
        </div>

        <div class="switch-row card-row">
          <span class="switch-row-label">
            激光使能
            <i class="switch-hint">{{ laserEnabled ? '已开启' : '已关闭' }}</i>
          </span>
          <label class="switch">
            <input type="checkbox" v-model="laserEnabled" @change="toggleLaser" />
            <span class="switch-track"><span class="switch-thumb"></span></span>
          </label>
        </div>
      </section>
    </div>

    <!-- Tab Content: 电机 -->
    <div v-show="activeTab === 'motor'" class="tab-content">
      <div class="empty-hint">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"
            stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3.2" />
            <path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1" />
          </svg>
        </div>
        <p class="empty-title">电机控制</p>
        <p class="empty-desc">功能开发中，暂无可调参数</p>
      </div>
    </div>

    <!-- Tab Content: 存储 -->
    <div v-show="activeTab === 'storage'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">存储控制</h4>
        <div class="switch-row card-row">
          <span class="switch-row-label">
            存储开关
            <i class="switch-hint">{{ storeEnabled ? '写入中' : '已关闭' }}</i>
          </span>
          <label class="switch">
            <input type="checkbox" v-model="storeEnabled" @change="setStoreEnable" />
            <span class="switch-track"><span class="switch-thumb"></span></span>
          </label>
        </div>
        <label class="field block-field">
          <span class="field-label">文件名</span>
          <input type="text" v-model="storeFilename" placeholder="输入文件名..." />
        </label>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">
          SSD 已存储文件
          <span v-if="ssdFiles.length" class="count-badge">{{ ssdFiles.length }}</span>
        </h4>
        <button class="accent block" :disabled="ssdLoading" @click="refreshSsdFiles">
          {{ ssdLoading ? "检索中..." : "检索文件" }}
        </button>
        <p v-if="ssdError" class="ssd-error">{{ ssdError }}</p>
        <div v-if="ssdFiles.length" class="ssd-table-wrap">
          <table class="ssd-table">
            <thead>
              <tr>
                <th class="col-name">文件名</th>
                <th>起始</th>
                <th>结束</th>
                <th>大小</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="f in ssdFiles" :key="f.start + '-' + f.name">
                <td class="col-name" :title="f.name">{{ f.name }}</td>
                <td>{{ f.start }}</td>
                <td>{{ f.end }}</td>
                <td>{{ formatSize(f.size) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-else-if="!ssdLoading && !ssdError" class="ssd-empty">暂无文件，点击“检索文件”</p>
        <p v-if="ssdFiles.length" class="ssd-unit">地址单位：扇区（512 B）</p>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">
          GPS 数据文件
          <span v-if="gpsFiles.length" class="count-badge">{{ gpsFiles.length }}</span>
        </h4>
        <button class="accent block" :disabled="gpsLoading" @click="refreshGpsFiles">
          {{ gpsLoading ? "检索中..." : "检索文件" }}
        </button>
        <p v-if="gpsError" class="ssd-error">{{ gpsError }}</p>
        <div v-if="gpsFiles.length" class="ssd-table-wrap">
          <table class="ssd-table">
            <thead>
              <tr>
                <th class="col-name">文件名</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="f in gpsFiles" :key="f">
                <td class="col-name" :title="f">{{ f }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-else-if="gpsQueried && !gpsLoading && !gpsError" class="ssd-empty">
          无文件（或目录不存在）
        </p>
        <p class="ssd-unit">路径：/run/media/mmcblk1p1/（可在 config.toml [gps].dir 修改）</p>
      </section>
    </div>
  </div>
</template>

<style scoped>
.control-panel {
  font-size: 13px;
}

/* ---------- 分组 ---------- */
.ctrl-group {
  margin-bottom: 18px;
}

.ctrl-group:last-child {
  margin-bottom: 4px;
}

.group-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: 650;
  color: var(--text-tertiary);
  letter-spacing: 0.09em;
  text-transform: uppercase;
  margin-bottom: 9px;
}

.group-title::after {
  content: "";
  flex: 1;
  height: 1px;
  background: var(--border-color);
}

.group-title .count-badge {
  font-size: 10px;
  font-weight: 650;
  letter-spacing: 0;
  color: var(--accent-dim);
  background: var(--accent-soft);
  border-radius: 999px;
  padding: 1px 7px;
}

/* ---------- 通用按钮 ---------- */
.block {
  width: 100%;
}

.btn-row {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.btn-row button {
  flex: 1;
}

.set-btn {
  height: 32px;
  padding: 0 12px;
  font-size: 12.5px;
  flex-shrink: 0;
}

/* ---------- Tab 分段控件 ---------- */
.tab-bar {
  display: flex;
  gap: 2px;
  padding: 3px;
  margin-bottom: 14px;
  background: var(--bg-tertiary);
  border-radius: var(--radius);
  border: 1px solid var(--border-color);
}

.tab-btn {
  flex: 1;
  height: 27px;
  padding: 0;
  font-size: 11.5px;
  font-weight: 550;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-radius: 7px;
  box-shadow: none;
  transition: background 0.16s ease, color 0.16s ease, box-shadow 0.16s ease;
}

.tab-btn:hover {
  background: rgba(255, 255, 255, 0.7);
  color: var(--text-primary);
  border-color: transparent;
}

.tab-btn.active {
  background: #fff;
  color: var(--accent-dim);
  font-weight: 650;
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.08);
}

/* ---------- 表单行 ---------- */
.field-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field {
  display: flex;
  align-items: center;
  gap: 8px;
}

.field-label {
  width: 84px;
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}

.field input {
  flex: 1;
  min-width: 0;
  height: 30px;
  font-size: 12.5px;
}

.field-inline {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  min-width: 0;
}

.block-field {
  margin-top: 8px;
}

/* ---------- 开关 ---------- */
.switch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-top: 10px;
}

.switch-row-label {
  font-size: 12.5px;
  color: var(--text-secondary);
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
}

.switch-hint {
  font-style: normal;
  font-size: 11px;
  color: var(--text-tertiary);
}

.card-row {
  margin-top: 0;
  padding: 9px 11px;
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.switch {
  position: relative;
  display: inline-flex;
  width: 36px;
  height: 20px;
  flex-shrink: 0;
  cursor: pointer;
}

.switch input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  margin: 0;
  opacity: 0;
  cursor: pointer;
  z-index: 1;
}

.switch-track {
  width: 36px;
  height: 20px;
  border-radius: 999px;
  background: #d7dee9;
  transition: background 0.2s ease;
}

.switch-thumb {
  display: block;
  width: 16px;
  height: 16px;
  margin: 2px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.28);
  transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.switch input:checked + .switch-track {
  background: var(--accent);
}

.switch input:checked + .switch-track .switch-thumb {
  transform: translateX(16px);
}

.switch input:focus-visible + .switch-track {
  box-shadow: 0 0 0 3px var(--accent-glow);
}

/* ---------- 采集模式 ---------- */
.mode-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
}

.mode-pill {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 34px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: #fff;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
  transition: all 0.16s ease;
  overflow: hidden;
}

.mode-pill:hover {
  border-color: var(--border-strong);
  color: var(--text-primary);
}

.mode-pill input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.mode-pill.active {
  background: var(--accent-soft);
  border-color: #b9cffb;
  color: var(--accent-dim);
  font-weight: 650;
  box-shadow: inset 0 0 0 1px rgba(37, 99, 235, 0.14);
}

/* ---------- 电压通道卡片 ---------- */
.channel-card {
  padding: 9px 11px;
  margin-bottom: 8px;
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  transition: border-color 0.16s ease, box-shadow 0.16s ease;
}

.channel-card:hover {
  border-color: var(--border-light);
  box-shadow: var(--shadow-xs);
}

.channel-card.accent-card {
  background: #fff8f0;
  border-color: #f6e0c6;
}

.channel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 7px;
}

.channel-name {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.ch-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.ch-dot.pmt {
  background: var(--accent);
}

.ch-dot.apd {
  background: #e0881a;
}

.channel-range {
  font-size: 10.5px;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.channel-body {
  display: flex;
  gap: 6px;
}

.channel-body input {
  flex: 1;
  min-width: 0;
  height: 30px;
  font-size: 12.5px;
}

.hint {
  color: var(--text-tertiary);
  font-size: 11px;
  line-height: 1.6;
  margin-top: 10px;
}

/* ---------- 空状态 ---------- */
.empty-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 48px 16px;
  text-align: center;
}

.empty-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  margin-bottom: 4px;
  border-radius: 12px;
  background: var(--bg-tertiary);
  color: var(--text-tertiary);
}

.empty-icon svg {
  width: 22px;
  height: 22px;
}

.empty-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
}

.empty-desc {
  font-size: 11.5px;
  color: var(--text-tertiary);
}

/* ---------- 文件表格 ---------- */
.ssd-error {
  color: var(--error);
  font-size: 11.5px;
  margin-top: 8px;
  padding: 6px 9px;
  background: var(--error-soft);
  border-radius: var(--radius-xs);
}

.ssd-empty {
  color: var(--text-tertiary);
  font-size: 11.5px;
  margin-top: 10px;
  padding: 10px;
  text-align: center;
  border: 1px dashed var(--border-color);
  border-radius: var(--radius-sm);
}

.ssd-unit {
  color: var(--text-tertiary);
  font-size: 10.5px;
  margin-top: 6px;
  line-height: 1.5;
}

.ssd-table-wrap {
  margin-top: 9px;
  max-height: 300px;
  overflow: auto;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: #fff;
}

.ssd-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11px;
}

.ssd-table th,
.ssd-table td {
  padding: 6px 8px;
  text-align: left;
  white-space: nowrap;
  border-bottom: 1px solid var(--border-color);
  color: var(--text-primary);
}

.ssd-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  font-weight: 650;
  font-size: 10.5px;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.ssd-table td {
  font-variant-numeric: tabular-nums;
  font-family: var(--font-mono);
}

.ssd-table tbody tr:last-child td {
  border-bottom: none;
}

.ssd-table tbody tr:hover {
  background: var(--bg-inset);
}

.ssd-table .col-name {
  max-width: 170px;
  overflow: hidden;
  text-overflow: ellipsis;
  font-family: inherit;
}
</style>
