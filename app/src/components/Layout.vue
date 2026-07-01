<script setup lang="ts">
import { ref, computed } from "vue";
import { useDeviceDiscovery } from "../composables/useDeviceDiscovery";
import { useSystemStatus } from "../composables/useSystemStatus";
import { usePreviewData } from "../composables/usePreviewData";
import DeviceSelector from "./DeviceSelector.vue";
import StatusPanel from "./StatusPanel.vue";
import ChartView from "./ChartView.vue";
import ControlPanel from "./ControlPanel.vue";
import { ConnectionState } from "../types";

const { devices, isSearching, selectedDevice, searchDevices, selectDevice, clearSelection } =
  useDeviceDiscovery();

const { status, start: startStatus, stop: stopStatus } = useSystemStatus();

const connectionState = ref<ConnectionState>(ConnectionState.Disconnected);
const showDeviceSelector = ref(false);
const isConnecting = ref(false);

// 通道可见性
const channelVisible = ref([true, true, true, true]);

// Chart ref
const chartRef = ref<InstanceType<typeof ChartView> | null>(null);

// Preview data → ChartView
function pushPreviewData(batches: number[][]) {
  for (const batch of batches) {
    chartRef.value?.pushData(batch);
  }
}

const previewOutput = usePreviewData(pushPreviewData);

const statusLabel = computed(() => {
  switch (connectionState.value) {
    case ConnectionState.Searching:
      return "搜索中...";
    case ConnectionState.Connected:
      return `已连接: ${selectedDevice.value?.ip ?? ""}`;
    default:
      return "未连接";
  }
});

const statusColor = computed(() => {
  switch (connectionState.value) {
    case ConnectionState.Connected:
      return "var(--success)";
    case ConnectionState.Searching:
      return "var(--warning)";
    default:
      return "var(--text-secondary)";
  }
});

async function handleSearch() {
  connectionState.value = ConnectionState.Searching;
  await searchDevices();

  if (devices.value.length === 1) {
    handleSelectDevice(devices.value[0]);
  } else if (devices.value.length > 1) {
    showDeviceSelector.value = true;
  } else {
    connectionState.value = ConnectionState.Disconnected;
  }
}

function handleSelectDevice(device: (typeof devices.value)[0]) {
  selectDevice(device);
  showDeviceSelector.value = false;
  handleConnect(device);
}

async function handleConnect(device: (typeof devices.value)[0]) {
  isConnecting.value = true;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("query_sys_status", {
      deviceIp: device.ip,
      enable: true,
    });
    connectionState.value = ConnectionState.Connected;
    startStatus();
    previewOutput.start();
  } catch (err) {
    console.error("Connection failed:", err);
    connectionState.value = ConnectionState.Disconnected;
    clearSelection();
  } finally {
    isConnecting.value = false;
  }
}

async function handleToggleStatus(enable: boolean) {
  if (!selectedDevice.value) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
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
  }
}

async function handleDisconnect() {
  stopStatus();
  previewOutput.stop();
  if (selectedDevice.value) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("query_sys_status", {
        deviceIp: selectedDevice.value.ip,
        enable: false,
      });
    } catch (err) {
      console.error("Disconnect failed:", err);
    }
  }
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
      <div class="toolbar-left">
        <span class="app-title">无人机采集卡上位机</span>
      </div>
      <div class="toolbar-center">
        <span class="connection-status" :style="{ color: statusColor }">
          {{ statusLabel }}
        </span>
      </div>
      <div class="toolbar-right">
        <button
          v-if="connectionState !== ConnectionState.Connected"
          :disabled="isSearching"
          @click="handleSearch"
        >
          {{ isSearching ? "搜索中..." : "搜索设备" }}
        </button>
      </div>
    </header>

    <!-- Main content: three columns -->
    <div class="main-content">
      <!-- Left: Control Panel -->
      <aside class="panel panel-left">
        <div class="panel-header">控制面板</div>
        <div class="panel-body">
          <ControlPanel
            :connected="connectionState === ConnectionState.Connected"
            :device-ip="selectedDevice?.ip"
            @search="handleSearch"
            @disconnect="handleDisconnect"
            @toggle-status="handleToggleStatus"
          />
        </div>
      </aside>

      <!-- Center: Chart -->
      <main class="panel panel-center">
        <div class="panel-header">数据预览</div>
        <div class="panel-body chart-body">
          <ChartView ref="chartRef" :channel-count="4" :max-points="4000" :channel-visible="channelVisible"
            @update:channel-visible="channelVisible = $event" />
        </div>
      </main>

      <!-- Right: Status Panel -->
      <aside class="panel panel-right">
        <div class="panel-header">设备状态</div>
        <div class="panel-body">
          <StatusPanel :status="status" />
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
  </div>
</template>

<style scoped>
.layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

/* Toolbar */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 40px;
  padding: 0 16px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}
.toolbar-left,
.toolbar-center,
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
.toolbar-left {
  min-width: 200px;
}
.toolbar-right {
  min-width: 200px;
  justify-content: flex-end;
}
.app-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--accent);
  white-space: nowrap;
  letter-spacing: 0.03em;
}

/* Main content area */
.main-content {
  display: flex;
  flex: 1;
  overflow: hidden;
  gap: 1px;
  background: var(--border-color);
}

/* Panels */
.panel {
  display: flex;
  flex-direction: column;
  background: var(--bg-secondary);
  overflow: hidden;
}
.panel-left {
  width: 260px;
  min-width: 200px;
  flex-shrink: 0;
}
.panel-center {
  flex: 1;
  min-width: 400px;
}
.panel-right {
  width: 340px;
  min-width: 280px;
  flex-shrink: 0;
}
.panel-header {
  padding: 9px 14px;
  font-size: 11px;
  font-weight: 700;
  color: var(--text-secondary);
  background: var(--bg-tertiary);
  border-bottom: 1px solid var(--border-color);
  text-transform: uppercase;
  letter-spacing: 1.2px;
  flex-shrink: 0;
}
.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}
.chart-body {
  padding: 0;
  display: flex;
}
.chart-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
}
.placeholder {
  color: var(--text-secondary);
  font-size: 13px;
  opacity: 0.6;
}
</style>
