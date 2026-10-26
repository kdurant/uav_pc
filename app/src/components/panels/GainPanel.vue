<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SystemStatus } from "../../types";
import { useToast } from "../../composables/useToast";

const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
  status?: SystemStatus | null;
}>();

const toast = useToast();

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
    const digital =
      channel === 3
        ? apdToDigital(apdVoltage.value)
        : pmtToDigital(pmtVoltages.value[channel]);
    await sendDaChannel(channel, digital);
  } catch (e) {
    console.error(`set DA channel ${channel} failed:`, e);
    const name = channel === 3 ? "APD 高压" : `PMT${channel}`;
    toast.error(`${name}设置失败`, String(e));
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
</script>

<template>
  <div class="tab-content">
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
          <span class="channel-feedback" title="设备回读值">{{ pmtFeedbackText(0) }}</span>
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
          <span class="channel-feedback" title="设备回读值">{{ pmtFeedbackText(1) }}</span>
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
          <span class="channel-feedback" title="设备回读值">{{ pmtFeedbackText(2) }}</span>
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
          <span class="channel-feedback" title="设备回读值">{{ apdFeedbackText() }}</span>
        </div>
      </div>

      <p class="hint">ch0~ch2 = PMT0~2（0~5V），ch3 = APD 高压（140~300V）</p>
      <p class="hint">右侧为设备回读值，随状态更新（约 1s 一次）。</p>
    </section>
  </div>
</template>
