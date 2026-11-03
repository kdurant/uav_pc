<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { SystemStatus } from "../types";
import LaserPanel from "./panels/LaserPanel.vue";
import MotorPanel from "./panels/MotorPanel.vue";
import GainPanel from "./panels/GainPanel.vue";
import CapturePanel from "./panels/CapturePanel.vue";
import StoragePanel from "./panels/StoragePanel.vue";

/**
 * 设置向导：按「连接 → 激光器 → 电机 → 增益 → 采集 → 存储」的顺序引导用户完成首次配置。
 * 每一步直接复用左侧控制面板的组件，避免重复实现设备通信逻辑。
 * 只有「连接设备」步骤强制要求连接成功，其余步骤可自由跳过。
 */
const props = defineProps<{
    visible: boolean;
    connected: boolean;
    deviceIp?: string;
    status?: SystemStatus | null;
    /** 是否正在读取设备状态（与主界面共用同一个状态源） */
    statusActive: boolean;
}>();

const emit = defineEmits<{
    (e: "update:visible", value: boolean): void;
    (e: "search"): void;
    (e: "toggle-status", enable: boolean): void;
}>();

const STEPS = [
    { key: "connect", title: "连接设备", desc: "搜索并连接采集卡，建议同时开启设备状态上报" },
    { key: "laser", title: "激光器", desc: "设置重频、电流、脉宽，确认开关状态与触发方式" },
    { key: "motor", title: "电机", desc: "初始化驱动器后设置速度/位置，或执行回零" },
    { key: "gain", title: "增益", desc: "设置 PMT 电压与 APD 高压" },
    { key: "capture", title: "采集", desc: "下发采集参数并开启预览" },
    { key: "storage", title: "存储", desc: "填写文件名并开启 SSD 存储" },
    { key: "done", title: "完成", desc: "向导已结束，可关闭后继续使用主界面" },
];

/** 步骤状态：未访问 / 已完成 / 已跳过 */
type StepState = "pending" | "done" | "skipped";

const current = ref(0);
const states = ref<Record<number, StepState>>({});

const currentStep = computed(() => STEPS[current.value]);
const isLast = computed(() => current.value === STEPS.length - 1);
/** 仅「连接设备」步骤强制：未连接不能继续 */
const canGoNext = computed(() => current.value !== 0 || props.connected);

function setState(index: number, state: StepState) {
    states.value = { ...states.value, [index]: state };
}

/** 跳转步骤：未连接时禁止跳到第 1 步之后 */
function goTo(index: number) {
    if (index < 0 || index >= STEPS.length) return;
    if (index > 0 && !props.connected) return;
    current.value = index;
}

function next() {
    if (!canGoNext.value) return;
    if (isLast.value) {
        close();
        return;
    }
    setState(current.value, "done");
    goTo(current.value + 1);
}

function skip() {
    // 连接步骤不允许跳过（否则后续步骤无法操作设备）
    if (current.value === 0) return;
    setState(current.value, "skipped");
    goTo(current.value + 1);
}

function prev() {
    goTo(current.value - 1);
}

function close() {
    emit("update:visible", false);
}

function onStatusToggle(event: Event) {
    emit("toggle-status", (event.target as HTMLInputElement).checked);
}

/** 步骤条右侧文字 */
function stateText(index: number): string {
    if (index === current.value) return "进行中";
    const state = states.value[index];
    if (state === "done") return "已完成";
    if (state === "skipped") return "已跳过";
    return "";
}

/** 完成页汇总（不含「完成」自身） */
const summary = computed(() =>
    STEPS.slice(0, STEPS.length - 1).map((step, index) => ({
        title: step.title,
        state: index === 0 ? (props.connected ? "已连接" : "未连接") : stateText(index) || "未设置",
        done: index === 0 ? props.connected : states.value[index] === "done",
    }))
);

/** 存储步骤的状态上报 / SATA 提示 */
const storageHint = computed(() => {
    if (!props.statusActive) return "未开启设备状态上报，无法判断 SATA 与存储状态，请回到第 1 步开启。";
    if (props.status && props.status.sata_dev_identify_done !== 1) return "SATA 未连接，无法开启存储，请检查硬盘连接。";
    return "";
});

function onKeydown(event: KeyboardEvent) {
    if (!props.visible) return;
    if (event.key === "Escape") close();
}

watch(
    () => props.visible,
    (visible) => {
        // 每次打开都从第一步开始，避免停留在上次的完成页
        if (visible) {
            current.value = 0;
            states.value = {};
        }
    }
);

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="props.visible" class="modal-overlay" @click.self="close">
      <div class="modal">
        <header class="modal-head">
          <div class="modal-title-wrap">
            <div class="modal-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
                stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="9" />
                <path d="M15.5 8.5l-2 5-5 2 2-5 5-2z" />
              </svg>
            </div>
            <div class="modal-titles">
              <h3>设置向导</h3>
              <p>按步骤完成设备初始化设置</p>
            </div>
          </div>
          <div class="head-right">
            <span class="step-count">第 {{ current + 1 }} / {{ STEPS.length }} 步</span>
            <button class="close-btn" aria-label="关闭" @click="close">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
            </button>
          </div>
        </header>

        <div class="wizard-body">
          <!-- 左侧步骤条 -->
          <aside class="stepper">
            <button
              v-for="(step, index) in STEPS"
              :key="step.key"
              class="step-item"
              :class="{
                active: index === current,
                done: states[index] === 'done',
                skipped: states[index] === 'skipped',
                locked: index > 0 && !props.connected,
              }"
              :disabled="index > 0 && !props.connected"
              @click="goTo(index)"
            >
              <span class="step-index">
                <svg v-if="states[index] === 'done'" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" stroke-width="2.4" stroke-linecap="round"
                  stroke-linejoin="round"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
                <template v-else>{{ index + 1 }}</template>
              </span>
              <span class="step-text">
                <span class="step-title">{{ step.title }}</span>
                <span v-if="stateText(index)" class="step-state">{{ stateText(index) }}</span>
              </span>
            </button>
          </aside>

          <!-- 右侧内容 -->
          <section class="step-content">
            <header class="step-head">
              <h4>{{ currentStep.title }}</h4>
              <p>{{ currentStep.desc }}</p>
            </header>

            <!-- 1. 连接设备 -->
            <div v-show="current === 0" class="step-pane">
              <div class="connect-card">
                <div class="connect-info">
                  <span class="conn-dot" :class="{ on: props.connected }"></span>
                  <div class="connect-text">
                    <p class="connect-title">
                      {{ props.connected ? "设备已连接" : "尚未连接设备" }}
                    </p>
                    <p class="connect-sub">
                      {{ props.connected ? (props.deviceIp || "未知设备") : "点击「搜索设备」查找局域网内的采集卡" }}
                    </p>
                  </div>
                </div>
                <button class="accent" @click="emit('search')">
                  {{ props.connected ? "重新搜索" : "搜索设备" }}
                </button>
              </div>

              <label class="status-row">
                <input type="checkbox" :checked="props.statusActive" @change="onStatusToggle" />
                <span class="status-text">
                  读取设备状态
                  <i>开启后可读取 SATA、温度、GPS 等信息，增益与存储步骤会用到</i>
                </span>
              </label>

              <p v-if="!props.connected" class="step-note warn">
                请先连接设备，连接成功后才能进入后续步骤。
              </p>
            </div>

            <!-- 2~6. 复用左侧控制面板 -->
            <div v-show="current === 1" class="step-pane panel-pane">
              <LaserPanel :connected="props.connected" :device-ip="props.deviceIp" />
            </div>
            <div v-show="current === 2" class="step-pane panel-pane">
              <MotorPanel :connected="props.connected" :device-ip="props.deviceIp" />
            </div>
            <div v-show="current === 3" class="step-pane panel-pane">
              <GainPanel
                :connected="props.connected"
                :device-ip="props.deviceIp"
                :status="props.status"
              />
            </div>
            <div v-show="current === 4" class="step-pane panel-pane">
              <CapturePanel :connected="props.connected" :device-ip="props.deviceIp" />
            </div>
            <div v-show="current === 5" class="step-pane panel-pane">
              <p v-if="storageHint" class="step-note warn">{{ storageHint }}</p>
              <StoragePanel
                :connected="props.connected"
                :device-ip="props.deviceIp"
                :status="props.status"
              />
            </div>

            <!-- 7. 完成 -->
            <div v-show="current === 6" class="step-pane">
              <ul class="summary">
                <li v-for="item in summary" :key="item.title" class="summary-item">
                  <span class="summary-dot" :class="{ done: item.done }">
                    <svg v-if="item.done" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                      stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M5 12.5l4.5 4.5L19 7.5" />
                    </svg>
                  </span>
                  <span class="summary-title">{{ item.title }}</span>
                  <span class="summary-state">{{ item.state }}</span>
                </li>
              </ul>
              <p class="step-note">
                向导中的设置会直接下发到设备；本机 IP、端口等配置文件参数请使用顶部「设置」修改（保存后立即生效，本机 IP / 端口需重启）。
              </p>
            </div>
          </section>
        </div>

        <footer class="modal-actions">
          <span v-if="!canGoNext" class="nav-hint">请先连接设备</span>
          <button :disabled="current === 0" @click="prev">上一步</button>
          <button v-if="current > 0 && !isLast" @click="skip">跳过</button>
          <button v-if="!isLast" class="accent" :disabled="!canGoNext" @click="next">
            下一步
          </button>
          <button v-else class="accent" @click="close">完成</button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(15, 23, 42, 0.32);
  z-index: 1000;
  animation: overlay-in 0.16s ease-out;
}

@keyframes overlay-in {
  from {
    opacity: 0;
  }
}

.modal {
  display: flex;
  flex-direction: column;
  width: 820px;
  max-width: calc(100vw - 40px);
  height: min(640px, calc(100vh - 80px));
  background: #fff;
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: 0 2px 8px rgba(16, 24, 40, 0.1);
  overflow: hidden;
  animation: modal-in 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes modal-in {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.985);
  }
}

.modal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 18px 14px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.modal-title-wrap {
  display: flex;
  align-items: center;
  gap: 11px;
}

.modal-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  background: var(--accent-soft);
  color: var(--accent);
  flex-shrink: 0;
}

.modal-icon svg {
  width: 19px;
  height: 19px;
}

.modal-titles h3 {
  font-size: 15px;
  font-weight: 650;
  color: var(--text-primary);
  letter-spacing: 0.01em;
}

.modal-titles p {
  margin-top: 2px;
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.head-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.step-count {
  font-size: 11.5px;
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.close-btn {
  width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: var(--text-tertiary);
  box-shadow: none;
  flex-shrink: 0;
}

.close-btn:hover {
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border-color: transparent;
}

.close-btn svg {
  width: 15px;
  height: 15px;
}

/* 主体：步骤条 + 内容 */
.wizard-body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.stepper {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 196px;
  padding: 14px 10px;
  border-right: 1px solid var(--border-color);
  background: var(--bg-inset);
  flex-shrink: 0;
  overflow-y: auto;
}

.step-item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 40px;
  padding: 0 9px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  box-shadow: none;
  text-align: left;
  justify-content: flex-start;
  color: var(--text-secondary);
  font-size: 12.5px;
}

.step-item:hover {
  background: var(--bg-hover);
  border-color: transparent;
  color: var(--text-primary);
}

.step-item.active {
  background: var(--accent-soft);
  color: var(--accent-dim);
}

.step-item.locked {
  opacity: 0.5;
  cursor: not-allowed;
}

.step-index {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 1px solid var(--border-strong);
  background: #fff;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.step-item.active .step-index {
  border-color: var(--accent);
  color: var(--accent);
}

.step-item.done .step-index {
  border-color: var(--success);
  background: var(--success-soft);
  color: var(--success);
}

.step-item.done .step-index svg {
  width: 12px;
  height: 12px;
}

.step-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.step-title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.step-state {
  font-size: 10px;
  color: var(--text-tertiary);
}

.step-item.active .step-state {
  color: var(--accent);
}

.step-content {
  flex: 1;
  min-width: 0;
  padding: 16px 20px 20px;
  overflow-y: auto;
}

.step-head {
  margin-bottom: 14px;
}

.step-head h4 {
  font-size: 14px;
  font-weight: 650;
  color: var(--text-primary);
}

.step-head p {
  margin-top: 2px;
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.step-pane {
  max-width: 460px;
}

/* 连接步骤 */
.connect-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  background: var(--bg-inset);
}

.connect-info {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.conn-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--border-strong);
  flex-shrink: 0;
}

.conn-dot.on {
  background: var(--success);
  box-shadow: 0 0 0 3px var(--success-soft);
}

.connect-text {
  min-width: 0;
}

.connect-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.connect-sub {
  margin-top: 1px;
  font-size: 11.5px;
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.status-row {
  display: flex;
  align-items: flex-start;
  gap: 9px;
  margin-top: 12px;
  padding: 11px 12px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  cursor: pointer;
}

.status-row input {
  width: 15px;
  height: 15px;
  margin-top: 1px;
  accent-color: var(--accent);
  flex-shrink: 0;
}

.status-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 12.5px;
  color: var(--text-primary);
}

.status-text i {
  font-style: normal;
  font-size: 11px;
  color: var(--text-tertiary);
}

.step-note {
  margin-top: 12px;
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--text-tertiary);
}

.step-note.warn {
  padding: 8px 10px;
  border: 1px solid #fde3b8;
  border-radius: var(--radius-sm);
  background: var(--warning-soft);
  color: var(--warning);
}

.panel-pane {
  max-width: 440px;
}

/* 完成页汇总 */
.summary {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 2px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  overflow: hidden;
}

.summary-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  font-size: 12.5px;
  background: #fff;
}

.summary-item + .summary-item {
  border-top: 1px solid var(--border-color);
}

.summary-dot {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 1px solid var(--border-strong);
  color: #fff;
  flex-shrink: 0;
}

.summary-dot.done {
  border-color: var(--success);
  background: var(--success);
}

.summary-dot svg {
  width: 11px;
  height: 11px;
}

.summary-title {
  flex: 1;
  color: var(--text-primary);
}

.summary-state {
  font-size: 11.5px;
  color: var(--text-tertiary);
}

/* 底部操作栏 */
.modal-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 18px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-inset);
  flex-shrink: 0;
}

.nav-hint {
  margin-right: auto;
  font-size: 11.5px;
  color: var(--warning);
}
</style>
