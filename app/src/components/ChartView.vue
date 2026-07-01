<script setup lang="ts">
import { ref, onMounted, onUnmounted, shallowRef } from "vue";
import { useChart } from "../composables/useChart";

const props = withDefaults(
  defineProps<{
    channelCount?: number;
    maxPoints?: number;
    lineColors?: string[];
    channelVisible?: boolean[];
  }>(),
  {
    channelCount: 4,
    maxPoints: 4000,
    lineColors: () => ["#00d4ff", "#ff6b6b", "#ffd93d", "#6bcb77"],
    channelVisible: () => [true, true, true, true],
  }
);

const canvasRef = ref<HTMLCanvasElement | null>(null);
const offscreenRef = shallowRef<HTMLCanvasElement | null>(null);
const containerRef = ref<HTMLDivElement | null>(null);

// 缩放状态
const zoomFactor = ref(1.0);
const visiblePoints = ref(props.maxPoints);

// 网格缓存
let lastYMin = 0;
let lastYMax = 0;
let lastWidth = 0;

const gridColor = "rgba(42, 42, 74, 0.5)";
const textColor = "#a0a0b0";
const fontSize = 10;

/** 鼠标滚轮缩放 */
function onWheel(e: WheelEvent) {
  e.preventDefault();
  if (e.ctrlKey || e.metaKey) {
    // Ctrl+滚轮: Y轴缩放
    const delta = e.deltaY > 0 ? 1.15 : 1 / 1.15;
    zoomFactor.value = Math.max(0.1, Math.min(50, zoomFactor.value * delta));
  } else if (e.shiftKey) {
    // Shift+滚轮: X轴缩放（调整可见点数）
    const delta = e.deltaY > 0 ? 100 : -100;
    visiblePoints.value = Math.max(100, Math.min(props.maxPoints, visiblePoints.value + delta));
  } else {
    // 纯滚轮: X轴缩放
    const delta = e.deltaY > 0 ? 1.1 : 1 / 1.1;
    visiblePoints.value = Math.max(100, Math.min(props.maxPoints, visiblePoints.value * delta));
  }
  chart.markDirty();
}

function resetZoom() {
  zoomFactor.value = 1.0;
  visiblePoints.value = props.maxPoints;
  chart.markDirty();
}

/** 离屏渲染回调 */
function render(buffers: Float64Array[], width: number, height: number) {
  const offscreen = offscreenRef.value;
  if (!offscreen) return;
  const ctx = offscreen.getContext("2d");
  if (!ctx) return;

  const margin = { top: 8, right: 12, bottom: 20, left: 50 };
  const plotW = width - margin.left - margin.right;
  const plotH = height - margin.top - margin.bottom;

  // 计算可见范围内的数据（X轴滚动窗口）
  const startIdx = Math.max(0, (buffers[0]?.length || 0) - Math.round(visiblePoints.value));
  const visibleLen = Math.min(Math.round(visiblePoints.value), buffers[0]?.length || 0);

  // 计算 Y 轴范围（仅可见通道 + 可见数据范围）
  let yMin = Infinity;
  let yMax = -Infinity;
  for (let ch = 0; ch < buffers.length; ch++) {
    if (!props.channelVisible[ch]) continue;
    const buf = buffers[ch];
    const end = Math.min(startIdx + visibleLen, buf.length);
    for (let i = startIdx; i < end; i++) {
      const v = buf[i];
      if (v < yMin) yMin = v;
      if (v > yMax) yMax = v;
    }
  }
  if (yMin === Infinity) { yMin = 0; yMax = 4096; }
  // 应用缩放
  const range = (yMax - yMin) || 1;
  const center = (yMin + yMax) / 2;
  yMin = center - (range / 2) * zoomFactor.value;
  yMax = center + (range / 2) * zoomFactor.value;

  const needsGridRedraw = yMin !== lastYMin || yMax !== lastYMax || width !== lastWidth;

  // 清除
  ctx.clearRect(0, 0, width, height);

  // 背景
  ctx.fillStyle = "#1a1a2e";
  ctx.fillRect(0, 0, width, height);

  // 绘图区域边框
  ctx.strokeStyle = gridColor;
  ctx.lineWidth = 1;
  ctx.strokeRect(margin.left, margin.top, plotW, plotH);

  if (needsGridRedraw) {
    lastYMin = yMin;
    lastYMax = yMax;
    lastWidth = width;

    // 水平网格线
    const gridLines = 5;
    ctx.strokeStyle = gridColor;
    ctx.fillStyle = textColor;
    ctx.font = `${fontSize}px monospace`;
    ctx.textAlign = "right";
    ctx.lineWidth = 0.5;

    for (let i = 0; i <= gridLines; i++) {
      const y = margin.top + (plotH / gridLines) * i;
      const val = yMax - ((yMax - yMin) / gridLines) * i;

      ctx.beginPath();
      ctx.moveTo(margin.left, y);
      ctx.lineTo(margin.left + plotW, y);
      ctx.stroke();

      const label = val >= 1000 ? (val / 1000).toFixed(1) + "k" : val.toFixed(0);
      ctx.fillText(label, margin.left - 4, y + 3);
    }

    // X 轴标签
    ctx.textAlign = "center";
    const xLabels = 4;
    for (let i = 0; i <= xLabels; i++) {
      const x = margin.left + (plotW / xLabels) * i;
      const idx = Math.round(startIdx + (visibleLen / xLabels) * i);
      ctx.fillText(String(idx), x, height - 2);
    }
  }

  // 裁剪
  ctx.save();
  ctx.beginPath();
  ctx.rect(margin.left, margin.top, plotW, plotH);
  ctx.clip();

  // 绘制折线
  ctx.lineWidth = 1.2;
  const totalDrawn = Math.max(1, visibleLen - 1);

  for (let ch = 0; ch < buffers.length; ch++) {
    if (!props.channelVisible[ch]) continue;
    const buf = buffers[ch];
    if (buf.length === 0) continue;

    ctx.strokeStyle = props.lineColors[ch] || "#ffffff";
    ctx.beginPath();

    let firstPoint = true;
    for (let i = startIdx; i < startIdx + visibleLen && i < buf.length; i++) {
      const x = margin.left + ((i - startIdx) / totalDrawn) * plotW;
      const y = margin.top + plotH - ((buf[i] - yMin) / (yMax - yMin)) * plotH;

      if (firstPoint) {
        ctx.moveTo(x, y);
        firstPoint = false;
      } else {
        ctx.lineTo(x, y);
      }
    }
    ctx.stroke();
  }

  ctx.restore();

  // 通道图例
  ctx.textAlign = "left";
  for (let ch = 0; ch < Math.min(buffers.length, 4); ch++) {
    const x = margin.left + 4 + ch * 80;
    const y = margin.top - 2;
    ctx.fillStyle = props.channelVisible[ch] ? props.lineColors[ch] : "rgba(160, 160, 176, 0.3)";
    ctx.fillRect(x, y, 10, 2);
    ctx.fillStyle = props.channelVisible[ch] ? textColor : "rgba(160, 160, 176, 0.3)";
    ctx.font = `${fontSize - 1}px sans-serif`;
    ctx.fillText(props.channelVisible[ch] ? `CH${ch}` : `CH${ch}(隐藏)`, x + 13, y + 3);
  }
}

/** 调整 canvas 尺寸 */
function resize() {
  const canvas = canvasRef.value;
  const offscreen = offscreenRef.value;
  const container = containerRef.value;
  if (!canvas || !offscreen || !container) return;

  const dpr = window.devicePixelRatio || 1;
  const rect = container.getBoundingClientRect();
  const w = rect.width;
  const h = rect.height;

  canvas.width = w * dpr;
  canvas.height = h * dpr;
  canvas.style.width = w + "px";
  canvas.style.height = h + "px";

  offscreen.width = w * dpr;
  offscreen.height = h * dpr;

  // 确保离屏 canvas 也使用 devicePixelRatio
  const ctx = canvas.getContext("2d");
  if (ctx) {
    ctx.imageSmoothingEnabled = false;
    ctx.scale(dpr, dpr);
  }
}

const chart = useChart({
  maxPoints: props.maxPoints,
  channels: props.channelCount,
  onRender: render,
});

onMounted(() => {
  offscreenRef.value = document.createElement("canvas");
  const canvas = canvasRef.value;
  if (!canvas) return;

  resize();
  chart.renderLoop(canvas, offscreenRef.value);
});

onUnmounted(() => {
  chart.stop();
});

// 暴露给父组件的方法
defineExpose({
  pushData: chart.pushData,
  clear: chart.clear,
  setChannelCount: chart.setChannelCount,
  resetZoom,
});

// 监听 ResizeObserver
let resizeObserver: ResizeObserver | null = null;
onMounted(() => {
  if (containerRef.value) {
    resizeObserver = new ResizeObserver(() => {
      resize();
      chart.markDirty();
    });
    resizeObserver.observe(containerRef.value);

    // 鼠标滚轮缩放
    containerRef.value.addEventListener("wheel", onWheel, { passive: false });
    // 双击重置缩放
    containerRef.value.addEventListener("dblclick", resetZoom);
  }
});
onUnmounted(() => {
  resizeObserver?.disconnect();
  containerRef.value?.removeEventListener("wheel", onWheel);
  containerRef.value?.removeEventListener("dblclick", resetZoom);
});
</script>

<template>
  <div ref="containerRef" class="chart-container">
    <canvas ref="canvasRef"></canvas>
    <div class="zoom-hint">滚轮X轴缩放 · Ctrl+滚轮Y轴缩放 · 双击重置</div>
  </div>
</template>

<style scoped>
.chart-container {
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: relative;
}

canvas {
  display: block;
  width: 100%;
  height: 100%;
}

.zoom-hint {
  position: absolute;
  bottom: 4px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 10px;
  color: rgba(160, 160, 176, 0.4);
  pointer-events: none;
  white-space: nowrap;
}
</style>
