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
      <div class="ctrl-row">
        <button v-if="!connected" @click="emit('search')">搜索设备</button>
        <button v-else class="accent" @click="emit('disconnect')">断开连接</button>
      </div>
      <div class="ctrl-row" v-if="connected">
        <label class="checkbox-row">
          <input type="checkbox" v-model="statusPollingEnabled" @change="toggleStatusPolling" />
          <span>读取设备状态</span>
        </label>
      </div>
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

        <div class="param-row">
          <span class="param-label">采样长度</span>
          <input type="number" v-model.number="waveLen" min="1" max="65535" />
        </div>
        <div class="param-row">
          <span class="param-label">采集数据抽样率</span>
          <input type="number" v-model.number="previewCoe" min="1" max="65535" />
        </div>
        <div class="param-row">
          <span class="param-label">第一段提取位置</span>
          <input type="number" v-model.number="firstPos" min="0" max="65535" />
        </div>
        <div class="param-row">
          <span class="param-label">第一段提取长度</span>
          <input type="number" v-model.number="firstLen" min="0" max="65535" />
        </div>
        <div class="param-row">
          <span class="param-label">第二段提取位置</span>
          <input type="number" v-model.number="secondPos" min="0" max="65535" />
        </div>
        <div class="param-row">
          <span class="param-label">第二段提取长度</span>
          <input type="number" v-model.number="secondLen" min="0" max="65535" />
        </div>
        <div class="param-row">
          <span class="param-label">和阈值</span>
          <input type="number" v-model.number="sumLevel" />
        </div>
        <div class="param-row">
          <span class="param-label">值阈值</span>
          <input type="number" v-model.number="minLevel" />
        </div>

        <div class="btn-row">
          <button class="accent" :disabled="settingInProgress" @click="sendAllCaptureParams">
            {{ settingInProgress ? '设置中...' : '设置' }}
          </button>
          <button :class="{ accent: previewEnabled }" @click="togglePreview">
            {{ previewEnabled ? '停止采集' : '开始采集' }}
          </button>
        </div>

        <h4 class="group-title mode-title">信号采集模式</h4>
        <div class="mode-list">
          <label
            v-for="m in trigModes"
            :key="m.value"
            class="switch-item"
          >
            <input
              type="radio"
              name="trgMode"
              :value="m.value"
              v-model.number="trgMode"
              @change="setTrgMode"
            />
            <span class="switch"></span>
            <span class="switch-label">{{ m.label }}</span>
          </label>
        </div>
      </section>
    </div>

    <!-- Tab Content: 电压（扩展板 DA 控制 0x0f01） -->
    <div v-show="activeTab === 'voltage'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">PMT / APD 电压</h4>

        <div class="param-row">
          <span class="param-label">PMT0 电压 (V)</span>
          <input type="number" v-model.number="pmtVoltages[0]" min="0" max="5" step="0.01" />
          <button class="set-btn" :disabled="daSetting !== null" @click="setDaChannel(0)">
            {{ daSetting === 0 ? "..." : "设置" }}
          </button>
        </div>
        <div class="param-row">
          <span class="param-label">PMT1 电压 (V)</span>
          <input type="number" v-model.number="pmtVoltages[1]" min="0" max="5" step="0.01" />
          <button class="set-btn" :disabled="daSetting !== null" @click="setDaChannel(1)">
            {{ daSetting === 1 ? "..." : "设置" }}
          </button>
        </div>
        <div class="param-row">
          <span class="param-label">PMT2 电压 (V)</span>
          <input type="number" v-model.number="pmtVoltages[2]" min="0" max="5" step="0.01" />
          <button class="set-btn" :disabled="daSetting !== null" @click="setDaChannel(2)">
            {{ daSetting === 2 ? "..." : "设置" }}
          </button>
        </div>
        <div class="param-row">
          <span class="param-label">APD 高压 (V)</span>
          <input type="number" v-model.number="apdVoltage" min="140" max="300" step="1" />
          <button class="set-btn" :disabled="daSetting !== null" @click="setDaChannel(3)">
            {{ daSetting === 3 ? "..." : "设置" }}
          </button>
        </div>

        <p class="hint">ch0~ch2 = PMT0~2（0~5V），ch3 = APD 高压（140~300V）</p>
      </section>
    </div>

    <!-- Tab Content: 激光器 -->
    <div v-show="activeTab === 'laser'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">激光器</h4>
        <div class="ctrl-row">
          <label>重频 (Hz)</label>
          <div class="input-row">
            <input type="number" v-model.number="laserFreq" min="1" />
            <button @click="setLaserFreq">设置</button>
          </div>
        </div>
        <div class="ctrl-row">
          <label>激光使能</label>
          <div class="toggle-row">
            <label class="toggle">
              <input type="checkbox" v-model="laserEnabled" @change="toggleLaser" />
              <span class="toggle-label">{{ laserEnabled ? 'ON' : 'OFF' }}</span>
            </label>
          </div>
        </div>
      </section>
    </div>

    <!-- Tab Content: 电机 -->
    <div v-show="activeTab === 'motor'" class="tab-content"></div>

    <!-- Tab Content: 存储 -->
    <div v-show="activeTab === 'storage'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">存储控制</h4>
        <div class="ctrl-row">
          <label>存储开关</label>
          <div class="toggle-row">
            <label class="toggle">
              <input type="checkbox" v-model="storeEnabled" @change="setStoreEnable" />
              <span class="toggle-label">{{ storeEnabled ? 'ON' : 'OFF' }}</span>
            </label>
          </div>
        </div>
        <div class="ctrl-row">
          <label>文件名</label>
          <input type="text" v-model="storeFilename" placeholder="输入文件名..." />
        </div>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">SSD 已存储文件</h4>
        <div class="ctrl-row">
          <button class="accent" :disabled="ssdLoading" @click="refreshSsdFiles">
            {{ ssdLoading ? "检索中..." : "检索文件" }}
          </button>
        </div>
        <p v-if="ssdError" class="ssd-error">{{ ssdError }}</p>
        <div v-if="ssdFiles.length" class="ssd-table-wrap">
          <table class="ssd-table">
            <thead>
              <tr>
                <th class="col-name">文件名</th>
                <th>起始地址</th>
                <th>结束地址</th>
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
        <h4 class="group-title">GPS 数据文件</h4>
        <div class="ctrl-row">
          <button class="accent" :disabled="gpsLoading" @click="refreshGpsFiles">
            {{ gpsLoading ? "检索中..." : "检索文件" }}
          </button>
        </div>
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
  font-size: 14px;
}

.ctrl-group {
  margin-bottom: 14px;
}

.group-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--accent);
  text-transform: uppercase;
  letter-spacing: 1px;
  margin-bottom: 6px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--border-color);
}

.ctrl-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 8px;
}

.ctrl-row label {
  color: var(--text-secondary);
  font-size: 13px;
}

/* 采集参数行：标签在左、输入在右（参考 ui.jpg 布局） */
.param-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.param-label {
  width: 124px;
  flex-shrink: 0;
  padding: 4px 8px;
  font-size: 13px;
  color: var(--text-primary);
  background: var(--accent-glow);
  border-radius: 3px;
  white-space: nowrap;
}
.param-row input {
  flex: 1;
  min-width: 0;
}
.btn-row {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}
.btn-row button {
  flex: 1;
}
/* 每行单独的“设置”按钮 */
.set-btn {
  flex: 0 0 auto;
  padding: 5px 10px;
  font-size: 13px;
}
.hint {
  color: var(--text-secondary);
  font-size: 11px;
  margin: 8px 0 0;
}

/* 信号采集模式：拨动开关（参考 ui.jpg） */
.mode-title {
  margin-top: 16px;
}
.mode-list {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 16px;
}
.switch-item {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;
}
.switch-item input {
  display: none;
}
.switch {
  position: relative;
  width: 40px;
  height: 22px;
  border-radius: 11px;
  background: var(--border-light);
  transition: background 0.2s;
  flex-shrink: 0;
}
.switch::after {
  content: "";
  position: absolute;
  top: 2px;
  left: 2px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
  transition: transform 0.2s;
}
.switch-item input:checked + .switch {
  background: var(--accent);
}
.switch-item input:checked + .switch::after {
  transform: translateX(18px);
}
.switch-label {
  font-size: 13px;
  color: var(--text-primary);
}

.checkbox-row {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.checkbox-row input[type="checkbox"] {
  width: 14px;
  height: 14px;
  cursor: pointer;
}

.checkbox-row span {
  color: var(--text-primary);
  font-size: 14px;
}

/* Tab bar */
.tab-bar {
  display: flex;
  gap: 1px;
  margin: 0 -12px 8px -12px;
  padding: 0 12px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
}
.tab-btn {
  flex: 1;
  padding: 6px 4px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  transition: color 0.15s, border-color 0.15s;
  white-space: nowrap;
  text-align: center;
}
.tab-btn:hover {
  color: var(--text-primary);
}
.tab-btn.active {
  color: var(--accent);
  border-bottom-color: var(--accent);
}
.tab-content {
  /* tab content fills the rest */
}

.input-row {
  display: flex;
  gap: 6px;
  align-items: center;
}

.input-row.gap-2 {
  gap: 4px;
}

.input-row input,
.ctrl-row input,
.ctrl-row select {
  flex: 1;
  min-width: 0;
}

.toggle-row {
  display: flex;
  align-items: center;
}

.toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.toggle input[type="checkbox"] {
  width: 14px;
  height: 14px;
  cursor: pointer;
}

.toggle-label {
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 600;
}

/* SSD 已存储文件列表 */
.ssd-error {
  color: var(--error);
  font-size: 12px;
  margin: 4px 0 0;
}
.ssd-empty {
  color: var(--text-secondary);
  font-size: 12px;
  margin: 6px 0 0;
}
.ssd-unit {
  color: var(--text-secondary);
  font-size: 11px;
  margin: 4px 0 0;
}
.ssd-table-wrap {
  margin-top: 6px;
  max-height: 320px;
  overflow: auto;
  border: 1px solid var(--border-color);
  border-radius: 4px;
}
.ssd-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11px;
}
.ssd-table th,
.ssd-table td {
  padding: 4px 8px;
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
  font-weight: 700;
}
.ssd-table td {
  font-variant-numeric: tabular-nums;
}
.ssd-table .col-name {
  max-width: 190px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ssd-table tbody tr:hover {
  background: var(--accent-glow);
}
</style>
