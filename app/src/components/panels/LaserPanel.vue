<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { LaserStatus } from "../../types";

const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
}>();

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
    await laserAction("current", Math.trunc(laserCurrent.value));
  } else {
    const closed = await laserAction("close");
    if (!closed) laserEnabled.value = true;
  }
}

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
    laserEnabled.value = laserStatus.value.status !== 0;
  }
}

/** 断开连接时清空设备读数 */
watch(
  () => props.connected,
  (connected) => {
    if (!connected) {
      laserStatus.value = null;
      laserError.value = "";
    }
  }
);

const laserStatusItems = computed(() => {  const s = laserStatus.value;
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
</script>

<template>
  <div class="tab-content">
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
</template>
