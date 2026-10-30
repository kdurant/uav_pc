<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, defineAsyncComponent } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useDeviceDiscovery } from "../composables/useDeviceDiscovery";
import { useSystemStatus } from "../composables/useSystemStatus";
import { usePreviewData } from "../composables/usePreviewData";
import { useToast } from "../composables/useToast";
import { useRadarConfig } from "../composables/useRadarConfig";
import DeviceSelector from "./DeviceSelector.vue";
import StatusPanel from "./StatusPanel.vue";
import ChartView from "./ChartView.vue";
import ControlPanel from "./ControlPanel.vue";
import TriggerModeDialog from "./TriggerModeDialog.vue";
import ReplayBar from "./ReplayBar.vue";
import CloudBar from "./CloudBar.vue";

// three.js 体积较大，点云视图按需加载，避免拖慢首屏
const PointCloudView = defineAsyncComponent(() => import("./PointCloudView.vue"));
import { useReplay } from "../composables/useReplay";
import { usePointCloud } from "../composables/usePointCloud";
import { ConnectionState } from "../types";
import type { PreviewFrame } from "../types";

const { devices, isSearching, selectedDevice, searchDevices, selectDevice, clearSelection } =
  useDeviceDiscovery();

const {
  displayName: radarDisplayName,
  load: loadRadarConfig,
} = useRadarConfig();

onMounted(loadRadarConfig);

const { status, stale: statusStale, start: startStatus, stop: stopStatus } = useSystemStatus();

const toast = useToast();

// 离线数据回放：与实时预览共用中间图表，二者互斥
const {
  active: replayActive,
  info: replayInfo,
  gps: replayGps,
  enter: enterReplay,
  exit: exitReplay,
  setSink: setReplaySink,
} = useReplay();

// 点云 / DEM 模式（与实时预览、离线回放互斥）
const {
  active: cloudActive,
  data: cloudData,
  display: cloudDisplay,
  zScale: cloudZScale,
  enter: enterCloud,
  exit: exitCloud,
} = usePointCloud();

/** 任何离线模式（回放 / 点云）都会暂停实时预览 */
const offlineMode = computed(() => replayActive.value || cloudActive.value);

function toggleReplay() {
  if (replayActive.value) {
    exitReplay();
    return;
  }
  if (cloudActive.value) exitCloud();
  enterReplay();
}

function toggleCloud() {
  if (cloudActive.value) {
    exitCloud();
    return;
  }
  if (replayActive.value) exitReplay();
  enterCloud();
}

const connectionState = ref<ConnectionState>(ConnectionState.Disconnected);
const showDeviceSelector = ref(false);
const isConnecting = ref(false);

// 通道可见性
const channelVisible = ref([true, true, true, true]);

// Chart ref
const chartRef = ref<InstanceType<typeof ChartView> | null>(null);

// 预览帧中的细分时间（100ns 单位），显示在状态面板 GPS 周下方
const previewSubTime = ref<number | null>(null);
let lastPreviewSubTimeUpdate = 0;
const PREVIEW_SUB_TIME_INTERVAL_MS = 100;

/** 清空预览细分时间（断开/停止采集时避免显示旧值） */
function clearPreviewSubTime() {
  previewSubTime.value = null;
  lastPreviewSubTimeUpdate = 0;
}

// Preview data → ChartView（整帧、按绝对采样位置对齐）+ 细分时间
function pushPreviewFrame(series: number[][], frame: PreviewFrame) {
  // 回放 / 点云模式与实时预览互斥，实时帧不上屏，避免两路数据互相覆盖
  if (offlineMode.value) return;

  chartRef.value?.setFrame(series);

  // 细分时间不需要跟 30Hz 帧率刷新，限频到 10Hz，避免状态面板频繁重绘
  const now = performance.now();
  if (now - lastPreviewSubTimeUpdate >= PREVIEW_SUB_TIME_INTERVAL_MS) {
    lastPreviewSubTimeUpdate = now;
    previewSubTime.value = frame.sub_time;
  }
}

const previewOutput = usePreviewData(pushPreviewFrame);

// 回放帧出口：与实时帧共用同一个图表实例
setReplaySink((series) => {
  if (series) chartRef.value?.setFrame(series);
  else chartRef.value?.clear();
});

// 离线模式与实时预览互斥：进入离线模式时停掉实时帧通道，退出时按连接状态恢复
watch(offlineMode, (on) => {
  if (on) {
    previewOutput.stop();
    clearPreviewSubTime();
  } else if (connectionState.value === ConnectionState.Connected) {
    previewOutput.start();
  }
});

// 连上设备后实时预览优先，自动退出回放
watch(connectionState, (state) => {
  if (state === ConnectionState.Connected && replayActive.value) {
    exitReplay();
    toast.info("已退出离线回放", "设备已连接，恢复实时预览");
  }
});

// 断开连接或设备停止上传预览时清空细分时间
watch(connectionState, (state) => {
  if (state !== ConnectionState.Connected) clearPreviewSubTime();
});
watch(
  () => status.value?.preview_enable,
  (enabled) => {
    if (!enabled) clearPreviewSubTime();
  }
);

/** 连接中断标记：状态数据中断后置位，收到新状态或重新连接后清除 */
const linkLost = ref(false);
/** 连接失败的目标 IP（等待状态超时） */
const connectFailedIp = ref("");
/** 等待第一帧状态的超时定时器 */
let connectTimer: number | null = null;

function clearConnectTimer() {
  if (connectTimer !== null) {
    clearTimeout(connectTimer);
    connectTimer = null;
  }
}

/** 下发查询后最多等待多久收到第一帧状态 */
const CONNECT_TIMEOUT_MS = 3500;

watch(statusStale, (stale) => {
  if (stale) {
    // 只有已经确认连接后才提示中断；连接中由超时逻辑处理
    if (connectionState.value === ConnectionState.Connected) {
      linkLost.value = true;
      toast.warning(
        "设备连接中断",
        `${selectedDevice.value?.ip ?? "设备"} 的状态上报已停止`
      );
    }
  } else if (connectionState.value === ConnectionState.Connected && status.value) {
    // 只有收到新的状态数据才恢复；关闭状态读取（status 被清空）时保持中断标记
    linkLost.value = false;
  }
});

// 第一帧状态到达后，才把“连接中”切换为“已连接”
watch(status, (s) => {
  if (s && connectionState.value === ConnectionState.Connecting) {
    clearConnectTimer();
    connectionState.value = ConnectionState.Connected;
    linkLost.value = false;
    connectFailedIp.value = "";
  }
});

onUnmounted(clearConnectTimer);

const statusLabel = computed(() => {
  if (connectFailedIp.value) {
    return `连接失败: ${connectFailedIp.value}`;
  }
  if (linkLost.value) {
    return `连接中断: ${selectedDevice.value?.ip ?? ""}`;
  }
  switch (connectionState.value) {
    case ConnectionState.Searching:
      return "搜索中...";
    case ConnectionState.Connecting:
      return `连接中: ${selectedDevice.value?.ip ?? ""}`;
    case ConnectionState.Connected:
      return `已连接: ${selectedDevice.value?.ip ?? ""}`;
    default:
      return "未连接";
  }
});

/** 连接状态胶囊样式 */
const connectionClass = computed(() => {
  if (connectFailedIp.value) return "is-lost";
  if (linkLost.value) return "is-lost";
  switch (connectionState.value) {
    case ConnectionState.Connected:
      return "is-connected";
    case ConnectionState.Searching:
    case ConnectionState.Connecting:
      return "is-searching";
    default:
      return "is-idle";
  }
});

async function handleSearch() {
  linkLost.value = false;
  connectFailedIp.value = "";
  clearConnectTimer();
  stopStatus();
  previewOutput.stop();
  connectionState.value = ConnectionState.Searching;
  await searchDevices();

  if (devices.value.length === 1) {
    handleSelectDevice(devices.value[0]);
  } else if (devices.value.length > 1) {
    showDeviceSelector.value = true;
  } else {
    connectionState.value = ConnectionState.Disconnected;
    clearSelection();
    toast.warning(
      "未发现设备",
      "请确认 PC 与设备在同一网段，检查网线连接后重新搜索"
    );
  }
}

function handleSelectDevice(device: (typeof devices.value)[0]) {
  selectDevice(device);
  showDeviceSelector.value = false;
  handleConnect(device);
}

async function handleConnect(device: (typeof devices.value)[0]) {
  isConnecting.value = true;
  linkLost.value = false;
  connectFailedIp.value = "";
  clearConnectTimer();
  // 清掉上一次的状态，避免用旧数据立即判定为已连接
  stopStatus();
  connectionState.value = ConnectionState.Connecting;

  try {
    // 先注册状态监听，再下发查询命令，避免错过第一帧
    await startStatus();
    connectTimer = window.setTimeout(() => {
      if (connectionState.value !== ConnectionState.Connecting) return;
      connectTimer = null;
      connectFailedIp.value = device.ip;
      connectionState.value = ConnectionState.Disconnected;
      stopStatus();
      previewOutput.stop();
      clearSelection();
      toast.error(
        `连接失败：${device.ip}`,
        `设备未在 ${CONNECT_TIMEOUT_MS / 1000} 秒内上报状态`
      );
      // 尽力关闭设备状态上传
      invoke("query_sys_status", { deviceIp: device.ip, enable: false }).catch(() => {});
    }, CONNECT_TIMEOUT_MS);

    await invoke("query_sys_status", {
      deviceIp: device.ip,
      enable: true,
    });
    previewOutput.start();
  } catch (err) {
    console.error("Connection failed:", err);
    clearConnectTimer();
    stopStatus();
    previewOutput.stop();
    connectFailedIp.value = device.ip;
    connectionState.value = ConnectionState.Disconnected;
    clearSelection();
    toast.error(`连接失败：${device.ip}`, String(err));
  } finally {
    isConnecting.value = false;
  }
}

async function handleToggleStatus(enable: boolean) {
  if (!selectedDevice.value) return;
  try {
    await invoke("query_sys_status", {
      deviceIp: selectedDevice.value.ip,
      enable,
    });
    if (enable) {
      startStatus();
    } else {
      stopStatus();
    }
  } catch (err) {
    console.error("Toggle status failed:", err);
    toast.error(
      enable ? "开启状态上报失败" : "关闭状态上报失败",
      String(err)
    );
  }
}

async function handleDisconnect() {
  clearConnectTimer();
  stopStatus();
  previewOutput.stop();
  if (selectedDevice.value) {
    try {
        await invoke("query_sys_status", {
          deviceIp: selectedDevice.value.ip,
          enable: false,
        });
    } catch (err) {
      console.error("Disconnect failed:", err);
      toast.error("断开连接失败", String(err));
    }
  }
  clearSelection();
  linkLost.value = false;
  connectFailedIp.value = "";
  connectionState.value = ConnectionState.Disconnected;
}

/** 设备重启/关机命令已发出：立即按断开处理，不再等待状态超时 */
function handlePowered() {
  clearConnectTimer();
  stopStatus();
  previewOutput.stop();
  linkLost.value = false;
  connectFailedIp.value = "";
  clearSelection();
  connectionState.value = ConnectionState.Disconnected;
}

// 启动时自动搜索
handleSearch();
</script>

<template>
  <div class="layout">
    <!-- Top toolbar -->
    <header class="toolbar">
      <div class="brand">
        <div class="brand-mark">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M2 13h4l2.4-6 3.1 12 3-9 2 3H22" />
          </svg>
        </div>
        <div class="brand-text">
          <span class="app-title">{{ radarDisplayName }}上位机</span>
        </div>
      </div>

      <div class="toolbar-right">
        <button
          class="tool-btn"
          :class="{ active: replayActive }"
          :title="replayActive ? '退出离线回放，恢复实时预览' : '按帧回放本地采集数据文件'"
          @click="toggleReplay"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 5v14l9-7-9-7z" />
            <path d="M14 5h2v14h-2z" />
          </svg>
          {{ replayActive ? "退出回放" : "离线回放" }}
        </button>
        <button
          class="tool-btn"
          :class="{ active: cloudActive }"
          :title="cloudActive ? '退出点云模式' : '把采集数据生成水下地形点云 / DEM'"
          @click="toggleCloud"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9L12 3z" />
            <path d="M4 7.5l8 4.5 8-4.5" />
            <path d="M12 12v9" />
          </svg>
          {{ cloudActive ? "退出点云" : "点云" }}
        </button>
        <div class="conn-pill" :class="connectionClass">
          <span class="conn-dot"></span>
          <span class="conn-label">{{ statusLabel }}</span>
        </div>
        <button
          v-if="connectionState !== ConnectionState.Connected || linkLost"
          class="accent"
          :disabled="isSearching || isConnecting || connectionState === ConnectionState.Connecting"
          @click="handleSearch"
        >
          {{
            isSearching
              ? "搜索中..."
              : connectionState === ConnectionState.Connecting
                ? "连接中..."
                : linkLost || connectFailedIp
                  ? "重新搜索"
                  : "搜索设备"
          }}
        </button>
      </div>
    </header>

    <!-- Main content: three columns -->
    <div class="main-content">
      <!-- Left: Control Panel -->
      <aside class="panel panel-left">
        <div class="panel-header">
          <span class="panel-title">控制面板</span>
          <span class="panel-tag">CONTROL</span>
        </div>
        <div class="panel-body">
          <ControlPanel
            :connected="connectionState === ConnectionState.Connected"
            :device-ip="selectedDevice?.ip"
            :status="status"
            @search="handleSearch"
            @disconnect="handleDisconnect"
            @toggle-status="handleToggleStatus"
            @powered="handlePowered"
          />
        </div>
      </aside>

      <!-- Center: Chart -->
      <main class="panel panel-center">
        <div class="panel-header">
          <span class="panel-title">数据预览</span>
          <span class="panel-tag" :class="{ replay: replayActive || cloudActive, live: !offlineMode && connectionState === ConnectionState.Connected }">
            {{
              cloudActive
                ? `点云地形 · ${cloudData ? cloudData.pointCount.toLocaleString("zh-CN") + " 点" : "未生成"}`
                : replayActive
                  ? `离线回放 · ${replayInfo ? replayInfo.frame_count.toLocaleString("zh-CN") + " 帧" : "未选择文件"}`
                  : "实时波形 · 4 CH"
            }}
          </span>
        </div>
        <div class="panel-body chart-body">
          <PointCloudView
            v-if="cloudActive"
            :data="cloudData"
            :display="cloudDisplay"
            :z-scale="cloudZScale"
          />
          <ChartView
            v-else
            ref="chartRef"
            :channel-count="4"
            :max-points="4000"
            :channel-visible="channelVisible"
            @update:channel-visible="channelVisible = $event"
          />
        </div>
        <ReplayBar v-if="replayActive" />
        <CloudBar v-if="cloudActive" />
      </main>

      <!-- Right: Status Panel -->
      <aside class="panel panel-right">
        <div class="panel-header">
          <span class="panel-title">设备状态</span>
          <span class="panel-tag" :class="{ live: connectionState === ConnectionState.Connected }">
            <span class="tag-dot"></span>{{ connectionState === ConnectionState.Connected ? "实时" : "离线" }}
          </span>
        </div>
        <div class="panel-body">
          <StatusPanel :status="status" :preview-sub-time="previewSubTime" :gps-override="replayGps" />
        </div>
      </aside>
    </div>

    <!-- Device Selector Modal -->
    <DeviceSelector
      :devices="devices"
      :visible="showDeviceSelector"
      @select="handleSelectDevice"
      @cancel="showDeviceSelector = false"
    />

    <!-- 触发方式说明（状态面板 / 控制面板共用） -->
    <TriggerModeDialog
      :device-mode="status ? Number(status.trg_mode) : undefined"
      :sum-level="status?.sum_level"
      :min-level="status?.min_level"
    />

  </div>
</template>

<style scoped>
.layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background: var(--bg-app);
}

/* ---------- 顶部工具栏 ---------- */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 58px;
  padding: 0 18px;
  background: #fff;
  border-bottom: 1px solid var(--border-color);
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.03);
  flex-shrink: 0;
  z-index: 10;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.brand-mark {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 9px;
  color: #fff;
  background: linear-gradient(145deg, #4b86ff, #1d4ed8);
  box-shadow: 0 2px 6px rgba(37, 99, 235, 0.32), inset 0 1px 0 rgba(255, 255, 255, 0.35);
  flex-shrink: 0;
}

.brand-mark svg {
  width: 19px;
  height: 19px;
}

.brand-text {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.app-title {
  font-size: 14.5px;
  font-weight: 650;
  color: var(--text-primary);
  letter-spacing: 0.012em;
  white-space: nowrap;
  line-height: 1.25;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.tool-btn.active {
  border-color: var(--accent);
  color: var(--accent-dim);
  background: var(--accent-glow, #eff4ff);
}

.tool-btn {
  gap: 6px;
  height: 30px;
  padding: 0 12px;
  font-size: 12.5px;
}

.tool-btn svg {
  width: 13px;
  height: 13px;
  color: var(--text-tertiary);
}

.tool-btn:hover svg {
  color: var(--accent-dim);
}

/* 连接状态胶囊 */
.conn-pill {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 12px;
  border-radius: 999px;
  border: 1px solid var(--border-color);
  background: var(--bg-inset);
  font-size: 12.5px;
  font-weight: 500;
  color: var(--text-secondary);
  max-width: 280px;
}

.conn-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.conn-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-tertiary);
  flex-shrink: 0;
}

.conn-pill.is-connected {
  background: var(--success-soft);
  border-color: #c8ecd7;
  color: #0e6b3c;
}

.conn-pill.is-connected .conn-dot {
  background: var(--success);
  box-shadow: 0 0 0 3px rgba(18, 136, 74, 0.14);
}

.conn-pill.is-searching {
  background: var(--warning-soft);
  border-color: #f6dfbc;
  color: #9a5b06;
}

.conn-pill.is-searching .conn-dot {
  background: #e08c1a;
  animation: dot-pulse 1.1s ease-in-out infinite;
}

/* 已连接但状态数据中断 */
.conn-pill.is-lost {
  background: var(--error-soft);
  border-color: #f7d4d4;
  color: #b42318;
}

.conn-pill.is-lost .conn-dot {
  background: #e5484d;
  box-shadow: 0 0 0 3px rgba(229, 72, 77, 0.14);
}

@keyframes dot-pulse {
  0%, 100% {
    box-shadow: 0 0 0 0 rgba(224, 140, 26, 0.45);
  }
  50% {
    box-shadow: 0 0 0 4px rgba(224, 140, 26, 0);
  }
}

/* ---------- 主内容区 ---------- */
.main-content {
  display: flex;
  flex: 1;
  overflow: hidden;
  gap: 12px;
  padding: 12px 14px 14px;
  min-height: 0;
}

/* 卡片式面板 */
/* 注意：这里刻意不使用 box-shadow。WebKitGTK 在面板内部内容变化时
   会重新栅格化整块模糊阴影，导致点击 Tab/按钮时出现数百毫秒卡顿。 */
.panel {
  display: flex;
  flex-direction: column;
  background: var(--bg-secondary);
  border: 1px solid #dde3ec;
  border-radius: var(--panel-radius);
  overflow: hidden;
  min-height: 0;
}

.panel-left {
  width: 292px;
  min-width: 240px;
  flex-shrink: 0;
}

.panel-center {
  flex: 1;
  min-width: 380px;
}

.panel-right {
  width: 356px;
  min-width: 290px;
  flex-shrink: 0;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 0 14px;
  height: 42px;
  background: #fff;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.panel-title {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 650;
  color: var(--text-primary);
  letter-spacing: 0.02em;
}

.panel-title::before {
  content: "";
  width: 3px;
  height: 13px;
  border-radius: 2px;
  background: linear-gradient(180deg, #4b86ff, var(--accent-dim));
}

.panel-tag {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.1em;
  color: var(--text-tertiary);
  text-transform: uppercase;
}

.panel-tag.live {
  color: var(--success);
}

.panel-tag.replay {
  color: var(--accent-dim, #2563eb);
}

.tag-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: currentColor;
}

.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 14px;
  min-height: 0;
}

.chart-body {
  padding: 0;
  display: flex;
  overflow: hidden;
  background: #fff;
}

/* ---------- 窄窗口自适应 ---------- */
@media (max-width: 1200px) {
  .main-content {
    gap: 10px;
    padding: 10px;
  }

  .panel-left {
    width: 272px;
  }

  .panel-right {
    width: 326px;
  }
}

@media (max-width: 1080px) {
  .toolbar {
    height: 52px;
    padding: 0 12px;
  }

  .main-content {
    gap: 8px;
    padding: 8px;
  }

  .panel-left {
    width: 258px;
  }

  .panel-right {
    width: 306px;
    min-width: 280px;
  }

  .panel-center {
    min-width: 300px;
  }

  .panel-header {
    padding: 0 11px;
    height: 38px;
  }

  .panel-body {
    padding: 11px;
  }
}
</style>
