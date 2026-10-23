<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { LaserStatus, MotorState, SsdFile, SystemStatus } from "../types";
import StorageDrawer from "./StorageDrawer.vue";
import PowerDialog from "./PowerDialog.vue";

/** 通用命令按钮 Props */
const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
  status?: SystemStatus | null;
}>();

// ===== 设备连接 =====
const emit = defineEmits<{
  (e: "search"): void;
  (e: "disconnect"): void;
  (e: "toggle-status", enable: boolean): void;
  (e: "powered", action: "reboot" | "shutdown"): void;
}>();

const statusPollingEnabled = ref(true);

/** 运行时间：秒 → HH:MM:SS，无数据显示 "-" */
function formatUptime(seconds: number | undefined): string {
  if (seconds === undefined) return "-";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const sec = seconds % 60;
  return [h, m, sec].map((v) => String(v).padStart(2, "0")).join(":");
}

function toggleStatusPolling() {
  emit("toggle-status", statusPollingEnabled.value);
}

// ===== 激光器控制（0x0200 透传 / 0x0201 重频） =====
const laserFreq = ref(20000);
const laserEnabled = ref(false);
const laserMode = ref(1); // 0 内触发，1 外触发
const laserCurrent = ref(1000);
const laserStatus = ref<LaserStatus | null>(null);
const laserBusy = ref(false);
const laserError = ref("");

async function laserAction(action: string, value?: number): Promise<boolean> {
  if (!props.connected || !props.deviceIp) return false;
  laserBusy.value = true;
  laserError.value = "";
  try {
    const status = await invoke<LaserStatus | null>("laser_command", {
      deviceIp: props.deviceIp,
      action,
      value,
    });
    if (status) laserStatus.value = status;
    return true;
  } catch (e) {
    laserError.value = String(e);
    console.error(`laser_command ${action} failed:`, e);
    return false;
  } finally {
    laserBusy.value = false;
  }
}

async function setLaserFreq() {
  await laserAction("freq", Math.trunc(laserFreq.value));
}

/** 电源开关：打开后立即重设电流（激光器关闭会把内部电流清零） */
async function toggleLaser() {
  const target = laserEnabled.value;
  if (target) {
    const opened = await laserAction("open");
    if (!opened) {
      laserEnabled.value = false;
      return;
    }
    // 打开成功但电流设置失败时保持开启状态，便于用户直接重试
    await laserAction("current", Math.trunc(laserCurrent.value));
  } else {
    const closed = await laserAction("close");
    if (!closed) laserEnabled.value = true;
  }
}

/** 模式切换：失败时回滚 */
async function setLaserMode(mode: number) {
  const prev = laserMode.value;
  laserMode.value = mode;
  const ok = await laserAction("mode", mode);
  if (!ok) laserMode.value = prev;
}

async function setLaserCurrent() {
  await laserAction("current", Math.trunc(laserCurrent.value));
}

async function queryLaserStatus() {
  const ok = await laserAction("status");
  if (ok && laserStatus.value) {
    // 用设备回报的开关状态校正界面
    laserEnabled.value = laserStatus.value.status !== 0;
  }
}

const laserStatusItems = computed(() => {
  const s = laserStatus.value;
  const dash = "-";
  return [
    { label: "开关状态", value: s ? (s.status === 0 ? "关闭" : "开启") : dash },
    {
      label: "错误码",
      value: s ? `0x${s.error_code.toString(16).padStart(2, "0")}` : dash,
    },
    { label: "箱体温度", value: s ? `${s.box_temp.toFixed(1)} ℃` : dash },
    { label: "泵浦1温度", value: s ? `${s.pump1_temp.toFixed(1)} ℃` : dash },
    { label: "泵浦2温度", value: s ? `${s.pump2_temp.toFixed(1)} ℃` : dash },
    { label: "泵浦1电流", value: s ? `${s.pump1_current} mA` : dash },
    { label: "泵浦2电流", value: s ? `${s.pump2_current} mA` : dash },
    { label: "输出功率", value: s ? `${(s.export_power / 1000).toFixed(2)} W` : dash },
  ];
});

// ===== 电机控制（0x0f00 透传） =====
const motorBusy = ref(false);
const motorError = ref("");
const motorState = ref<MotorState | null>(null);
const motorSpeed = ref(100); // r/s
const motorTargetPos = ref(0);
const motorHomeMethod = ref(34);

async function motorAction(action: string, value?: number): Promise<boolean> {
  if (!props.connected || !props.deviceIp) return false;
  motorBusy.value = true;
  motorError.value = "";
  try {
    const state = await invoke<MotorState | null>("motor_command", {
      deviceIp: props.deviceIp,
      action,
      value,
    });
    if (state) motorState.value = state;
    return true;
  } catch (e) {
    motorError.value = String(e);
    console.error(`motor_command ${action} failed:`, e);
    return false;
  } finally {
    motorBusy.value = false;
  }
}

async function initMotor() {
  await motorAction("init");
}

async function runMotor() {
  await motorAction("run", Math.trunc(motorSpeed.value));
}

async function stopMotor() {
  await motorAction("stop");
}

async function moveMotor() {
  await motorAction("move_to", Math.trunc(motorTargetPos.value));
}

async function homeMotor() {
  await motorAction("home", Math.trunc(motorHomeMethod.value));
}

async function stopHoming() {
  await motorAction("home_stop");
}

async function refreshMotorState() {
  await motorAction("read");
}

/** 断开连接时清空设备侧读数 */
watch(
  () => props.connected,
  (connected) => {
    if (connected) return;
    laserStatus.value = null;
    motorState.value = null;
    laserError.value = "";
    motorError.value = "";
  }
);

// ===== 设备电源控制（0x0103 重启 / 0x0104 关机） =====
const powerDialogVisible = ref(false);
const powerAction = ref<"reboot" | "shutdown">("reboot");
const powerPhase = ref<"confirm" | "countdown" | "done">("confirm");
const powerRemaining = ref(0);
const powerBusy = ref(false);
const powerError = ref("");
let powerCountdownTimer: number | null = null;

function stopPowerCountdown() {
  if (powerCountdownTimer !== null) {
    window.clearInterval(powerCountdownTimer);
    powerCountdownTimer = null;
  }
}

function startPowerCountdown() {
  stopPowerCountdown();
  powerCountdownTimer = window.setInterval(() => {
    if (powerRemaining.value > 0) {
      powerRemaining.value -= 1;
    }
    if (powerRemaining.value <= 0) {
      stopPowerCountdown();
      powerPhase.value = "done";
    }
  }, 1000);
}

/** 打开电源操作对话框（确认阶段） */
function openPowerDialog(action: "reboot" | "shutdown") {
  if (!props.connected || !props.deviceIp) return;
  stopPowerCountdown();
  powerAction.value = action;
  powerPhase.value = "confirm";
  powerRemaining.value = action === "reboot" ? 25 : 15;
  powerError.value = "";
  powerDialogVisible.value = true;
}

function cancelPowerDialog() {
  powerDialogVisible.value = false;
  powerError.value = "";
}

function closePowerDialog() {
  stopPowerCountdown();
  powerDialogVisible.value = false;
}

/** 确认后下发命令，并进入倒计时阶段 */
async function confirmPower() {
  if (!props.deviceIp || powerBusy.value) return;
  powerBusy.value = true;
  powerError.value = "";
  try {
    const action = powerAction.value;
    const cmd = action === "reboot" ? 0x0103 : 0x0104;
    await invoke("send_command", {
      deviceIp: props.deviceIp,
      cmd,
      data: [0, 0, 0, 1],
    });
    powerPhase.value = "countdown";
    powerRemaining.value = action === "reboot" ? 25 : 15;
    startPowerCountdown();
    emit("powered", action);
  } catch (e) {
    powerError.value = String(e);
    console.error(`power command failed:`, e);
  } finally {
    powerBusy.value = false;
  }
}

onUnmounted(stopPowerCountdown);

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
// 当前文件名是否由程序自动生成：自动生成的名字每次启动都重新生成，
// 手动输入的名字则保持复用
const storeFilenameAuto = ref(false);
const storeBusy = ref(false);
const storeError = ref("");

/** 默认文件名：uav_YYYYMMDD_HHMMSS */
function defaultStoreName(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `uav_${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}_${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

/** 打开/关闭 SSD 存储：开启时先写文件名与数据起始地址，再发开始命令 */
async function toggleStore() {
  const enable = storeEnabled.value;
  if (!props.connected || !props.deviceIp) {
    storeEnabled.value = !enable;
    storeError.value = "请先连接设备";
    return;
  }
  if (enable && (!storeFilename.value.trim() || storeFilenameAuto.value)) {
    storeFilename.value = defaultStoreName();
    storeFilenameAuto.value = true;
  }

  storeBusy.value = true;
  storeError.value = "";
  try {
    await invoke("set_ssd_store", {
      deviceIp: props.deviceIp,
      enable,
      filename: storeFilename.value.trim(),
    });
    if (!enable) {
      // 停止后稍等设备写完结束地址，再静默刷新文件摘要
      await new Promise((r) => setTimeout(r, 400));
      await loadSsdFiles(false);
    }
  } catch (e) {
    storeEnabled.value = !enable;
    storeError.value = String(e);
    console.error("set_ssd_store failed:", e);
  } finally {
    storeBusy.value = false;
  }
}

// ===== SSD 已存储文件检索 =====
const ssdFiles = ref<SsdFile[]>([]);
const ssdLoading = ref(false);
const ssdError = ref("");
// 文件列表抽屉（左栏宽度固定，列表在抽屉中展示）
const ssdDrawerVisible = ref(false);

/** SSD 文件总大小 */
const ssdTotalSize = computed(() => ssdFiles.value.reduce((sum, f) => sum + f.size, 0));

async function loadSsdFiles(openDrawer: boolean) {
  if (!props.connected || !props.deviceIp) return;
  if (openDrawer) ssdDrawerVisible.value = true;
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

function refreshSsdFiles() {
  return loadSsdFiles(true);
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
const gpsDrawerVisible = ref(false);

/** GPS 文件名 → 抽屉条目 */
const gpsDrawerFiles = computed(() => gpsFiles.value.map((name) => ({ name })));

async function refreshGpsFiles() {
  if (!props.connected || !props.deviceIp) return;
  gpsDrawerVisible.value = true;
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

// ===== 增益反馈（来自设备状态 0x0102） =====
/** ADC 原始值 → 电压：16 位，满量程 5V */
function adcVoltage(value: number | undefined): number {
  return ((value ?? 0) / 65536) * 5;
}

/** APD 高压反馈换算系数：Vhv = Vadc * 293 * 0.798 */
const APD_HV_COEFF = 293 * 0.798;

/** 取状态中的 ADC 通道原始值 */
function statusAdc(ch: number): number | undefined {
  const s = props.status;
  if (!s) return undefined;
  return [s.adc_ch0, s.adc_ch1, s.adc_ch2, s.adc_ch3][ch];
}

function pmtFeedbackText(ch: number): string {
  const v = statusAdc(ch);
  return v === undefined ? "-" : adcVoltage(v).toFixed(3) + " V";
}

function apdFeedbackText(): string {
  const v = statusAdc(3);
  return v === undefined ? "-" : (adcVoltage(v) * APD_HV_COEFF).toFixed(1) + " V";
}

/** 增益页底部的四路反馈读数 */
const feedbackItems = computed(() => [
  { key: "pmt1", label: "PMT1 反馈", value: pmtFeedbackText(0), type: "pmt" },
  { key: "pmt2", label: "PMT2 反馈", value: pmtFeedbackText(1), type: "pmt" },
  { key: "pmt3", label: "PMT3 反馈", value: pmtFeedbackText(2), type: "pmt" },
  { key: "apd", label: "APD 高压反馈", value: apdFeedbackText(), type: "apd" },
]);

// ===== Tab =====
const activeTab = ref("laser");
const tabs = [
  { key: "laser", label: "激光器" },
  { key: "motor", label: "电机" },
  { key: "voltage", label: "增益" },
  { key: "capture", label: "采集" },
  { key: "storage", label: "存储" },
];

function selectTab(key: string) {
  activeTab.value = key;
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

      <div class="device-meta">
        <div class="device-meta-row">
          <span class="k">SN</span>
          <span class="v mono" :title="status?.device_sn || '-'">
            {{ status?.device_sn || "-" }}
          </span>
        </div>
        <div class="device-meta-row">
          <span class="k">设备类型</span>
          <span class="v" :title="status?.device_type || '-'">
            {{ status?.device_type || "-" }}
          </span>
        </div>
        <div class="device-meta-row">
          <span class="k">运行时间</span>
          <span class="v mono">{{ formatUptime(status?.uptime) }}</span>
        </div>
      </div>

      <div class="power-actions">
        <button
          class="danger"
          :disabled="!connected || powerBusy"
          @click="openPowerDialog('reboot')"
        >
          重启
        </button>
        <button
          class="danger"
          :disabled="!connected || powerBusy"
          @click="openPowerDialog('shutdown')"
        >
          关机
        </button>
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

      <section class="ctrl-group">
        <h4 class="group-title">实时反馈</h4>
        <div class="feedback-list">
          <div v-for="item in feedbackItems" :key="item.key" class="feedback-row">
            <span class="feedback-name">
              <span class="ch-dot" :class="item.type"></span>{{ item.label }}
            </span>
            <span class="feedback-val">{{ item.value }}</span>
          </div>
        </div>
        <p class="hint">反馈值随设备状态更新（约 1s 一次）。</p>
      </section>
    </div>

    <!-- Tab Content: 激光器 -->
    <div v-show="activeTab === 'laser'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">激光器参数</h4>

        <div class="field-list">
          <div class="field">
            <span class="field-label">重频 (Hz)</span>
            <div class="field-inline">
              <input type="number" v-model.number="laserFreq" min="1" max="100000" :disabled="laserBusy" />
              <button class="set-btn" :disabled="laserBusy" @click="setLaserFreq">设置</button>
            </div>
          </div>
          <div class="field">
            <span class="field-label">电流 (mA)</span>
            <div class="field-inline">
              <input type="number" v-model.number="laserCurrent" min="0" max="6000" :disabled="laserBusy" />
              <button class="set-btn" :disabled="laserBusy" @click="setLaserCurrent">设置</button>
            </div>
          </div>
        </div>

        <div class="field">
          <span class="field-label">工作模式</span>
          <div class="mode-grid">
            <label class="mode-pill" :class="{ active: laserMode === 0 }">
              <input
                type="radio"
                name="laserMode"
                :value="0"
                v-model.number="laserMode"
                :disabled="laserBusy"
                @change="setLaserMode(0)"
              />
              <span>内触发</span>
            </label>
            <label class="mode-pill" :class="{ active: laserMode === 1 }">
              <input
                type="radio"
                name="laserMode"
                :value="1"
                v-model.number="laserMode"
                :disabled="laserBusy"
                @change="setLaserMode(1)"
              />
              <span>外触发</span>
            </label>
          </div>
        </div>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">激光器状态</h4>
        <div class="switch-row card-row">
          <span class="switch-row-label">
            激光使能
            <i class="switch-hint">{{ laserEnabled ? '已开启' : '已关闭' }}</i>
          </span>
          <label class="switch">
            <input type="checkbox" v-model="laserEnabled" :disabled="laserBusy" @change="toggleLaser" />
            <span class="switch-track"><span class="switch-thumb"></span></span>
          </label>
        </div>

        <button class="accent block status-query" :disabled="laserBusy" @click="queryLaserStatus">
          {{ laserBusy ? "查询中..." : "查询状态" }}
        </button>

        <div class="feedback-list">
          <div v-for="item in laserStatusItems" :key="item.label" class="feedback-row">
            <span class="feedback-name">{{ item.label }}</span>
            <span class="feedback-val">{{ item.value }}</span>
          </div>
        </div>

        <p v-if="laserError" class="ssd-error">{{ laserError }}</p>
        <p class="hint">未查询或未连接时显示占位值；点击“查询状态”刷新读数。</p>
        <p class="hint">激光器命令经采集卡透传，单次操作最长等待 1 秒；打开时会自动重新下发电流。</p>
      </section>
    </div>

    <!-- Tab Content: 电机 -->
    <div v-show="activeTab === 'motor'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">电机状态</h4>
        <div class="feedback-list">
          <div class="feedback-row">
            <span class="feedback-name">实际位置</span>
            <span class="feedback-val">{{ motorState ? motorState.position : "-" }}</span>
          </div>
          <div class="feedback-row">
            <span class="feedback-name">实际速度</span>
            <span class="feedback-val">{{ motorState ? motorState.velocity + " r/s" : "-" }}</span>
          </div>
        </div>
        <button class="block" :disabled="motorBusy" @click="refreshMotorState">
          {{ motorBusy ? "读取中..." : "刷新状态" }}
        </button>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">驱动器</h4>
        <button class="accent block" :disabled="motorBusy" @click="initMotor">
          {{ motorBusy ? "执行中..." : "初始化驱动器" }}
        </button>
        <p class="hint">初始化依次下发故障复位、关机、暂停与速度模式参数。</p>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">速度模式</h4>
        <div class="field-list">
          <label class="field">
            <span class="field-label">目标速度 (r/s)</span>
            <input type="number" v-model.number="motorSpeed" min="0" max="3571" />
          </label>
        </div>
        <div class="btn-row">
          <button class="accent" :disabled="motorBusy" @click="runMotor">运行</button>
          <button :disabled="motorBusy" @click="stopMotor">停止</button>
        </div>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">位置模式</h4>
        <div class="field-list">
          <label class="field">
            <span class="field-label">目标位置 (steps)</span>
            <input type="number" v-model.number="motorTargetPos" />
          </label>
        </div>
        <button class="block" :disabled="motorBusy" @click="moveMotor">绝对定位</button>
      </section>

      <section class="ctrl-group">
        <h4 class="group-title">回零</h4>
        <div class="field-list">
          <label class="field">
            <span class="field-label">回零方法</span>
            <input type="number" v-model.number="motorHomeMethod" min="0" max="35" />
          </label>
        </div>
        <div class="btn-row">
          <button :disabled="motorBusy" @click="homeMotor">开始回零</button>
          <button :disabled="motorBusy" @click="stopHoming">停止回零</button>
        </div>
        <p class="hint">回零方法与驱动器配置需一致，默认 34。</p>
      </section>

      <p v-if="motorError" class="ssd-error">{{ motorError }}</p>
    </div>

    <!-- Tab Content: 存储 -->
    <div v-show="activeTab === 'storage'" class="tab-content">
      <section class="ctrl-group">
        <h4 class="group-title">存储控制</h4>
        <div class="switch-row card-row">
          <span class="switch-row-label">
            存储开关
            <i class="switch-hint">
              {{ storeBusy ? '处理中...' : storeEnabled ? '写入中' : '已关闭' }}
            </i>
          </span>
          <label class="switch">
            <input type="checkbox" v-model="storeEnabled" :disabled="storeBusy" @change="toggleStore" />
            <span class="switch-track"><span class="switch-thumb"></span></span>
          </label>
        </div>
        <label class="field block-field">
          <span class="field-label">文件名</span>
          <input
            type="text"
            v-model="storeFilename"
            maxlength="32"
            placeholder="留空自动生成"
            :disabled="storeBusy || storeEnabled"
            @input="storeFilenameAuto = false"
          />
        </label>
        <p v-if="storeError" class="ssd-error">{{ storeError }}</p>
        <p class="hint">写入的是采集数据；文件名最多 32 个 ASCII 字符，留空时自动生成。</p>
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
        <template v-else-if="ssdFiles.length">
          <div class="file-summary">
            <span class="summary-main">{{ ssdFiles.length }} 个文件</span>
            <span class="summary-sub">合计 {{ formatSize(ssdTotalSize) }}</span>
          </div>
          <button class="block view-btn" @click="ssdDrawerVisible = true">查看文件列表</button>
        </template>
        <p v-else class="ssd-empty">暂无文件，点击“检索文件”</p>

        <p class="ssd-unit">地址单位：扇区（512 B）</p>
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
        <template v-else-if="gpsFiles.length">
          <div class="file-summary">
            <span class="summary-main">{{ gpsFiles.length }} 个文件</span>
          </div>
          <button class="block view-btn" @click="gpsDrawerVisible = true">查看文件列表</button>
        </template>
        <p v-else-if="gpsQueried" class="ssd-empty">无文件（或目录不存在）</p>

        <p class="ssd-unit">路径：/run/media/mmcblk1p1/（可在 config.toml [gps].dir 修改）</p>
      </section>
    </div>

    <!-- 文件列表抽屉：左栏宽度固定，表格在抽屉中展示 -->
    <StorageDrawer
      :visible="ssdDrawerVisible"
      title="SSD 已存储文件"
      mode="ssd"
      :loading="ssdLoading"
      :error="ssdError"
      :files="ssdFiles"
      @close="ssdDrawerVisible = false"
      @refresh="refreshSsdFiles"
    />
    <StorageDrawer
      :visible="gpsDrawerVisible"
      title="GPS 数据文件"
      mode="gps"
      :loading="gpsLoading"
      :error="gpsError"
      :files="gpsDrawerFiles"
      @close="gpsDrawerVisible = false"
      @refresh="refreshGpsFiles"
    />

    <PowerDialog
      :visible="powerDialogVisible"
      :action="powerAction"
      :phase="powerPhase"
      :remaining="powerRemaining"
      :error="powerError"
      @confirm="confirmPower"
      @cancel="cancelPowerDialog"
      @close="closePowerDialog"
    />
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

/* 设备信息（连接组底部） */
.device-meta {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin-top: 10px;
  padding-top: 9px;
  border-top: 1px dashed var(--border-color);
}

.device-meta-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: 11.5px;
}

.device-meta-row .k {
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.device-meta-row .v {
  color: var(--text-secondary);
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.device-meta-row .v.mono {
  font-family: var(--font-mono);
  font-size: 11px;
}

/* 设备电源操作 */
.power-actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}

.power-actions button {
  flex: 1;
  height: 30px;
  font-size: 12px;
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

.status-query {
  margin-top: 10px;
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

/* 放在 .field 行内时占满剩余宽度（激光器工作模式为 2 项） */
.field .mode-grid {
  flex: 1;
  grid-template-columns: repeat(2, 1fr);
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

/* ---------- 增益实时反馈 ---------- */
.feedback-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.feedback-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 11px;
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.feedback-name {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12.5px;
  color: var(--text-secondary);
}

.feedback-val {
  font-family: var(--font-mono);
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
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

/* ---------- 文件摘要（列表在抽屉中） ---------- */
.ssd-error {
  color: var(--error);
  font-size: 11.5px;
  margin-top: 8px;
  padding: 6px 9px;
  background: var(--error-soft);
  border-radius: var(--radius-xs);
  word-break: break-all;
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

.file-summary {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  margin-top: 9px;
  padding: 8px 10px;
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.summary-main {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.summary-sub {
  font-size: 11px;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.view-btn {
  margin-top: 8px;
  font-size: 12px;
  color: var(--accent-dim);
}
</style>
