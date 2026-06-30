<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, shallowRef } from "vue";
import { useChart } from "../composables/useChart";

const props = withDefaults(
  defineProps<{
    channelCount?: number;
    maxPoints?: number;
    lineColors?: string[];
  }>(),
  {
    channelCount: 4,
    maxPoints: 4000,
    lineColors: () => ["#00d4ff", "#ff6b6b", "#ffd93d", "#6bcb77"],
  }
);

const canvasRef = ref<HTMLCanvasElement | null>(null);
const offscreenRef = shallowRef<HTMLCanvasElement | null>(null);
const containerRef = ref<HTMLDivElement | null>(null);

// 网格缓存
let lastYMin = 0;
let lastYMax = 0;
let lastWidth = 0;

const gridColor = "rgba(42, 42, 74, 0.5)";
const textColor = "#a0a0b0";
const fontSize = 10;

/** 离屏渲染回调 */
function render(buffers: Float64Array[], width: number, height: number) {
  const offscreen = offscreenRef.value;
  if (!offscreen) return;
  const ctx = offscreen.getContext("2d");
  if (!ctx) return;

  const margin = { top: 8, right: 12, bottom: 20, left: 50 };
  const plotW = width - margin.left - margin.right;
  const plotH = height - margin.top - margin.bottom;

  // 计算 Y 轴范围
  let yMin = Infinity;
  let yMax = -Infinity;
  for (const buf of buffers) {
    for (let i = 0; i < buf.length; i++) {
      const v = buf[i];
      if (v < yMin) yMin = v;
      if (v > yMax) yMax = v;
    }
  }
  if (yMin === Infinity) { yMin = 0; yMax = 4096; }
  // 留 10% 边距
  const range = yMax - yMin || 1;
  yMin = yMin - range * 0.05;
  yMax = yMax + range * 0.05;

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

      // 网格线
      ctx.beginPath();
      ctx.moveTo(margin.left, y);
      ctx.lineTo(margin.left + plotW, y);
      ctx.stroke();

      // Y 轴标签
      const label = val >= 1000 ? (val / 1000).toFixed(1) + "k" : val.toFixed(0);
      ctx.fillText(label, margin.left - 4, y + 3);
    }

    // X 轴标签
    ctx.textAlign = "center";
    const totalPoints = buffers[0]?.length || 0;
    const xLabels = 4;
    for (let i = 0; i <= xLabels; i++) {
      const x = margin.left + (plotW / xLabels) * i;
      const idx = Math.round((totalPoints / xLabels) * i);
      ctx.fillText(String(idx), x, height - 2);
    }
  }

  // 设置裁剪区域
  ctx.save();
  ctx.beginPath();
  ctx.rect(margin.left, margin.top, plotW, plotH);
  ctx.clip();

  // 绘制折线
  const totalPoints = buffers[0]?.length || 0;
  ctx.lineWidth = 1.2;

  for (let ch = 0; ch < buffers.length; ch++) {
    const buf = buffers[ch];
    if (buf.length === 0) continue;

    ctx.strokeStyle = props.lineColors[ch] || "#ffffff";
    ctx.beginPath();

    let firstPoint = true;
    for (let i = 0; i < buf.length; i++) {
      const x = margin.left + (i / (totalPoints - 1 || 1)) * plotW;
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
    ctx.fillStyle = props.lineColors[ch];
    ctx.fillRect(x, y, 10, 2);
    ctx.fillStyle = textColor;
    ctx.font = `${fontSize - 1}px sans-serif`;
    ctx.fillText(`CH${ch}`, x + 13, y + 3);
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
  }
});
onUnmounted(() => {
  resizeObserver?.disconnect();
});
</script>

<template>
  <div ref="containerRef" class="chart-container">
    <canvas ref="canvasRef"></canvas>
  </div>
</template>

<style scoped>
.chart-container {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

canvas {
  display: block;
  width: 100%;
  height: 100%;
}
</style>
