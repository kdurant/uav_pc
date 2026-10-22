<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
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
    lineColors: () => ["#2f6fed", "#ef4444", "#f59e0b", "#10b981"],
    channelVisible: () => [true, true, true, true],
  }
);

const emit = defineEmits<{
  (e: "update:channelVisible", value: boolean[]): void;
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
const containerRef = ref<HTMLDivElement | null>(null);
// 是否有数据（无数据时显示占位提示）
const chartHasData = ref(false);
// 当前画布尺寸（CSS 像素），绘制时统一使用 CSS 像素 + dpr 变换
const viewSize = { width: 0, height: 0 };

// 缩放状态
const zoomFactor = ref(1.0);
const visiblePoints = ref(props.maxPoints);

// 手动坐标范围
const yMinManual = ref(0);
const yMaxManual = ref(4096);
const xStartManual = ref(0);
const xEndManual = ref(3999);
const useManualRangeY = ref(false);
const useManualRangeX = ref(false);

// 显示选项
const showDots = ref(false);
const showGrid = ref(true);
const autoRefresh = ref(true);

// 网格缓存（始终强制重绘以响应 showGrid 切换）
let lastYMin = NaN;
let lastYMax = NaN;
let lastWidth = 0;

const gridColor = "rgba(16, 24, 40, 0.055)";
const textColor = "#8b97aa";
const axisColor = "rgba(16, 24, 40, 0.14)";
const canvasBg = "#ffffff";
const hiddenColor = "rgba(147, 160, 180, 0.55)";
const fontSize = 11;

/** 图表绘图区边距（顶部留出图例空间） */
const chartMargin = { top: 30, right: 18, bottom: 26, left: 56 };

/** 十六进制颜色 → rgba（用于图例胶囊底色） */
function withAlpha(color: string, alpha: number): string {
  const hex = color.replace("#", "");
  if (!/^[0-9a-f]{3}$|^[0-9a-f]{6}$/i.test(hex)) return color;
  const full = hex.length === 3 ? hex.split("").map((c) => c + c).join("") : hex;
  const n = parseInt(full, 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}

// 鼠标悬停坐标（canvas 像素坐标，-1 表示不在图表区域）
let hoverX = -1;
let hoverY = -1;
// 最近一次 render 的绘图区域参数（供 overlay 使用）
let lastRenderState = {
  margin: chartMargin,
  plotW: 0, plotH: 0,
  startIdx: 0, visibleLen: 0,
  yMin: 0, yMax: 0,
  xOffset: 0,
  buffers: [] as Float64Array[],
};


// 位置对齐模式：显示整帧数据，缓冲区索引 = 绝对采样位置（从 0 开始）
const frameMode = ref(false);
const frameStart = ref(0);
// X 轴默认右端 = 第二段终点 * 1.2
const frameAxisEnd = ref(0);
const DEFAULT_X_SCALE = 1.2;

/** 用一整帧预览数据刷新图表（按绝对采样位置对齐，NaN 为空档） */
function setFrame(series: number[][]) {
  frameMode.value = true;
  frameStart.value = 0;
  const len = series.reduce((m, s) => Math.max(m, s.length), 0);
  frameAxisEnd.value = Math.max(1, Math.round(len * DEFAULT_X_SCALE));
  chart.setFrame(series, 0);
}

// 框选缩放状态
let isDragging = false;
let dragStartX = -1;
let dragStartY = -1;
let dragCurrentX = -1;
let dragCurrentY = -1;

function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return; // 仅左键
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;
  const { margin, plotW, plotH } = lastRenderState;
  // 检查是否在绘图区域内
  const plotRight = margin.left + plotW;
  const plotBottom = margin.top + plotH;
  if (x >= margin.left && x <= plotRight && y >= margin.top && y <= plotBottom) {
    isDragging = true;
    dragStartX = x;
    dragStartY = y;
    dragCurrentX = x;
    dragCurrentY = y;
  }
}

function onMouseMove(e: MouseEvent) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  hoverX = e.clientX - rect.left;
  hoverY = e.clientY - rect.top;
  if (isDragging) {
    dragCurrentX = hoverX;
    dragCurrentY = hoverY;
  }
  chart.markOverlayDirty();
}

function onMouseUp(e: MouseEvent) {
  if (!isDragging) return;
  isDragging = false;
  const canvas = canvasRef.value;
  if (!canvas) return;

  const x0 = dragStartX;
  const y0 = dragStartY;
  const x1 = dragCurrentX;
  const y1 = dragCurrentY;

  // 最小框选距离
  const minBox = 8;
  if (Math.abs(x1 - x0) < minBox && Math.abs(y1 - y0) < minBox) {
    dragStartX = -1;
    chart.markDirty();
    return;
  }

  const { margin, plotW, plotH, startIdx, visibleLen, yMin, yMax, xOffset } = lastRenderState;

  // 转换像素坐标到数据坐标（X 为采样点位置）
  const toDataX = (px: number) => xOffset + startIdx + ((px - margin.left) / plotW) * (visibleLen - 1);
  const toDataY = (py: number) => yMax - ((py - margin.top) / plotH) * (yMax - yMin);

  const boxLeft = Math.min(x0, x1);
  const boxRight = Math.max(x0, x1);
  const boxTop = Math.min(y0, y1);
  const boxBottom = Math.max(y0, y1);

  // 应用框选的 X 范围
  let newXStart = toDataX(boxLeft);
  let newXEnd = toDataX(boxRight);
  newXStart = Math.max(0, Math.round(newXStart));
  newXEnd = Math.round(newXEnd);
  if (newXEnd <= newXStart) newXEnd = newXStart + 1;

  // 应用框选的 Y 范围（仅当框选高度足够大时才调整 Y 轴）
  const boxH = Math.abs(boxBottom - boxTop);
  if (boxH > 20) {
    const newYMax = toDataY(boxTop);
    const newYMin = toDataY(boxBottom);
    yMinManual.value = Math.min(newYMin, newYMax);
    yMaxManual.value = Math.max(newYMin, newYMax);
    useManualRangeY.value = true;
  }

  xStartManual.value = newXStart;
  xEndManual.value = newXEnd;
  useManualRangeX.value = true;

  dragStartX = -1;
  chart.markDirty();
}

function onMouseLeave() {
  hoverX = -1;
  hoverY = -1;
  if (isDragging) {
    isDragging = false;
    dragStartX = -1;
  }
  chart.markOverlayDirty();
}

/** 应用手动范围 */
function applyRange() {
  useManualRangeX.value = true;
  useManualRangeY.value = true;
  chart.markDirty();
}
function resetAutoRange() {
  useManualRangeY.value = false;
  useManualRangeX.value = false;
  chart.markDirty();
}
function onRefreshToggle() {
  chart.setPaused(!autoRefresh.value);
  if (autoRefresh.value) chart.markDirty();
}

/** 鼠标滚轮缩放 */
function onWheel(e: WheelEvent) {
  e.preventDefault();
  if (e.ctrlKey || e.metaKey) {
    const delta = e.deltaY > 0 ? 1.15 : 1 / 1.15;
    zoomFactor.value = Math.max(0.1, Math.min(50, zoomFactor.value * delta));
  } else if (frameMode.value) {
    // 位置对齐模式：滚轮按采样点位置缩放 X（写入手动范围）
    const { startIdx, visibleLen, xOffset } = lastRenderState;
    if (visibleLen > 1) {
      const center = xOffset + startIdx + (visibleLen - 1) / 2;
      const factor = e.deltaY > 0 ? 1.15 : 1 / 1.15;
      const half = Math.max(1, ((visibleLen - 1) / 2) * factor);
      xStartManual.value = Math.max(0, Math.round(center - half));
      xEndManual.value = Math.min(frameAxisEnd.value, Math.round(center + half));
      useManualRangeX.value = true;
    }
  } else if (e.shiftKey) {
    const delta = e.deltaY > 0 ? 100 : -100;
    visiblePoints.value = Math.max(100, Math.min(props.maxPoints, visiblePoints.value + delta));
  } else {
    const delta = e.deltaY > 0 ? 1.1 : 1 / 1.1;
    visiblePoints.value = Math.max(100, Math.min(props.maxPoints, visiblePoints.value * delta));
  }
  chart.markDirty();
}

function resetZoom() {
  zoomFactor.value = 1.0;
  visiblePoints.value = props.maxPoints;
  useManualRangeY.value = false;
  useManualRangeX.value = false;
  chart.markDirty();
}

function toggleCh(ch: number) {
  const arr = [...props.channelVisible];
  arr[ch] = !arr[ch];
  emit("update:channelVisible", arr);
  chart.markDirty();
}

/** 渲染回调（直接绘制到可见画布） */
function render(buffers: Float64Array[]) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const width = viewSize.width;
  const height = viewSize.height;
  if (width <= 0 || height <= 0) return;

  const margin = chartMargin;
  const plotW = width - margin.left - margin.right;
  const plotH = height - margin.top - margin.bottom;

  const totalPoints = buffers[0]?.length || 0;
  // X 轴：位置对齐模式显示整帧；手动范围优先；否则自动显示最新 N 点
  const xOffset = frameMode.value ? frameStart.value : 0;
  let startIdx: number, visibleLen: number;
  if (useManualRangeX.value && totalPoints > 0) {
    // 手动范围输入的是采样点位置，减去帧起始位置换算成索引
    startIdx = Math.max(0, Math.min(totalPoints - 1, Math.round(xStartManual.value - xOffset)));
    visibleLen = Math.max(1, Math.round(xEndManual.value - xOffset) - startIdx + 1);
  } else if (frameMode.value && totalPoints > 0) {
    // 默认 X 轴范围 [0, 第二段终点 * 1.2]，超出数据的部分留空
    startIdx = 0;
    visibleLen = Math.max(1, frameAxisEnd.value + 1);
  } else {
    startIdx = Math.max(0, totalPoints - Math.round(visiblePoints.value));
    visibleLen = Math.max(1, Math.min(Math.round(visiblePoints.value), totalPoints));
  }

  // 无数据时显示占位提示（数据驱动，避免每帧刷新 DOM）
  const hasData = totalPoints > 0;
  if (hasData !== chartHasData.value) chartHasData.value = hasData;

  // 计算 Y 轴范围
  let yMin: number, yMax: number;
  if (useManualRangeY.value) {
    yMin = yMinManual.value;
    yMax = yMaxManual.value;
  } else {
    yMin = Infinity;
    yMax = -Infinity;
    for (let ch = 0; ch < buffers.length; ch++) {
      if (!props.channelVisible[ch]) continue;
      const buf = buffers[ch];
      const end = Math.min(startIdx + visibleLen, buf.length);
      for (let i = startIdx; i < end; i++) {
        const v = buf[i];
        if (!Number.isFinite(v)) continue; // 空档（无数据）
        if (v < yMin) yMin = v;
        if (v > yMax) yMax = v;
      }
    }
    if (yMin === Infinity) {
      // 无数据：使用干净的默认量程，不做留白
      yMin = 0;
      yMax = 4096;
    } else {
      // 上下留 6% 余量，避免波形紧贴边框
      const rawRange = (yMax - yMin) || 1;
      yMin -= rawRange * 0.06;
      yMax += rawRange * 0.06;
    }
    const range = (yMax - yMin) || 1;
    const center = (yMin + yMax) / 2;
    yMin = center - (range / 2) * zoomFactor.value;
    yMax = center + (range / 2) * zoomFactor.value;
  }

  lastYMin = yMin;
  lastYMax = yMax;
  lastWidth = width;

  // 清除
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = canvasBg;
  ctx.fillRect(0, 0, width, height);

  // 复位文本对齐：悬停提示会把 textBaseline 设为 "top"，不复位会导致
  // X 轴刻度以 height-2 为顶部向下绘制、跑到画布外而不可见
  ctx.textBaseline = "alphabetic";

  // 绘图区域边框
  ctx.strokeStyle = axisColor;
  ctx.lineWidth = 1;
  ctx.strokeRect(margin.left, margin.top, plotW, plotH);

  if (showGrid.value) {
    // 水平网格线
    const gridLines = 5;
    ctx.strokeStyle = gridColor;
    ctx.fillStyle = textColor;
    ctx.font = `${fontSize}px ui-monospace, "Cascadia Code", monospace`;
    ctx.textAlign = "right";
    ctx.lineWidth = 1;

    for (let i = 0; i <= gridLines; i++) {
      const y = margin.top + (plotH / gridLines) * i;
      const val = yMax - ((yMax - yMin) / gridLines) * i;

      ctx.beginPath();
      ctx.moveTo(margin.left, y);
      ctx.lineTo(margin.left + plotW, y);
      ctx.stroke();

      const label = val >= 1000 ? (val / 1000).toFixed(1) + "k" : val.toFixed(0);
      ctx.fillText(label, margin.left - 8, y + 3.5);
    }

    // 垂直网格线
    const xLabels = 4;
    ctx.textAlign = "center";
    for (let i = 0; i <= xLabels; i++) {
      const x = margin.left + (plotW / xLabels) * i;
      ctx.beginPath();
      ctx.moveTo(x, margin.top);
      ctx.lineTo(x, margin.top + plotH);
      ctx.stroke();

      const idx = Math.round(startIdx + ((visibleLen - 1) / xLabels) * i);
      if (totalPoints > 0) ctx.fillText(String(idx + xOffset), x, height - 8);
    }
  } else {
    // 仅 X 轴标签（无网格线）
    ctx.fillStyle = textColor;
    ctx.font = `${fontSize}px ui-monospace, "Cascadia Code", monospace`;
    ctx.textAlign = "center";
    const xLabels = 4;
    for (let i = 0; i <= xLabels; i++) {
      const x = margin.left + (plotW / xLabels) * i;
      const idx = Math.round(startIdx + ((visibleLen - 1) / xLabels) * i);
      if (totalPoints > 0) ctx.fillText(String(idx + xOffset), x, height - 8);
    }
  }

  // 裁剪
  ctx.save();
  ctx.beginPath();
  ctx.rect(margin.left, margin.top, plotW, plotH);
  ctx.clip();

  const totalDrawn = Math.max(1, visibleLen - 1);
  const dotRadius = 2.5;

  for (let ch = 0; ch < buffers.length; ch++) {
    if (!props.channelVisible[ch]) continue;
    const buf = buffers[ch];
    if (buf.length === 0) continue;

    const color = props.lineColors[ch] || "#ffffff";
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.lineWidth = 1.4;
    ctx.lineJoin = "round";
    ctx.lineCap = "round";

    ctx.beginPath();

    let firstPoint = true;
    for (let i = startIdx; i < startIdx + visibleLen && i < buf.length; i++) {
      const v = buf[i];
      if (!Number.isFinite(v)) {
        firstPoint = true; // 空档：断开折线，下一段重新起笔
        continue;
      }
      const x = margin.left + ((i - startIdx) / totalDrawn) * plotW;
      const y = margin.top + plotH - ((v - yMin) / (yMax - yMin)) * plotH;

      if (showDots.value) {
        ctx.fillRect(x - dotRadius, y - dotRadius, dotRadius * 2, dotRadius * 2);
      }

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

  // 通道图例（胶囊样式，位于绘图区上方的留白内）
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  const legendY = 15;
  let legendX = margin.left;
  for (let ch = 0; ch < Math.min(buffers.length, 4); ch++) {
    const visible = props.channelVisible[ch];
    const color = visible ? props.lineColors[ch] || "#2f6fed" : hiddenColor;
    const label = `CH${ch}`;
    ctx.font = `600 ${fontSize}px "Inter", "PingFang SC", sans-serif`;
    const boxW = ctx.measureText(label).width + 30;
    const boxH = 21;

    // 胶囊底色
    ctx.fillStyle = visible ? withAlpha(props.lineColors[ch] || "#2f6fed", 0.09) : "rgba(147, 160, 180, 0.07)";
    ctx.beginPath();
    ctx.roundRect(legendX, legendY - boxH / 2, boxW, boxH, 6);
    ctx.fill();

    // 色块
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.roundRect(legendX + 8, legendY - 1.5, 11, 3, 1.5);
    ctx.fill();

    // 文字
    ctx.fillStyle = visible ? "#4a5568" : "#a3adbd";
    ctx.fillText(label, legendX + 23, legendY + 0.5);

    legendX += boxW + 6;
  }
  ctx.textBaseline = "alphabetic";

  // 保存渲染参数供 overlay 使用
  lastRenderState = { margin, plotW, plotH, startIdx, visibleLen, yMin, yMax, xOffset, buffers };

  // ---- 鼠标悬停十字光标 + 坐标提示 ----
  // 画布绘制坐标统一为 CSS 像素（resize 时已用 transform 处理 dpr）
  const mx = hoverX;
  const my = hoverY;

  // 框选矩形（在光标之前绘制，避免遮挡）
  if (isDragging && dragStartX >= 0 && dragCurrentX >= 0) {
    const sx = Math.min(dragStartX, dragCurrentX);
    const sy = Math.min(dragStartY, dragCurrentY);
    const sw = Math.abs(dragCurrentX - dragStartX);
    const sh = Math.abs(dragCurrentY - dragStartY);

    ctx.fillStyle = "rgba(37, 99, 235, 0.10)";
    ctx.fillRect(sx, sy, sw, sh);
    ctx.strokeStyle = "rgba(37, 99, 235, 0.55)";
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 2]);
    ctx.strokeRect(sx, sy, sw, sh);
    ctx.setLineDash([]);
  }

  // 判断鼠标是否在绘图区域内
  if (
    mx >= margin.left && mx <= margin.left + plotW &&
    my >= margin.top  && my <= margin.top + plotH
  ) {
    // 根据鼠标 X 位置反推最近的数据索引
    const fraction = (mx - margin.left) / plotW;
    const nearIdx = Math.round(startIdx + fraction * (visibleLen - 1));

    // 十字光标
    ctx.save();
    ctx.setLineDash([4, 3]);
    ctx.strokeStyle = "rgba(37, 99, 235, 0.32)";
    ctx.lineWidth = 1;
    // 竖线
    ctx.beginPath();
    ctx.moveTo(mx, margin.top);
    ctx.lineTo(mx, margin.top + plotH);
    ctx.stroke();
    // 横线
    ctx.beginPath();
    ctx.moveTo(margin.left, my);
    ctx.lineTo(margin.left + plotW, my);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.restore();

    // 在十字交叉点处标记最近的数据点（每个可见通道一个圆点）
    const tipLines: string[] = [];
    const tipColors: string[] = [];
    tipLines.push(`X = ${nearIdx + xOffset}`);
    tipColors.push(textColor);

    for (let ch = 0; ch < Math.min(buffers.length, 4); ch++) {
      if (!props.channelVisible[ch]) continue;
      const buf = buffers[ch];
      if (nearIdx < 0 || nearIdx >= buf.length) continue;

      const val = buf[nearIdx];
      if (!Number.isFinite(val)) continue; // 空档：无数据
      const px = margin.left + ((nearIdx - startIdx) / Math.max(1, visibleLen - 1)) * plotW;
      const py = margin.top + plotH - ((val - yMin) / (yMax - yMin || 1)) * plotH;

      // 实心圆点
      const dotR = 3.5;
      const color = props.lineColors[ch] || "#333333";
      ctx.beginPath();
      ctx.arc(px, py, dotR, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();
      ctx.strokeStyle = canvasBg;
      ctx.lineWidth = 1;
      ctx.stroke();

      tipLines.push(`CH${ch} = ${val.toFixed(1)}`);
      tipColors.push(color);
    }

    // 绘制坐标提示框
    if (tipLines.length > 0) {
      ctx.font = `${fontSize + 1}px ui-monospace, "Cascadia Code", monospace`;
      const padding = 8;
      const lineH = fontSize + 6;
      const boxW = Math.max(...tipLines.map(l => ctx.measureText(l).width)) + padding * 2;
      const boxH = tipLines.length * lineH + padding * 2 - 4;

      // 计算位置（避免超出图表边界）
      let bx = mx + 14;
      let by = my - boxH / 2;
      if (bx + boxW > margin.left + plotW) bx = mx - boxW - 14;
      if (by < margin.top) by = margin.top;
      if (by + boxH > margin.top + plotH) by = margin.top + plotH - boxH;

      // 背景 + 阴影
      ctx.save();
      ctx.shadowColor = "rgba(16, 24, 40, 0.14)";
      ctx.shadowBlur = 14;
      ctx.shadowOffsetY = 4;
      ctx.fillStyle = "rgba(255, 255, 255, 0.98)";
      ctx.beginPath();
      ctx.roundRect(bx, by, boxW, boxH, 7);
      ctx.fill();
      ctx.restore();

      ctx.strokeStyle = "rgba(16, 24, 40, 0.1)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.roundRect(bx, by, boxW, boxH, 7);
      ctx.stroke();

      // 文字
      ctx.textAlign = "left";
      ctx.textBaseline = "top";
      for (let i = 0; i < tipLines.length; i++) {
        ctx.fillStyle = tipColors[i] || textColor;
        ctx.fillText(tipLines[i], bx + padding, by + padding + i * lineH);
      }
    }
  }
}

function resize() {
  const canvas = canvasRef.value;
  if (!canvas) return;

  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  const w = rect.width;
  const h = rect.height;
  if (w === 0 || h === 0) return;

  viewSize.width = w;
  viewSize.height = h;
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(h * dpr);

  const ctx = canvas.getContext("2d");
  if (ctx) {
    // 统一用 CSS 像素绘制，由 transform 负责 HiDPI 缩放
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
}

const chart = useChart({
  maxPoints: props.maxPoints,
  channels: props.channelCount,
  onRender: render,
});

onMounted(() => {
  const canvas = canvasRef.value;
  if (!canvas) return;
  resize();
  chart.renderLoop();
});

onUnmounted(() => chart.stop());

defineExpose({
  pushData: chart.pushData,
  setFrame,
  clear: chart.clear,
  setChannelCount: chart.setChannelCount,
  resetZoom,
});

let resizeObserver: ResizeObserver | null = null;
onMounted(() => {
  const canvas = canvasRef.value;
  const container = containerRef.value;
  if (canvas) {
    resizeObserver = new ResizeObserver(() => { resize(); chart.markDirty(); });
    resizeObserver.observe(canvas);
  }
  if (container) {
    container.addEventListener("wheel", onWheel, { passive: false });
    container.addEventListener("dblclick", resetZoom);
  }
  if (canvas) {
    canvas.addEventListener("mousemove", onMouseMove);
    canvas.addEventListener("mousedown", onMouseDown);
    canvas.addEventListener("mouseup", onMouseUp);
    canvas.addEventListener("mouseleave", onMouseLeave);
  }
});
onUnmounted(() => {
  resizeObserver?.disconnect();
  containerRef.value?.removeEventListener("wheel", onWheel);
  containerRef.value?.removeEventListener("dblclick", resetZoom);
  canvasRef.value?.removeEventListener("mousemove", onMouseMove);
  canvasRef.value?.removeEventListener("mousedown", onMouseDown);
  canvasRef.value?.removeEventListener("mouseup", onMouseUp);
  canvasRef.value?.removeEventListener("mouseleave", onMouseLeave);
});
</script>

<template>
  <div ref="containerRef" class="chart-container">
    <div class="canvas-wrap">
      <canvas ref="canvasRef"></canvas>
      <div v-if="!chartHasData" class="chart-empty">
        <div class="chart-empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 17l4.5-5.5 3.5 3L15 8l6 6" />
            <path d="M3 4v16h18" />
          </svg>
        </div>
        <p class="chart-empty-title">等待采集数据</p>
        <p class="chart-empty-desc">连接设备并开始采集后，实时波形将在此显示</p>
      </div>
    </div>
    <div class="channel-bar">
      <!-- 通道可见性 -->
      <div class="bar-group">
        <label
          v-for="ch in props.channelCount"
          :key="ch"
          class="ch-chip"
          :class="{ off: !props.channelVisible[ch - 1] }"
          :style="{ '--ch-color': props.lineColors[ch - 1] }"
        >
          <input type="checkbox" :checked="props.channelVisible[ch - 1]" @change="toggleCh(ch - 1)" />
          <span class="ch-swatch"></span>
          <span>CH{{ ch - 1 }}</span>
        </label>
      </div>

      <span class="bar-divider"></span>

      <!-- 坐标范围 X -->
      <div class="bar-group">
        <span class="bar-label">X</span>
        <input type="number" v-model.number="xStartManual" class="range-inp" title="X 起始采样位置" />
        <span class="bar-tilde">~</span>
        <input type="number" v-model.number="xEndManual" class="range-inp" title="X 结束采样位置" />
      </div>

      <span class="bar-divider"></span>

      <!-- 坐标范围 Y -->
      <div class="bar-group">
        <span class="bar-label">Y</span>
        <input type="number" v-model.number="yMinManual" class="range-inp" title="Y 最小值" />
        <span class="bar-tilde">~</span>
        <input type="number" v-model.number="yMaxManual" class="range-inp" title="Y 最大值" />
        <button class="bar-btn" @click="applyRange">设定</button>
        <button class="bar-btn" @click="resetAutoRange">自动</button>
      </div>

      <span class="bar-divider"></span>

      <!-- 显示选项 -->
      <div class="bar-group">
        <label class="ch-chip plain">
          <input type="checkbox" v-model="showGrid" @change="chart.markDirty()" />
          <span>栅格</span>
        </label>
        <label class="ch-chip plain">
          <input type="checkbox" v-model="showDots" @change="chart.markDirty()" />
          <span>圆点</span>
        </label>
        <label class="ch-chip plain" :class="{ paused: !autoRefresh }">
          <input type="checkbox" v-model="autoRefresh" @change="onRefreshToggle" />
          <span>{{ autoRefresh ? '实时' : '暂停' }}</span>
        </label>
      </div>

      <span class="zoom-hint">滚轮缩放 X · Ctrl+滚轮缩放 Y · 框选放大 · 双击重置</span>
    </div>
  </div>
</template>

<style scoped>
.chart-container {
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
  background: #fff;
}

.canvas-wrap {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
}

canvas {
  flex: 1;
  display: block;
  min-height: 0;
  min-width: 0;
  cursor: crosshair;
}

.chart-empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  pointer-events: none;
  text-align: center;
  padding: 20px;
}

.chart-empty-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 46px;
  margin-bottom: 6px;
  border-radius: 13px;
  background: var(--bg-tertiary);
  color: var(--text-tertiary);
}

.chart-empty-icon svg {
  width: 24px;
  height: 24px;
}

.chart-empty-title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text-secondary);
}

.chart-empty-desc {
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.zoom-hint {
  margin-left: auto;
  font-size: 10.5px;
  color: var(--text-tertiary);
  white-space: nowrap;
  padding: 0 4px;
}

.channel-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 12px;
  background: #fbfcfe;
  border-top: 1px solid var(--border-color);
  flex-shrink: 0;
  flex-wrap: wrap;
  row-gap: 6px;
}

.bar-group {
  display: flex;
  align-items: center;
  gap: 5px;
}

.bar-divider {
  width: 1px;
  height: 16px;
  background: var(--border-color);
  flex-shrink: 0;
}

.bar-label {
  font-size: 11px;
  font-weight: 650;
  color: var(--text-tertiary);
  letter-spacing: 0.06em;
}

.bar-tilde {
  color: var(--text-tertiary);
  font-size: 11px;
}

.range-inp {
  width: 62px;
  height: 26px;
  padding: 0 7px;
  font-size: 11.5px;
  border-radius: var(--radius-xs);
  background: #fff;
}

.range-inp:focus {
  box-shadow: 0 0 0 2px var(--accent-glow);
}

.bar-btn {
  height: 26px;
  padding: 0 9px;
  font-size: 11.5px;
  border-radius: var(--radius-xs);
  box-shadow: none;
  color: var(--text-secondary);
}

.bar-btn:hover {
  color: var(--accent-dim);
}

/* 通道/选项胶囊 */
.ch-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 8px;
  border-radius: var(--radius-xs);
  border: 1px solid transparent;
  cursor: pointer;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-secondary);
  background: transparent;
  transition: background 0.16s ease, border-color 0.16s ease, opacity 0.16s ease;
  user-select: none;
}

.ch-chip:hover {
  background: #fff;
  border-color: var(--border-color);
}

.ch-chip input[type="checkbox"] {
  width: 13px;
  height: 13px;
}

.ch-swatch {
  width: 8px;
  height: 8px;
  border-radius: 2px;
  background: var(--ch-color, var(--accent));
  flex-shrink: 0;
}

.ch-chip.plain {
  font-weight: 500;
  color: var(--text-secondary);
}

.ch-chip.off {
  opacity: 0.55;
}

.ch-chip.off .ch-swatch {
  background: var(--text-tertiary);
}

.ch-chip.paused {
  color: var(--warning);
  background: var(--warning-soft);
  border-color: #f2dec2;
}

/* 窄窗口：压缩工具条，隐藏操作提示 */
@media (max-width: 1200px) {
  .channel-bar {
    gap: 6px;
    padding: 6px 8px;
  }

  .zoom-hint {
    display: none;
  }

  .range-inp {
    width: 52px;
    padding: 0 6px;
  }

  .bar-btn {
    padding: 0 7px;
  }

  .ch-chip {
    gap: 4px;
    padding: 0 6px;
  }
}
</style>
