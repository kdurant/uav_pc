<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    /** 方位角（°），0=北，顺时针递增；null 表示无数据 */
    azimuth?: number | null;
    /** 俯仰角（°），抬头为正；null 表示无数据 */
    pitch?: number | null;
    /** 横滚角（°），右倾为正；null 表示无数据 */
    roll?: number | null;
  }>(),
  { azimuth: null, pitch: null, roll: null }
);

const canvasRef = ref<HTMLCanvasElement | null>(null);

// ===== 航空惯例符号（实机方向相反时改这里） =====
/** 屏幕旋转：方位角顺时针为正，因此绕 Y 轴取负 */
const YAW_SIGN = -1;
/** 俯仰：抬头为正 */
const PITCH_SIGN = 1;
/** 横滚：右倾为正（右翼向下） */
const ROLL_SIGN = -1;

const DEG = Math.PI / 180;
const VIEW_TILT = 55 * DEG; // 俯视角：兼顾航向直观性与俯仰/横滚的可读性
const CAM_DIST = 6;

// ===== 线框飞机模型（机头朝 -Z，右翼 +X，机背 +Y） =====
const MODEL: number[][][] = [
  // 机身
  [[0, 0, 0.95], [0, 0, -1.15]],
  // 主翼（左翼尖 → 翼根 → 右翼尖）
  [[-1.15, 0, 0.35], [-0.12, 0, -0.05], [0.12, 0, -0.05], [1.15, 0, 0.35]],
  // 平尾
  [[-0.45, 0, 0.9], [-0.1, 0, 0.72], [0.1, 0, 0.72], [0.45, 0, 0.9]],
  // 垂尾
  [[0, 0, 0.9], [0, 0.45, 1.0], [0, 0, 1.0]],
];

const current = { azimuth: 0, pitch: 0, roll: 0 };
const target = { azimuth: 0, pitch: 0, roll: 0 };
let initialized = false;
let rafId = 0;
let lastTs = 0;
let viewW = 0;
let viewH = 0;

/** 归一化到 [0, 360) */
function normalize360(v: number): number {
  return ((v % 360) + 360) % 360;
}

function fmtAngle(v: number | null | undefined): string {
  return v === undefined || v === null || !Number.isFinite(v) ? "-" : v.toFixed(2) + "°";
}

/** 三维点 → 画布坐标 */
function project(
  p: number[],
  cx: number,
  cy: number,
  scale: number
): [number, number] {
  let [x, y, z] = p;

  // 横滚（绕机头轴 Z）
  const cr = Math.cos(current.roll * ROLL_SIGN * DEG);
  const sr = Math.sin(current.roll * ROLL_SIGN * DEG);
  let x1 = x * cr - y * sr;
  let y1 = x * sr + y * cr;
  let z1 = z;

  // 俯仰（绕 X）
  const cp = Math.cos(current.pitch * PITCH_SIGN * DEG);
  const sp = Math.sin(current.pitch * PITCH_SIGN * DEG);
  let x2 = x1;
  let y2 = y1 * cp - z1 * sp;
  let z2 = y1 * sp + z1 * cp;

  // 方位（绕 Y）
  const cyw = Math.cos(current.azimuth * YAW_SIGN * DEG);
  const syw = Math.sin(current.azimuth * YAW_SIGN * DEG);
  let x3 = x2 * cyw + z2 * syw;
  let y3 = y2;
  let z3 = -x2 * syw + z2 * cyw;

  // 固定俯视角
  const cv = Math.cos(VIEW_TILT);
  const sv = Math.sin(VIEW_TILT);
  let x4 = x3;
  let y4 = y3 * cv - z3 * sv;
  let z4 = y3 * sv + z3 * cv;

  // 透视投影
  const persp = CAM_DIST / (CAM_DIST + z4);
  return [cx + x4 * scale * persp, cy - y4 * scale * persp];
}

function draw() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx || viewW <= 0 || viewH <= 0) return;

  const w = viewW;
  const h = viewH;
  ctx.clearRect(0, 0, w, h);

  const cx = w / 2;
  const cy = h / 2 + 4;
  const r = Math.min(w, h) * 0.4;

  // ---- 罗盘环 ----
  ctx.strokeStyle = "#dbe1ea";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.arc(cx, cy, r, 0, Math.PI * 2);
  ctx.stroke();

  // 刻度
  ctx.strokeStyle = "#c8d1df";
  for (let i = 0; i < 12; i++) {
    const a = i * 30 * DEG;
    const inner = i % 3 === 0 ? r - 6 : r - 3;
    ctx.beginPath();
    ctx.moveTo(cx + Math.sin(a) * inner, cy - Math.cos(a) * inner);
    ctx.lineTo(cx + Math.sin(a) * r, cy - Math.cos(a) * r);
    ctx.stroke();
  }

  // N/E/S/W
  ctx.fillStyle = "#93a0b4";
  ctx.font = '600 9px "Inter", "PingFang SC", sans-serif';
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  const cardinals: [string, number][] = [
    ["N", 0],
    ["E", 90],
    ["S", 180],
    ["W", 270],
  ];
  for (const [label, deg] of cardinals) {
    const a = deg * DEG;
    ctx.fillText(label, cx + Math.sin(a) * (r + 10), cy - Math.cos(a) * (r + 10));
  }

  // 当前航向标记
  const azRad = current.azimuth * DEG;
  const hx = cx + Math.sin(azRad) * r;
  const hy = cy - Math.cos(azRad) * r;
  ctx.fillStyle = "#f59e0b";
  ctx.beginPath();
  ctx.arc(hx, hy, 3.5, 0, Math.PI * 2);
  ctx.fill();

  // ---- 线框飞机 ----
  const scale = Math.min(w, h) * 0.22;
  ctx.strokeStyle = "#2f6fed";
  ctx.lineWidth = 1.6;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";

  for (const line of MODEL) {
    ctx.beginPath();
    for (let i = 0; i < line.length; i++) {
      const [px, py] = project(line[i], cx, cy, scale);
      if (i === 0) ctx.moveTo(px, py);
      else ctx.lineTo(px, py);
    }
    ctx.stroke();
  }

  // 机头点
  const [nx, ny] = project([0, 0, -1.15], cx, cy, scale);
  ctx.fillStyle = "#2f6fed";
  ctx.beginPath();
  ctx.arc(nx, ny, 2.2, 0, Math.PI * 2);
  ctx.fill();
}

function startAnimation() {
  if (rafId) return;
  lastTs = performance.now();
  rafId = requestAnimationFrame(tick);
}

function tick(ts: number) {
  const dt = Math.min(Math.max((ts - lastTs) / 1000, 0), 0.1);
  lastTs = ts;

  // 指数平滑，约 250~300ms 收敛
  const k = 1 - Math.exp(-dt / 0.1);
  current.azimuth += (target.azimuth - current.azimuth) * k;
  current.pitch += (target.pitch - current.pitch) * k;
  current.roll += (target.roll - current.roll) * k;
  draw();

  const settled =
    Math.abs(target.azimuth - current.azimuth) < 0.05 &&
    Math.abs(target.pitch - current.pitch) < 0.05 &&
    Math.abs(target.roll - current.roll) < 0.05;

  if (settled) {
    current.azimuth = normalize360(current.azimuth);
    current.pitch = target.pitch;
    current.roll = target.roll;
    target.azimuth = current.azimuth;
    rafId = 0;
    draw();
  } else {
    rafId = requestAnimationFrame(tick);
  }
}

/** 状态更新（1Hz）后设置目标角度，方位角走最短路径 */
function updateTargets() {
  const az = normalize360(props.azimuth ?? 0);
  const pitch = props.pitch ?? 0;
  const roll = props.roll ?? 0;
  if (!initialized) {
    current.azimuth = az;
    current.pitch = pitch;
    current.roll = roll;
    target.azimuth = az;
    target.pitch = pitch;
    target.roll = roll;
    initialized = true;
    draw();
    return;
  }

  const delta = ((az - normalize360(current.azimuth) + 540) % 360) - 180;
  target.azimuth = current.azimuth + delta;
  target.pitch = pitch;
  target.roll = roll;
  startAnimation();
}

watch(
  () => [props.azimuth, props.pitch, props.roll],
  () => updateTargets(),
  { immediate: true }
);

function resize() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return;

  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  viewW = rect.width;
  viewH = rect.height;
  canvas.width = Math.round(rect.width * dpr);
  canvas.height = Math.round(rect.height * dpr);

  const ctx = canvas.getContext("2d");
  if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  draw();
}

let resizeObserver: ResizeObserver | null = null;

onMounted(() => {
  resize();
  const canvas = canvasRef.value;
  if (canvas) {
    resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(canvas);
  }
});

onUnmounted(() => {
  if (rafId) cancelAnimationFrame(rafId);
  rafId = 0;
  resizeObserver?.disconnect();
});
</script>

<template>
  <div class="attitude-view">
    <div class="att-canvas-wrap">
      <canvas ref="canvasRef"></canvas>
    </div>
    <div class="att-values">
      <div class="att-item">
        <span>方位角</span>
        <b>{{ fmtAngle(azimuth) }}</b>
      </div>
      <div class="att-item">
        <span>俯仰角</span>
        <b>{{ fmtAngle(pitch) }}</b>
      </div>
      <div class="att-item">
        <span>横滚角</span>
        <b>{{ fmtAngle(roll) }}</b>
      </div>
    </div>
  </div>
</template>

<style scoped>
.attitude-view {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.att-canvas-wrap {
  height: 190px;
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

canvas {
  display: block;
  width: 100%;
  height: 100%;
}

.att-values {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
}

.att-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1px;
  padding: 5px 4px;
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-xs);
}

.att-item span {
  font-size: 10px;
  color: var(--text-tertiary);
}

.att-item b {
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}
</style>
