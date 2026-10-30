<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { LaserStatus } from "../../types";
import { useToast } from "../../composables/useToast";
import { useRadarConfig } from "../../composables/useRadarConfig";

const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
}>();

const toast = useToast();
const { laserType, load: loadRadarConfig } = useRadarConfig();

onMounted(loadRadarConfig);

/** 当前支持 laser_connet（含设备端旧写法）和 laser_5kHz */
const laserSupported = computed(() =>
  ["laser_connet", "laserConnet", "laser_5kHz"].includes(laserType.value)
);
const laserUnavailable = computed(() => !laserSupported.value);

/** 命令动作 → 中文提示名，用于失败通知 */
const LASER_ACTION_LABELS: Record<string, string> = {
  open: "打开激光器",
  close: "关闭激光器",
  freq: "设置重频",
  current: "设置电流",
  mode: "设置触发模式",
  pulse_width: "设置脉宽",
  clear_error: "清除报警",
  status: "查询状态",
};

// ===== 激光器控制（0x0200 透传 / 0x0201 重频） =====
const laserFreq = ref(20000);
const laserEnabled = ref(false);
const laserMode = ref(1); // 0 内触发，1 外触发
const laserCurrent = ref(1000);
const laserPulseWidth = ref(60);
const laserStatus = ref<LaserStatus | null>(null);
const laserBusy = ref(false);

const laserFreqMax = computed(() =>
  laserType.value === "laser_5kHz" ? 5000 : 100000
);

watch(
  laserType,
  (type) => {
    if (type === "laser_5kHz" && laserFreq.value > 5000) {
      laserFreq.value = 5000;
    }
  },
  { immediate: true }
);

async function laserAction(action: string, value?: number): Promise<boolean> {
  if (!props.connected || !props.deviceIp) return false;
  if (laserUnavailable.value) {
    toast.error("激光器不可用", "当前激光器协议未配置或不支持，请检查配置文件");
    return false;
  }
  laserBusy.value = true;
  try {
    const status = await invoke<LaserStatus | null>("laser_command", {
      deviceIp: props.deviceIp,
      action,
      value,
    });
    if (status) laserStatus.value = status;
    return true;
  } catch (e) {
    console.error(`laser_command ${action} failed:`, e);
    toast.error(
      `${LASER_ACTION_LABELS[action] ?? action}失败`,
      String(e)
    );
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
    if (laserType.value === "laser_5kHz") {
      // 5kHz 激光器先设置 LD 预置电流，再一键开光
      const currentOk = await laserAction("current", Math.trunc(laserCurrent.value));
      if (!currentOk) {
        laserEnabled.value = false;
        return;
      }
      const opened = await laserAction("open");
      if (!opened) {
        laserEnabled.value = false;
      }
      return;
    }
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

async function setLaserPulseWidth() {
  await laserAction("pulse_width", Math.trunc(laserPulseWidth.value));
}

async function clearLaserError() {
  await laserAction("clear_error");
}

async function queryLaserStatus() {
  const ok = await laserAction("status");
  if (!ok || !laserStatus.value) return;

  const s = laserStatus.value;
  laserEnabled.value = s.status !== 0;

  // 5kHz 状态帧带回设定值，同步到输入框，便于现场核对
  if (s.status_flags != null) {
    laserMode.value = s.status_flags & 0x02 ? 1 : 0;
  }
  if (s.freq_set_hz != null && s.freq_set_hz > 0) {
    laserFreq.value = s.freq_set_hz;
  }
  if (s.pulse_width_us != null) {
    laserPulseWidth.value = s.pulse_width_us;
  }
  if (s.ld_current_ma != null) {
    laserCurrent.value = s.ld_current_ma;
  }
}

/** 断开连接时清空设备读数 */
watch(
  () => props.connected,
  (connected) => {
    if (!connected) {
      laserStatus.value = null;
    }
  }
);

const laserStatusItems = computed(() => {
  const s = laserStatus.value;
  const dash = "-";

  if (laserType.value === "laser_5kHz") {
    return [
      { label: "开关状态", value: s ? (s.status === 0 ? "关闭" : "开启") : dash },
      {
        label: "触发模式",
        value:
          s?.status_flags != null
            ? s.status_flags & 0x02
              ? "外触发"
              : "内触发"
            : dash,
      },
      {
        label: "检测频率",
        value: s?.freq_detected_hz != null ? `${s.freq_detected_hz} Hz` : dash,
      },
      {
        label: "设定频率",
        value: s?.freq_set_hz != null ? `${s.freq_set_hz} Hz` : dash,
      },
      {
        label: "脉宽",
        value: s?.pulse_width_us != null ? `${s.pulse_width_us} µs` : dash,
      },
      {
        label: "LD电流",
        value: s?.ld_current_ma != null ? `${s.ld_current_ma} mA` : dash,
      },
      { label: "温度1", value: s?.temp1_c != null ? `${s.temp1_c} ℃` : dash },
      { label: "温度2", value: s?.temp2_c != null ? `${s.temp2_c} ℃` : dash },
      { label: "调Q延时", value: s?.q_delay_us != null ? `${s.q_delay_us} µs` : dash },
      { label: "工作时间", value: s?.work_time_s != null ? `${s.work_time_s} s` : dash },
      {
        label: "错误码",
        value: s ? `0x${s.error_code.toString(16).padStart(2, "0")}` : dash,
      },
    ];
  }

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

      <p v-if="laserUnavailable" class="laser-warning">
        当前激光器协议不可用，激光器控制已禁用。请检查配置文件。
      </p>

      <div class="field-list">
        <div class="field">
          <span class="field-label">重频 (Hz)</span>
          <div class="field-inline">
            <input
              type="number"
              v-model.number="laserFreq"
              min="1"
              :max="laserFreqMax"
              :disabled="laserBusy || laserUnavailable"
            />
            <button class="set-btn" :disabled="laserBusy || laserUnavailable" @click="setLaserFreq">设置</button>
          </div>
        </div>
        <div class="field">
          <span class="field-label">电流 (mA)</span>
          <div class="field-inline">
            <input type="number" v-model.number="laserCurrent" min="0" max="6000" :disabled="laserBusy || laserUnavailable" />
            <button class="set-btn" :disabled="laserBusy || laserUnavailable" @click="setLaserCurrent">设置</button>
          </div>
        </div>
        <div v-if="laserType === 'laser_5kHz'" class="field">
          <span class="field-label">脉宽 (µs)</span>
          <div class="field-inline">
            <input
              type="number"
              v-model.number="laserPulseWidth"
              min="50"
              max="80"
              :disabled="laserBusy || laserUnavailable"
            />
            <button class="set-btn" :disabled="laserBusy || laserUnavailable" @click="setLaserPulseWidth">
              设置
            </button>
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
              :disabled="laserBusy || laserUnavailable"
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
              :disabled="laserBusy || laserUnavailable"
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
          <input type="checkbox" v-model="laserEnabled" :disabled="laserBusy || laserUnavailable" @change="toggleLaser" />
          <span class="switch-track"><span class="switch-thumb"></span></span>
        </label>
      </div>

      <button
        class="accent block status-query"
        :disabled="laserBusy || laserUnavailable"
        @click="queryLaserStatus"
      >
        {{ laserUnavailable ? "协议未配置" : laserBusy ? "查询中..." : "查询状态" }}
      </button>
      <button
        v-if="laserType === 'laser_5kHz'"
        class="block"
        :disabled="laserBusy || laserUnavailable"
        @click="clearLaserError"
      >
        清除报警
      </button>

      <div class="feedback-list">
        <div v-for="item in laserStatusItems" :key="item.label" class="feedback-row">
          <span class="feedback-name">{{ item.label }}</span>
          <span class="feedback-val">{{ item.value }}</span>
        </div>
      </div>

      <p class="hint">未查询或未连接时显示占位值；点击“查询状态”刷新读数。</p>
      <p class="hint">激光器命令经采集卡透传，单次操作最长等待 1 秒；打开时会自动重新下发电流。</p>
    </section>
  </div>
</template>
