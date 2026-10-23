<script setup lang="ts">
import { onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SystemStatus } from "../../types";
import PowerDialog from "../PowerDialog.vue";

const props = defineProps<{
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

const statusPollingEnabled = ref(true);

function toggleStatusPolling() {
  emit("toggle-status", statusPollingEnabled.value);
}

/** 运行时间：秒 → HH:MM:SS，无数据显示 "-" */
function formatUptime(seconds: number | undefined): string {
  if (seconds === undefined) return "-";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const sec = seconds % 60;
  return [h, m, sec].map((v) => String(v).padStart(2, "0")).join(":");
}

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
    console.error("power command failed:", e);
  } finally {
    powerBusy.value = false;
  }
}

onUnmounted(stopPowerCountdown);
</script>

<template>
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
  </section>
</template>
