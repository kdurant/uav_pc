<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useTriggerHelp } from "../composables/useTriggerHelp";

const props = defineProps<{
  /** 设备当前模式：0 内触发 / 1 外触发 / 2 PIN 触发 */
  deviceMode?: number;
  /** 和阈值（PIN 触发用） */
  sumLevel?: number;
  /** 值阈值（PIN 触发用） */
  minLevel?: number;
}>();

const { visible, activeMode, close } = useTriggerHelp();

const TABS = [
  { mode: 0, label: "内触发", badge: "卡内" },
  { mode: 1, label: "外触发", badge: "卡外" },
  { mode: 2, label: "PIN 触发", badge: "数据流" },
];

const MODE_LABELS = ["内触发", "外触发", "PIN 触发"];

/** 当前查看的 tab，打开时同步为设备实际模式 */
const active = ref(0);

watch([visible, activeMode], ([shown, mode]) => {
  if (shown) active.value = mode;
});

const NOTES: string[][] = [
  [
    "触发信号由采集卡内部产生，不依赖任何外部设备。",
    "脉冲从 TRG 引脚输出后，经卡外短接线环回 TRG 输入，形成自触发回路。",
    "采集节拍完全由采集卡自己决定，适合整机自检或独立采集。",
  ],
  [
    "触发信号来自卡外的激光器同步输出，接到采集卡 TRG 输入。",
    "每个激光脉冲对应一次采集，采集时刻与出光严格对齐。",
    "需要激光器提供同步输出，并确认电平、极性匹配。",
  ],
  [
    "触发源是卡内输入数据流本身：逐点与阈值比较，超过阈值即产生触发。",
    "阈值取自设备状态中的值阈值 / 和阈值，需按回波强度现场调整。",
    "阈值过低会被噪声误触发，过高则会漏掉弱回波。",
  ],
];

const notes = computed(() => NOTES[active.value] ?? NOTES[0]);

const deviceModeLabel = computed(() => MODE_LABELS[props.deviceMode ?? -1] ?? "未知");

/** PIN 图阈值标注，未连接设备时显示占位 */
const thresholdText = computed(() => {
  const min = props.minLevel;
  const sum = props.sumLevel;
  if (min === undefined && sum === undefined) return "阈值：—";
  return `阈值：值 ${min ?? "—"} · 和 ${sum ?? "—"}`;
});

// ===== 示意图路径 =====
/** 内触发：卡内触发源 → TRG OUT → 卡外短接线 → TRG IN → 采集 */
const INTERNAL = {
  out: "M330 105 L285 105",
  loop: "M285 105 L243 105 L243 155 L285 155",
  back: "M285 155 L630 155 L630 235",
  motion:
    "M330 105 L285 105 L243 105 L243 155 L285 155 L630 155 L630 235",
};

/** 外触发：激光器 → TRG IN → 触发整形 → 采集 */
const EXTERNAL_MOTION =
  "M220 165 L285 165 L330 165 L500 165 L630 165 L630 235";
/** PIN：阈值比较输出 → 采集 */
const PIN_MOTION = "M622 170 L622 235";

// ===== PIN 波形（两周期滚动，周期 210px） =====
const WAVE_W = 440;
const WAVE_VIEW_W = 215;
const WAVE_BASE = 175;
const WAVE_THRESHOLD = 128;
const WAVE_PERIOD = 210;

/** 波形纵坐标：低频起伏 + 每周期一个高斯峰（峰值超过阈值线） */
function waveY(x: number): number {
  const base = WAVE_BASE - 8 * Math.sin((2 * Math.PI * x) / 105);
  const gauss = (center: number) =>
    Math.exp(-((x - center) ** 2) / (2 * 18 * 18));
  return base - 62 * (gauss(140) + gauss(140 + WAVE_PERIOD));
}

const wavePath = computed(() => {
  const points: string[] = [];
  for (let x = 0; x <= WAVE_W; x += 3) {
    points.push(`${x} ${waveY(x).toFixed(1)}`);
  }
  return `M${points.join(" L")}`;
});

// ===== Esc 关闭 =====
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && visible.value) close();
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-overlay" @click.self="close()">
      <div class="modal" role="dialog" aria-modal="true" aria-label="触发方式说明">
        <header class="modal-head">
          <div class="modal-titles">
            <h3>触发方式说明</h3>
            <p>三种采集模式的触发信号来源与时序差异</p>
          </div>
          <button class="close-btn" aria-label="关闭" @click="close()">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
              stroke-linecap="round">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </header>

        <div class="tabs" role="tablist">
          <button
            v-for="tab in TABS"
            :key="tab.mode"
            type="button"
            role="tab"
            class="tab"
            :class="{ active: active === tab.mode }"
            :aria-selected="active === tab.mode"
            @click="active = tab.mode"
          >
            <span class="tab-label">{{ tab.label }}</span>
            <span class="tab-badge">{{ tab.badge }}</span>
          </button>
        </div>

        <div class="diagram">
          <svg viewBox="0 0 760 360" role="img" :aria-label="`${MODE_LABELS[active]}示意图`">
            <defs>
              <marker
                id="trg-arrow"
                viewBox="0 0 10 10"
                refX="8"
                refY="5"
                markerWidth="6"
                markerHeight="6"
                orient="auto-start-reverse"
              >
                <path d="M0 0 L10 5 L0 10 z" fill="#2563eb" />
              </marker>
              <marker
                id="beam-arrow"
                viewBox="0 0 10 10"
                refX="8"
                refY="5"
                markerWidth="5"
                markerHeight="5"
                orient="auto-start-reverse"
              >
                <path d="M0 0 L10 5 L0 10 z" fill="#dc2626" />
              </marker>
              <clipPath id="wave-above" clipPathUnits="userSpaceOnUse">
                <rect x="0" y="0" :width="WAVE_W" :height="WAVE_THRESHOLD" />
              </clipPath>
              <clipPath id="wave-viewport" clipPathUnits="userSpaceOnUse">
                <rect x="0" y="60" :width="WAVE_VIEW_W" height="140" />
              </clipPath>
            </defs>

            <!-- 采集卡轮廓 -->
            <rect class="card-body" x="285" y="40" width="450" height="280" rx="12" />
            <text class="zone-label" x="510" y="31" text-anchor="middle">采集卡内部</text>
            <text class="zone-label" x="150" y="31" text-anchor="middle">卡外</text>

            <!-- ===== 内触发 ===== -->
            <g v-if="active === 0">
              <rect class="module" x="330" y="75" width="180" height="60" rx="8" />
              <circle class="pulse-ring" cx="350" cy="105" r="8" />
              <text class="module-title" x="372" y="101">内部触发源</text>
              <text class="module-sub" x="372" y="118">定时器 / 重复频率</text>

              <path class="signal-line" :d="INTERNAL.out" />
              <path class="signal-line" :d="INTERNAL.loop" />
              <path class="signal-line" :d="INTERNAL.back" marker-end="url(#trg-arrow)" />

              <circle class="terminal" cx="285" cy="105" r="4" />
              <circle class="terminal" cx="285" cy="155" r="4" />
              <text class="terminal-label" x="278" y="98" text-anchor="end">TRG OUT</text>
              <text class="terminal-label" x="298" y="147">TRG IN</text>
              <text class="terminal-label" x="235" y="134" text-anchor="end">短接线</text>

              <circle class="spark" r="4">
                <animateMotion :path="INTERNAL.motion" dur="2s" repeatCount="indefinite" />
              </circle>
            </g>

            <!-- ===== 外触发 ===== -->
            <g v-else-if="active === 1">
              <!-- 卡外：激光器与出光示意 -->
              <rect class="module" x="40" y="130" width="180" height="70" rx="8" />
              <circle class="laser-glow" cx="70" cy="165" r="8" />
              <text class="module-title" x="140" y="161" text-anchor="middle">激光器</text>
              <text class="module-sub" x="140" y="179" text-anchor="middle">出光 + 同步脉冲</text>

              <path class="laser-beam" d="M90 130 L90 65" marker-end="url(#beam-arrow)" />
              <text class="axis-label" x="100" y="80">激光输出</text>

              <!-- 同步信号 → TRG IN → 触发整形 → 采集 -->
              <path class="signal-line" d="M220 165 L285 165" />
              <circle class="terminal" cx="285" cy="165" r="4" />
              <path class="signal-line" d="M285 165 L330 165" marker-end="url(#trg-arrow)" />

              <rect class="module" x="330" y="130" width="170" height="70" rx="8" />
              <text class="module-title" x="415" y="161" text-anchor="middle">触发整形</text>
              <text class="module-sub" x="415" y="179" text-anchor="middle">电平 / 极性匹配</text>

              <path
                class="signal-line"
                d="M500 165 L630 165 L630 235"
                marker-end="url(#trg-arrow)"
              />

              <text class="terminal-label" x="289" y="150">TRG IN</text>

              <circle class="spark" r="4">
                <animateMotion :path="EXTERNAL_MOTION" dur="2s" repeatCount="indefinite" />
              </circle>
            </g>

            <!-- ===== PIN 触发 ===== -->
            <g v-else>
              <g transform="translate(305 0)" clip-path="url(#wave-viewport)">
                <g class="wave-scroll">
                  <path :d="wavePath" class="wave-line" />
                </g>
                <g clip-path="url(#wave-above)">
                  <g class="wave-scroll">
                    <path :d="wavePath" class="wave-over" />
                  </g>
                </g>
              </g>

              <line class="threshold-line" x1="305" y1="128" x2="540" y2="128" />
              <text class="threshold-label" x="308" y="100">{{ thresholdText }}</text>
              <text class="axis-label" x="305" y="205">输入数据流</text>

              <rect class="module" x="545" y="100" width="155" height="70" rx="8" />
              <text class="module-title" x="622" y="128" text-anchor="middle">阈值比较</text>
              <text class="module-sub" x="622" y="146" text-anchor="middle">超过阈值 → 触发</text>

              <path class="signal-line" d="M520 155 L545 155" marker-end="url(#trg-arrow)" />
              <path class="signal-line" :d="PIN_MOTION" marker-end="url(#trg-arrow)" />

              <circle class="spark" r="4">
                <animateMotion :path="PIN_MOTION" dur="1.2s" repeatCount="indefinite" />
              </circle>
            </g>

            <!-- 采集模块：三种模式共用 -->
            <rect class="module capture" x="545" y="235" width="180" height="70" rx="8" />
            <circle class="capture-led" :class="{ fast: active === 2 }" cx="572" cy="270" r="6" />
            <text class="module-title" x="642" y="266" text-anchor="middle">采集 / 存储</text>
            <text class="module-sub" x="642" y="284" text-anchor="middle">单次采集 → 波形</text>
          </svg>
        </div>

        <ul class="notes">
          <li v-for="(line, i) in notes" :key="i">{{ line }}</li>
        </ul>

        <footer class="modal-foot">
          <span class="device-mode">设备当前：{{ deviceModeLabel }}</span>
          <button @click="close()">关闭</button>
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
  padding: 20px;
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
  width: 800px;
  max-width: calc(100vw - 40px);
  max-height: 90vh;
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
  padding: 16px 18px 12px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
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
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  border-radius: 7px;
  background: transparent;
  box-shadow: none;
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.close-btn:hover {
  background: var(--bg-hover);
  border: none;
  color: var(--text-primary);
}

.close-btn:active {
  transform: none;
}

.close-btn svg {
  width: 15px;
  height: 15px;
}

/* ---------- Tabs ---------- */
.tabs {
  display: flex;
  gap: 6px;
  padding: 12px 18px 0;
  flex-shrink: 0;
}

.tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 12px;
  border: 1px solid var(--border-color);
  border-radius: 999px;
  background: #fff;
  box-shadow: none;
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 550;
}

.tab:hover {
  background: var(--bg-inset);
  border-color: var(--border-strong);
  color: var(--text-primary);
}

.tab:active {
  transform: none;
}

.tab.active {
  background: var(--accent-soft);
  border-color: #bcd0f8;
  color: var(--accent-dim);
}

.tab-badge {
  padding: 1px 6px;
  border: 1px solid var(--border-color);
  border-radius: 999px;
  background: var(--bg-inset);
  color: var(--text-tertiary);
  font-size: 10px;
  font-weight: 550;
}

.tab.active .tab-badge {
  background: #fff;
  border-color: #bcd0f8;
  color: var(--accent-dim);
}

/* ---------- 示意图 ---------- */
.diagram {
  padding: 2px 18px 0;
  overflow: auto;
  flex-shrink: 0;
}

.diagram svg {
  display: block;
  width: 100%;
  height: auto;
}

.card-body {
  fill: #f8fafc;
  stroke: #e5e9f0;
  stroke-width: 1.2;
}

.zone-label {
  fill: #93a0b4;
  font-size: 10.5px;
  letter-spacing: 0.08em;
}

.module {
  fill: #fff;
  stroke: #d8dee9;
  stroke-width: 1.2;
}

.module.capture {
  stroke: #bcd0f8;
}

.module-title {
  fill: #101828;
  font-size: 12.5px;
  font-weight: 600;
}

.module-sub {
  fill: #93a0b4;
  font-size: 10.5px;
}

.signal-line {
  fill: none;
  stroke: var(--accent);
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-dasharray: 6 5;
  animation: signal-flow 0.9s linear infinite;
}

@keyframes signal-flow {
  to {
    stroke-dashoffset: -22;
  }
}

.terminal {
  fill: #fff;
  stroke: var(--accent);
  stroke-width: 2;
}

.terminal-label {
  fill: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 10px;
}

.axis-label {
  fill: var(--text-tertiary);
  font-size: 10.5px;
  letter-spacing: 0.02em;
}

.spark {
  fill: var(--accent);
}

.capture-led {
  fill: var(--success);
  animation: led-blink 2s ease-in-out infinite;
}

.capture-led.fast {
  animation-duration: 1.2s;
}

@keyframes led-blink {
  0%,
  60%,
  100% {
    opacity: 0.22;
  }
  28% {
    opacity: 1;
  }
}

/* 内触发：源脉冲扩散 */
.pulse-ring {
  fill: none;
  stroke: var(--accent);
  stroke-width: 1.6;
  transform-box: fill-box;
  transform-origin: center;
  animation: ring-pulse 2s ease-out infinite;
}

@keyframes ring-pulse {
  0% {
    transform: scale(1);
    opacity: 0.9;
  }
  70%,
  100% {
    transform: scale(2.3);
    opacity: 0;
  }
}

/* 外触发：激光器发光 */
.laser-glow {
  fill: #dc2626;
  animation: laser-blink 2s ease-in-out infinite;
}

/* 外触发：出光示意（卡外淡红虚线） */
.laser-beam {
  fill: none;
  stroke: #dc2626;
  stroke-width: 1.4;
  stroke-dasharray: 4 4;
  opacity: 0.7;
}

@keyframes laser-blink {
  0%,
  60%,
  100% {
    opacity: 0.28;
  }
  28% {
    opacity: 1;
  }
}

/* PIN：阈值与波形 */
.threshold-line {
  stroke: var(--warning);
  stroke-width: 1.2;
  stroke-dasharray: 5 4;
}

.threshold-label {
  fill: var(--warning);
  font-family: var(--font-mono);
  font-size: 10px;
}

.wave-line {
  fill: none;
  stroke: var(--accent);
  stroke-width: 2;
  stroke-linejoin: round;
}

.wave-over {
  fill: none;
  stroke: #dc2626;
  stroke-width: 2.2;
  stroke-linejoin: round;
}

.wave-scroll {
  animation: wave-scroll 2.4s linear infinite;
}

@keyframes wave-scroll {
  from {
    transform: translateX(0);
  }
  to {
    transform: translateX(-210px);
  }
}

/* ---------- 说明文字 ---------- */
.notes {
  margin: 0;
  padding: 2px 22px 14px 34px;
  flex-shrink: 0;
}

.notes li {
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-secondary);
}

.notes li::marker {
  color: var(--text-tertiary);
}

.modal-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 18px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-inset);
  flex-shrink: 0;
}

.device-mode {
  font-size: 11.5px;
  color: var(--text-tertiary);
}

@media (prefers-reduced-motion: reduce) {
  .signal-line,
  .wave-scroll,
  .capture-led,
  .laser-glow,
  .pulse-ring {
    animation: none;
  }

  .spark {
    display: none;
  }
}
</style>
