<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { MotorState } from "../../types";

const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
}>();

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

/** 断开连接时清空设备读数 */
watch(
  () => props.connected,
  (connected) => {
    if (!connected) {
      motorState.value = null;
      motorError.value = "";
    }
  }
);
</script>

<template>
  <div class="tab-content">
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
</template>
