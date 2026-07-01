<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** 通用命令按钮 Props */
const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
  channelVisible?: boolean[];
}>();

// ===== 设备连接 =====
const emit = defineEmits<{
  (e: "search"): void;
  (e: "disconnect"): void;
  (e: "toggle-status", enable: boolean): void;
  (e: "update:channelVisible", value: boolean[]): void;
}>();

const statusPollingEnabled = ref(true);

function toggleStatusPolling() {
  emit("toggle-status", statusPollingEnabled.value);
}

function toggleCh(ch: number) {
  const arr = [...(props.channelVisible ?? [true, true, true, true])];
  arr[ch] = !arr[ch];
  emit("update:channelVisible", arr);
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
const refChannel = ref(1);
const saveChannel = ref(0xff);
const waveLen = ref(6000);
const firstPos = ref(100);
const firstLen = ref(200);
const secondPos = ref(400);
const secondLen = ref(400);
const sumLevel = ref(0);
const minLevel = ref(0);
const pinThreshold = ref(1000);

const settingInProgress = ref(false);

async function setPreviewEnable() {
  if (!props.connected || !props.deviceIp) return;
  try {
    await invoke("start_preview", { deviceIp: props.deviceIp, enable: previewEnabled.value });
  } catch (e) {
    console.error("start_preview failed:", e);
  }
}

async function sendAllCaptureParams() {
  if (!props.connected || !props.deviceIp) return;
  settingInProgress.value = true;
  const batch: [number, number][] = [
    [0x0301, previewCoe.value],
    [0x0302, refChannel.value],
    [0x0303, saveChannel.value],
    [0x0304, waveLen.value],
    [0x0305, firstPos.value],
    [0x0306, firstLen.value],
    [0x0307, secondPos.value],
    [0x0308, secondLen.value],
    [0x0309, sumLevel.value],
    [0x0310, minLevel.value],
    [0x0321, pinThreshold.value],
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

// ===== 电机控制 =====
const trgMode = ref(0);

async function setTrgMode() {
  if (!props.connected || !props.deviceIp) return;
  await sendParam(0x0f03, trgMode.value);
}

// ===== Tab =====
const activeTab = ref("capture");
const tabs = [
  { key: "capture", label: "采集参数" },
  { key: "laser", label: "激光器" },
  { key: "motor", label: "电机&触发" },
  { key: "storage", label: "存储" },
];
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

    <!-- 通道可见性 — 始终可见 -->
    <section class="ctrl-group" v-if="connected">
      <h4 class="group-title">显示通道</h4>
      <div class="ch-vis-row">
        <label v-for="ch in 4" :key="ch" class="ch-cb" :style="{ '--ch-color': ['#00d4ff','#ff6b6b','#ffd93d','#6bcb77'][ch-1] }">
          <input type="checkbox" :checked="props.channelVisible?.[ch-1] ?? true"
            @change="toggleCh(ch-1)" />
          <span>CH{{ ch-1 }}</span>
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
        @click="activeTab = tab.key"
      >{{ tab.label }}</button>
    </div>

    <!-- Tab Content: 采集参数 -->
    <div v-show="activeTab === 'capture'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">采集控制</h4>
        <div class="ctrl-row">
          <label>采集开关</label>
          <div class="toggle-row">
            <label class="toggle">
              <input type="checkbox" v-model="previewEnabled" @change="setPreviewEnable" />
              <span class="toggle-label">{{ previewEnabled ? '开始采集' : '停止采集' }}</span>
            </label>
          </div>
        </div>
        <div class="ctrl-row">
          <label>抽样率</label>
          <input type="number" v-model.number="previewCoe" min="1" max="65535" style="width:80px" />
        </div>
        <div class="ctrl-row">
          <label>参考通道</label>
          <select v-model.number="refChannel" style="width:80px">
            <option :value="0">通道0</option>
            <option :value="1">通道1</option>
            <option :value="2">通道2</option>
            <option :value="3">通道3</option>
            <option :value="4">任意通道</option>
          </select>
        </div>
        <div class="ctrl-row">
          <label>上传通道</label>
          <input type="number" v-model.number="saveChannel" min="0" max="255" style="width:80px" />
        </div>
        <div class="ctrl-row">
          <label>采集长度</label>
          <input type="number" v-model.number="waveLen" min="1" max="65535" style="width:80px" />
        </div>
        <div class="ctrl-row">
          <label>第一段</label>
          <div class="input-row gap-2">
            <input type="number" v-model.number="firstPos" placeholder="pos" style="width:55px" />
            <input type="number" v-model.number="firstLen" placeholder="len" style="width:55px" />
          </div>
        </div>
        <div class="ctrl-row">
          <label>第二段</label>
          <div class="input-row gap-2">
            <input type="number" v-model.number="secondPos" placeholder="pos" style="width:55px" />
            <input type="number" v-model.number="secondLen" placeholder="len" style="width:55px" />
          </div>
        </div>
        <div class="ctrl-row">
          <label>和阈值</label>
          <input type="number" v-model.number="sumLevel" style="width:80px" />
        </div>
        <div class="ctrl-row">
          <label>值阈值</label>
          <input type="number" v-model.number="minLevel" style="width:80px" />
        </div>
        <div class="ctrl-row">
          <label>Pin阈值</label>
          <input type="number" v-model.number="pinThreshold" style="width:80px" />
        </div>
        <div class="ctrl-row">
          <button class="accent" :disabled="settingInProgress" @click="sendAllCaptureParams">
            {{ settingInProgress ? '设置中...' : '统一设置' }}
          </button>
        </div>
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

    <!-- Tab Content: 电机 & 触发 -->
    <div v-show="activeTab === 'motor'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">电机 & 触发</h4>
        <div class="ctrl-row">
          <label>触发模式</label>
          <select v-model.number="trgMode" @change="setTrgMode">
            <option :value="0">内触发</option>
            <option :value="1">外触发</option>
          </select>
        </div>
      </section>
    </div>

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
    </div>
  </div>
</template>

<style scoped>
.control-panel {
  font-size: 12px;
}

.ctrl-group {
  margin-bottom: 14px;
}

.group-title {
  font-size: 11px;
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
  font-size: 11px;
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
  font-size: 12px;
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
  font-size: 11px;
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
  font-size: 11px;
  font-weight: 600;
}

/* 通道可见性 */
.ch-vis-row {
  display: flex;
  gap: 2px;
  flex-wrap: wrap;
}
.ch-cb {
  display: flex;
  align-items: center;
  gap: 2px;
  cursor: pointer;
  font-size: 10px;
  background: var(--bg-primary);
  padding: 1px 6px;
  border-radius: 3px;
  border: 1px solid transparent;
  transition: border-color 0.15s;
}
.ch-cb:hover {
  border-color: var(--ch-color, var(--accent-dim));
}
.ch-cb input[type="checkbox"] {
  width: 12px;
  height: 12px;
  cursor: pointer;
}
.ch-cb span {
  color: var(--ch-color, var(--text-primary));
  font-weight: 600;
}
</style>
