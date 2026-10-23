<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import type { TemperaturePoint } from "../types";

const props = defineProps<{
  visible: boolean;
  points: TemperaturePoint[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);

const WIDTH = 640;
const HEIGHT = 300;

const pad2 = (n: number) => String(n).padStart(2, "0");

function fmtTime(ts: number): string {
  const d = new Date(ts);
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
}

function draw() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  canvas.width = WIDTH * dpr;
  canvas.height = HEIGHT * dpr;
  canvas.style.width = "100%";
  canvas.style.height = `${HEIGHT}px`;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, WIDTH, HEIGHT);

  const padL = 48;
  const padR = 16;
  const padT = 16;
  const padB = 30;
  const plotW = WIDTH - padL - padR;
  const plotH = HEIGHT - padT - padB;

  if (!props.points.length) {
    ctx.fillStyle = "#9aa4b2";
    ctx.font = "12px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText("暂无历史温度数据（每 10 秒记录一次）", WIDTH / 2, HEIGHT / 2);
    return;
  }

  const temps = props.points.map((p) => p.temp);
  let tMin = Math.min(...temps);
  let tMax = Math.max(...temps);
  if (tMax - tMin < 1) {
    const mid = (tMax + tMin) / 2;
    tMin = mid - 0.5;
    tMax = mid + 0.5;
  } else {
    const margin = (tMax - tMin) * 0.12;
    tMin -= margin;
    tMax += margin;
  }

  const t0 = props.points[0].time;
  const t1 = props.points[props.points.length - 1].time;
  const span = Math.max(t1 - t0, 10_000);

  const xOf = (time: number) => padL + ((time - t0) / span) * plotW;
  const yOf = (temp: number) => padT + (1 - (temp - tMin) / (tMax - tMin)) * plotH;

  // 横向网格与温度刻度
  ctx.lineWidth = 1;
  ctx.strokeStyle = "#edf1f6";
  ctx.fillStyle = "#8a94a6";
  ctx.font = "10.5px sans-serif";
  ctx.textAlign = "right";
  ctx.textBaseline = "middle";
  const yTicks = 4;
  for (let i = 0; i <= yTicks; i++) {
    const value = tMin + ((tMax - tMin) * i) / yTicks;
    const y = yOf(value);
    ctx.beginPath();
    ctx.moveTo(padL, y);
    ctx.lineTo(padL + plotW, y);
    ctx.stroke();
    ctx.fillText(value.toFixed(1), padL - 8, y);
  }

  // 纵向网格与时间刻度
  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  const xTicks = 4;
  for (let i = 0; i <= xTicks; i++) {
    const x = padL + (plotW * i) / xTicks;
    ctx.strokeStyle = "#f1f4f8";
    ctx.beginPath();
    ctx.moveTo(x, padT);
    ctx.lineTo(x, padT + plotH);
    ctx.stroke();

    const time = t0 + (span * i) / xTicks;
    ctx.fillStyle = "#8a94a6";
    ctx.fillText(fmtTime(time), x, padT + plotH + 8);
  }

  // 温度曲线
  ctx.strokeStyle = "#2563eb";
  ctx.lineWidth = 1.8;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.beginPath();
  props.points.forEach((p, i) => {
    const x = xOf(p.time);
    const y = yOf(p.temp);
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();

  // 采样点：以圆点形式显示
  ctx.fillStyle = "#2563eb";
  props.points.forEach((p) => {
    ctx.beginPath();
    ctx.arc(xOf(p.time), yOf(p.temp), 2, 0, Math.PI * 2);
    ctx.fill();
  });
}

watch(
  () => props.visible,
  (visible) => {
    if (visible) nextTick(draw);
  }
);

watch(
  () => props.points,
  () => {
    if (props.visible) nextTick(draw);
  },
  { deep: true }
);
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-overlay" @click.self="emit('close')">
      <div class="modal">
        <header class="modal-head">
          <div class="modal-titles">
            <h3>板载温度历史</h3>
            <p>{{ points.length ? `共 ${points.length} 个点 · 每 10 秒记录` : "暂无历史数据" }}</p>
          </div>
          <button class="close-btn" aria-label="关闭" @click="emit('close')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
              stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </header>

        <div class="modal-body">
          <canvas ref="canvasRef"></canvas>
        </div>

        <footer class="modal-actions">
          <button @click="emit('close')">关闭</button>
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
  width: 700px;
  max-width: calc(100vw - 40px);
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
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 18px 18px 14px;
  border-bottom: 1px solid var(--border-color);
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
}

.close-btn svg {
  width: 15px;
  height: 15px;
}

.modal-body {
  padding: 12px 14px 4px;
}

canvas {
  display: block;
  width: 100%;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 10px 18px 14px;
}

.modal-actions button {
  height: 30px;
  padding: 0 14px;
  font-size: 12.5px;
}
</style>
