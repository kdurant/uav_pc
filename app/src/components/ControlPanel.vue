<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** 通用命令按钮 Props */
defineProps<{
  connected: boolean;
  deviceIp?: string;
}>();

// ===== 设备连接 =====
const emit = defineEmits<{
  (e: "search"): void;
  (e: "disconnect"): void;
  (e: "toggle-status", enable: boolean): void;
}>();

const statusPollingEnabled = ref(true);

function toggleStatusPolling() {
  emit("toggle-status", statusPollingEnabled.value);
}

// ===== 激光器控制 =====
const laserFreq = ref(1000);
const laserEnabled = ref(false);

async function setLaserFreq() {
  // 预留：调用 send_command(0x0201, ...)
  console.log("setLaserFreq:", laserFreq.value);
}
async function toggleLaser() {
  // 预留：调用 send_command(0x0202, ...)
  console.log("toggleLaser:", laserEnabled.value);
}

// ===== 采集控制 =====
const previewEnabled = ref(false);
const previewCoe = ref(1);
const saveChannel = ref(0xff);
const waveLen = ref(2048);
const firstPos = ref(0);
const firstLen = ref(200);
const secondPos = ref(0);
const secondLen = ref(200);
const sumLevel = ref(100);
const minLevel = ref(50);

async function setPreviewEnable() {
  // 预留：调用 send_command(0x0300, ...)
  console.log("setPreviewEnable:", previewEnabled.value);
}

// ===== 存储控制 =====
const storeEnabled = ref(false);
const storeFilename = ref("");

async function setStoreEnable() {
  // 预留：调用 send_command(0x0406, ...)
  console.log("setStoreEnable:", storeEnabled.value);
}

// ===== 电机控制 =====
const trgMode = ref(0);

async function setTrgMode() {
  // 预留：调用 send_command(0x0f03, ...)
  console.log("setTrgMode:", trgMode.value);
}
</script>

<template>
  <div class="control-panel">
    <!-- 设备连接组 -->
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

    <!-- 激光器控制组 -->
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

    <!-- 采集参数组 -->
    <section class="ctrl-group">
      <h4 class="group-title">采集参数</h4>
      <div class="ctrl-row">
        <label>采集开关</label>
        <div class="toggle-row">
          <label class="toggle">
            <input type="checkbox" v-model="previewEnabled" @change="setPreviewEnable" />
            <span class="toggle-label">{{ previewEnabled ? 'ON' : 'OFF' }}</span>
          </label>
        </div>
      </div>
      <div class="ctrl-row">
        <label>抽样率</label>
        <div class="input-row">
          <input type="number" v-model.number="previewCoe" min="1" max="255" style="width:70px" />
        </div>
      </div>
      <div class="ctrl-row">
        <label>采集长度</label>
        <div class="input-row">
          <input type="number" v-model.number="waveLen" min="1" max="8192" style="width:70px" />
        </div>
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
        <input type="number" v-model.number="sumLevel" style="width:70px" />
      </div>
      <div class="ctrl-row">
        <label>值阈值</label>
        <input type="number" v-model.number="minLevel" style="width:70px" />
      </div>
    </section>

    <!-- 电机控制组 -->
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

    <!-- 存储控制组 -->
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
</style>
