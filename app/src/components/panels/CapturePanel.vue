<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useToast } from "../../composables/useToast";
import { useTriggerHelp } from "../../composables/useTriggerHelp";

const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
}>();

const toast = useToast();
const { open: openTriggerHelp } = useTriggerHelp();

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
  const enable = previewEnabled.value;
  try {
    await invoke("start_preview", {
      deviceIp: props.deviceIp,
      enable,
    });
  } catch (e) {
    // 下发失败时把按钮状态回滚，避免界面显示与设备实际状态不一致
    previewEnabled.value = !enable;
    console.error("start_preview failed:", e);
    toast.error(enable ? "开始采集失败" : "停止采集失败", String(e));
  }
}

function togglePreview() {
  if (!props.connected || !props.deviceIp) return;
  previewEnabled.value = !previewEnabled.value;
  setPreviewEnable();
}

async function sendParam(cmd: number, value: number) {
  if (!props.connected || !props.deviceIp) return;
  const buf = new ArrayBuffer(4);
  new DataView(buf).setUint32(0, value, false);
  try {
    await invoke("send_command", {
      deviceIp: props.deviceIp,
      cmd,
      data: Array.from(new Uint8Array(buf)),
    });
  } catch (e) {
    console.error(`send_command 0x${cmd.toString(16)} failed:`, e);
    toast.error(
      "参数下发失败",
      `命令 0x${cmd.toString(16)}：${String(e)}`
    );
  }
}

async function sendAllCaptureParams() {
  if (!props.connected || !props.deviceIp) return;
  settingInProgress.value = true;

  // 参考通道由 config.toml 的 [capture].ref_channal 指定，界面不显示，由后端下发
  try {
    await invoke("send_ref_channel", { deviceIp: props.deviceIp });
  } catch (e) {
    console.error("send_ref_channel failed:", e);
    toast.error("参考通道下发失败", String(e));
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
    await sendParam(cmd, value);
  }
  settingInProgress.value = false;
}

// ===== 信号采集模式 (0x0f03) =====
const trgMode = ref(0);
const trigModes = [
  { value: 0, label: "内触发" },
  { value: 1, label: "外触发" },
  { value: 2, label: "PIN触发" },
];

async function setTrgMode() {
  await sendParam(0x0f03, trgMode.value);
}
</script>

<template>
  <div class="tab-content">
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
      <h4 class="group-title">
        信号采集模式
        <button
          type="button"
          class="help-btn"
          title="查看三种触发方式示意图"
          aria-label="查看触发方式说明"
          @click="openTriggerHelp(trgMode)"
        >
          ?
        </button>
      </h4>
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
</template>
