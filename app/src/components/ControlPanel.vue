<script setup lang="ts">
import { ref } from "vue";
import type { SystemStatus } from "../types";
import ConnectionPanel from "./panels/ConnectionPanel.vue";
import CapturePanel from "./panels/CapturePanel.vue";
import GainPanel from "./panels/GainPanel.vue";
import LaserPanel from "./panels/LaserPanel.vue";
import MotorPanel from "./panels/MotorPanel.vue";
import StoragePanel from "./panels/StoragePanel.vue";

defineProps<{
  connected: boolean;
  deviceIp?: string;
  status?: SystemStatus | null;
}>();

const emit = defineEmits<{
  (e: "search"): void;
  (e: "disconnect"): void;
  (e: "toggle-status", enable: boolean): void;
  (e: "powered", action: "reboot" | "shutdown"): void;
}>();

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
    <ConnectionPanel
      :connected="connected"
      :device-ip="deviceIp"
      :status="status"
      @search="emit('search')"
      @disconnect="emit('disconnect')"
      @toggle-status="emit('toggle-status', $event)"
      @powered="emit('powered', $event)"
    />

    <div class="tab-bar">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        class="tab-btn"
        :class="{ active: activeTab === tab.key }"
        @click="selectTab(tab.key)"
      >{{ tab.label }}</button>
    </div>

    <LaserPanel
      v-show="activeTab === 'laser'"
      :connected="connected"
      :device-ip="deviceIp"
    />
    <MotorPanel
      v-show="activeTab === 'motor'"
      :connected="connected"
      :device-ip="deviceIp"
    />
    <GainPanel
      v-show="activeTab === 'voltage'"
      :connected="connected"
      :device-ip="deviceIp"
      :status="status"
    />
    <CapturePanel
      v-show="activeTab === 'capture'"
      :connected="connected"
      :device-ip="deviceIp"
    />
    <StoragePanel
      v-show="activeTab === 'storage'"
      :connected="connected"
      :device-ip="deviceIp"
    />
  </div>
</template>
