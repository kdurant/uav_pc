<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";

const props = defineProps<{
  visible: boolean;
  /** 采样长度（点，1 点 = 1 ns） */
  waveLen: number;
  firstPos: number;
  firstLen: number;
  secondPos: number;
  secondLen: number;
  /** 和阈值 */
  sumLevel: number;
  /** 值阈值 */
  minLevel: number;
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

// ===== 全景图坐标 =====
const AXIS_X0 = 60;
const AXIS_X1 = 740;
const AXIS_Y = 64;

/** 采样窗口归一化，长度为 0 时退化为 1，避免除零 */
const wave = computed(() => Math.max(1, Math.round(props.waveLen || 0)));

/** 采样点 → 全景图横坐标（越界值会被夹到两端） */
function xOf(point: number): number {
  const clamped = Math.min(Math.max(point, 0), wave.value);
  return AXIS_X0 + (clamped / wave.value) * (AXIS_X1 - AXIS_X0);
}

const firstStart = computed(() => Math.max(0, props.firstPos || 0));
const firstEnd = computed(() => firstStart.value + Math.max(0, props.firstLen || 0));
const secondStart = computed(() => Math.max(0, props.secondPos || 0));
const secondLength = computed(() => Math.max(0, props.secondLen || 0));

/** 第二段搜索上限：留 800 点余量，窗口过小时退化为整个采样窗口 */
const SEARCH_MARGIN = 800;
const searchEnd = computed(() => {
  const limit = wave.value - SEARCH_MARGIN;
  if (limit <= secondStart.value) return wave.value;
  return Math.min(limit, wave.value);
});

/** 模拟的命中位置：搜索范围内约 1/3 处，并保证第二段放得下 */
const hitPos = computed(() => {
  const span = searchEnd.value - secondStart.value;
  if (span <= 0) return secondStart.value;
  const candidate = secondStart.value + Math.round(span * 0.32);
  const maxStart = Math.max(secondStart.value, wave.value - secondLength.value);
  return Math.min(candidate, maxStart);
});

/** 第二段实际采集区间：从命中位置开始，长度 second_len */
const collectStart = hitPos;
const collectEnd = computed(() =>
  Math.min(collectStart.value + secondLength.value, wave.value)
);

const firstX1 = computed(() => xOf(firstStart.value));
const firstX2 = computed(() => xOf(firstEnd.value));
const secondX1 = computed(() => xOf(secondStart.value));
const secondX2 = computed(() => xOf(searchEnd.value));
const collectX1 = computed(() => xOf(collectStart.value));
const collectX2 = computed(() => xOf(collectEnd.value));

/** 标注文字居中位置，限制在画布可视区内 */
function labelX(x1: number, x2: number): number {
  const mid = (x1 + x2) / 2;
  return Math.min(Math.max(mid, 59), 721);
}

/** 区间宽度太小（不足 2px）时也算不可见，改为在轴下方标注 */
const firstVisible = computed(() => props.firstLen > 0 && firstX2.value - firstX1.value >= 2);
/** 第二段是否使能：长度为 0 或搜索范围为空时都不显示 */
const secondVisible = computed(
  () =>
    secondLength.value > 0 && secondX2.value - secondX1.value >= 2
);

// ===== 参数校验 =====
const firstOverflow = computed(() => firstEnd.value > wave.value);
const secondOverflow = computed(() => collectEnd.value > wave.value);
const searchEmpty = computed(() => searchEnd.value <= secondStart.value);
const segmentsOverlap = computed(
  () => firstEnd.value > secondStart.value && secondStart.value >= firstStart.value
);

const warnings = computed(() => {
  const list: string[] = [];
  if (firstOverflow.value) list.push("第一段超出采集窗口");
  if (searchEmpty.value) list.push("第二段搜索范围为空");
  if (secondOverflow.value) list.push("第二段超出采集窗口");
  if (segmentsOverlap.value) list.push("搜索起点落在第一段区间内");
  return list;
});

/** 微秒换算 */
const micros = computed(() => (wave.value / 1000).toFixed(2));

// ===== 8 点判定（示例数据）=====
/** 相对基线的 8 点增量，强回波 */
const SAMPLE_DELTA = [120, 260, 520, 880, 640, 380, 180, 90];
/** 基线 M：命中位置处 8 点平均值 */
const BASE_M = 80;

type Strength = "strong" | "weak";
const strength = ref<Strength>("strong");
const strengthFactor = computed(() => (strength.value === "strong" ? 1 : 0.35));

/** 8 点实测值 = M + 增量 × 强度系数 */
const samples = computed(() =>
  SAMPLE_DELTA.map((d) => Math.round(BASE_M + d * strengthFactor.value))
);

const sampleSum = computed(() => samples.value.reduce((a, b) => a + b, 0));
/** 和阈值判定量：连续 8 点之和 − 8M */
const sumDelta = computed(() => sampleSum.value - 8 * BASE_M);
/** 值阈值判定量：8 点中最大值 − M */
const maxDelta = computed(() => Math.max(...samples.value) - BASE_M);

const sumPass = computed(() => sumDelta.value > (props.sumLevel || 0));
const maxPass = computed(() => maxDelta.value > (props.minLevel || 0));
/** 两个条件同时满足才采集第二段 */
const hit = computed(() => sumPass.value && maxPass.value);

/** 搜索窗口动画的横向位移：命中则停在命中位置，否则滑到搜索上限 */
const searchTravel = computed(
  () => xOf(hit.value ? collectStart.value : searchEnd.value) - secondX1.value
);

/** 全景图第二段标注文字：随阶段切换 */
const secondTag = computed(() => {
  if (phase.value >= 4) {
    return hit.value
      ? `命中 ${collectStart.value} → 采集 ${secondLength.value} 点`
      : "未搜索到满足条件的信号";
  }
  return `搜索 ${secondStart.value} → ${searchEnd.value}`;
});

/** 标注跟随搜索范围或命中后的采集区间 */
const secondTagX = computed(() =>
  phase.value >= 4 && hit.value
    ? labelX(collectX1.value, collectX2.value)
    : labelX(secondX1.value, secondX2.value)
);

// ===== 8 点柱状图坐标 =====
const BAR_BASELINE = 330;
const BAR_MAX_H = 118;
/** 柱状图纵向满量程：固定 900，阈值线超出时夹到顶部 */
const BAR_SCALE = 900;

const bars = computed(() =>
  samples.value.map((value, i) => {
    const h = Math.min((value / BAR_SCALE) * BAR_MAX_H, BAR_MAX_H);
    return {
      x: 286 + i * 24,
      y: BAR_BASELINE - h,
      h,
      value,
    };
  })
);

/** 值阈值线的纵坐标（M + min_level） */
const valueThresholdY = computed(() => {
  const y = BAR_BASELINE - ((BASE_M + (props.minLevel || 0)) / BAR_SCALE) * BAR_MAX_H;
  return Math.min(Math.max(y, 190), BAR_BASELINE);
});

/** 值阈值线是否超出绘图量程 */
const thresholdOutOfRange = computed(
  () => BASE_M + (props.minLevel || 0) > BAR_SCALE
);

/** 阈值标签纵坐标：贴近顶部时放线下方，避免压到面板副标题 */
const thresholdLabelY = computed(() => {
  const y = valueThresholdY.value;
  if (y < 200) return y + 13;
  return y < 250 ? y - 7 : y + 13;
});

/** 阈值标签背景宽度：带"超出量程"说明时加宽 */
const thresholdLabelWidth = computed(() => (thresholdOutOfRange.value ? 132 : 72));

// ===== 回波波形 =====
/** 生成一条带高斯回波的波形，返回 SVG path */
function echoPath(
  x0: number,
  x1: number,
  baseY: number,
  amp: number,
  centerRatio: number,
  widthRatio: number
): string {
  const cx = x0 + (x1 - x0) * centerRatio;
  const sigma = Math.max((x1 - x0) * widthRatio, 4);
  const steps = 56;
  const points: string[] = [];
  for (let i = 0; i <= steps; i++) {
    const x = x0 + ((x1 - x0) * i) / steps;
    const y = baseY - amp * Math.exp(-((x - cx) ** 2) / (2 * sigma * sigma));
    points.push(`${x.toFixed(1)} ${y.toFixed(1)}`);
  }
  return `M${points.join(" L")}`;
}

const firstWavePath = computed(() => echoPath(26, 234, 330, 96, 0.46, 0.1));
const secondWavePath = computed(() => echoPath(526, 734, 330, 92, 0.52, 0.1));

// ===== 播放时序 =====
/** -1 未播放 / 0 触发 / 1 第一段 / 2 八点判定 / 3 结果 / 4 第二段 */
const phase = ref(-1);
const playing = ref(false);
const cycleKey = ref(0);
const STEP_MS = 1300;
let timer: number | null = null;

function stopCycle() {
  if (timer !== null) {
    window.clearInterval(timer);
    timer = null;
  }
  playing.value = false;
}

/** 回到未播放状态（显示骨架，等待手动播放） */
function reset() {
  stopCycle();
  phase.value = -1;
}

/** 播放一次：0 → 4 后停住，不循环 */
function play() {
  if (playing.value) return;
  stopCycle();
  cycleKey.value += 1;
  phase.value = 0;
  playing.value = true;

  let step = 0;
  timer = window.setInterval(() => {
    step += 1;
    phase.value = step;
    if (step >= 4) stopCycle();
  }, STEP_MS);
}

watch(() => props.visible, reset);

onUnmounted(stopCycle);

/** 切换强度：播放中则打断回到初始状态；已播放完则保留最终帧联动刷新 */
function setStrength(value: Strength) {
  strength.value = value;
  if (playing.value) reset();
}

const STEPS = ["触发", "第一段", "8 点判定", "第二段"];

/** 步骤指示器与阶段一一对应（第二阶段在判定后才亮起） */
const activeStep = computed(() => Math.min(phase.value, 3));

function onOverlayClick(e: MouseEvent) {
  if (e.target === e.currentTarget) emit("close");
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-overlay" @click="onOverlayClick">
      <div class="modal" role="dialog" aria-modal="true" aria-label="采集参数说明">
        <header class="modal-head">
          <div class="modal-titles">
            <h3>采集参数说明</h3>
            <p>一次触发内的采样窗口、两段提取区间与阈值判定</p>
          </div>
          <button class="close-btn" aria-label="关闭" @click="emit('close')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
              stroke-linecap="round">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </header>

        <div class="steps">
          <span
            v-for="(label, i) in STEPS"
            :key="label"
            class="step"
            :class="{ active: activeStep === i, done: activeStep > i }"
          >
            <i class="step-index">{{ i + 1 }}</i>{{ label }}
          </span>

          <button
            type="button"
            class="play-btn"
            :class="{ playing }"
            :disabled="playing"
            @click="play"
          >
            <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M5 3.2l7.2 4.8-7.2 4.8z" />
            </svg>
            {{ playing ? "播放中…" : phase >= 4 ? "重播" : "播放" }}
          </button>
        </div>

        <div class="diagram">
          <svg viewBox="0 0 780 420" role="img" aria-label="采集参数示意图">
            <defs>
              <marker
                id="cap-arrow"
                viewBox="0 0 10 10"
                refX="8"
                refY="5"
                markerWidth="5"
                markerHeight="5"
                orient="auto-start-reverse"
              >
                <path d="M0 0 L10 5 L0 10 z" fill="#93a0b4" />
              </marker>
            </defs>

            <!-- ===== 全景时间轴 ===== -->
            <text class="zone-label" x="60" y="20">采样窗口（1 点 = 1 ns）</text>

            <!-- 触发基线 -->
            <line class="axis-line" :x1="AXIS_X0" :y1="AXIS_Y" :x2="AXIS_X1" :y2="AXIS_Y" />

            <!-- 触发标记 -->
            <g :key="cycleKey" class="trigger-mark">
              <line class="trigger-line" :x1="AXIS_X0" y1="40" :x2="AXIS_X0" y2="88" />
              <circle
                class="trigger-pulse"
                :class="{ animating: playing }"
                :cx="AXIS_X0"
                :cy="AXIS_Y"
                r="6"
              />
            </g>
            <text class="axis-tag" x="52" y="68" text-anchor="end">t=0</text>

            <!-- 第一段色块 -->
            <rect
              v-if="firstVisible"
              class="seg-first"
              :class="{ lit: phase >= 1, over: firstOverflow }"
              :x="firstX1"
              :width="Math.max(firstX2 - firstX1, 1)"
              :y="AXIS_Y - 16"
              height="16"
              rx="2"
            />
            <text
              v-if="firstVisible"
              class="seg-tag first"
              :x="labelX(firstX1, firstX2)"
              y="40"
              text-anchor="middle"
            >
              第一段 pos {{ firstStart }} · len {{ firstLen }}
            </text>
            <text
              v-else-if="firstLen <= 0"
              class="seg-tag muted"
              :x="AXIS_X0 + 70"
              y="40"
            >
              第一段未使能
            </text>

            <!-- 第二段搜索范围：从 second_pos 到搜索上限 -->
            <rect
              v-if="secondVisible"
              class="search-range"
              :class="{ empty: searchEmpty }"
              :x="secondX1"
              :width="Math.max(secondX2 - secondX1, 1)"
              :y="AXIS_Y"
              height="16"
              rx="2"
            />
            <text
              v-if="secondVisible"
              class="search-range-tag"
              :x="secondX1 + 6"
              :y="AXIS_Y + 12"
            >
              搜索
            </text>

            <!-- 8 点搜索窗口在范围内滑动，命中即停 -->
            <g
              v-if="playing && secondVisible && phase >= 2 && phase < 4"
              :key="`search-${cycleKey}`"
              class="search-window"
              :class="{ found: hit }"
              :style="{ '--travel': `${searchTravel}px` }"
            >
              <rect :x="secondX1" :y="AXIS_Y" width="10" height="16" rx="2" />
            </g>

            <!-- 命中后从信号位置开始采集第二段 -->
            <template v-if="secondVisible && phase >= 4 && hit">
              <rect
                class="seg-second"
                :class="{ over: secondOverflow }"
                :x="collectX1"
                :width="Math.max(collectX2 - collectX1, 1)"
                :y="AXIS_Y"
                height="16"
                rx="2"
              />
              <line
                class="hit-mark"
                :x1="collectX1"
                :y1="AXIS_Y - 5"
                :x2="collectX1"
                :y2="AXIS_Y + 20"
              />
            </template>

            <text
              v-if="secondVisible"
              class="seg-tag second"
              :x="secondTagX"
              y="100"
              text-anchor="middle"
            >
              {{ secondTag }}
            </text>
            <text
              v-else-if="secondLen <= 0"
              class="seg-tag muted"
              :x="AXIS_X1 - 70"
              y="100"
              text-anchor="end"
            >
              第二段未使能
            </text>

            <!-- 扫描线 -->
            <g v-if="playing && phase >= 0 && phase < 4" :key="`scan-${cycleKey}`" class="scan-line">
              <line :x1="AXIS_X0" y1="34" :x2="AXIS_X0" y2="104" />
            </g>

            <!-- 刻度 -->
            <g class="axis-ticks">
              <text
                v-for="tick in [0, 0.25, 0.5, 0.75, 1]"
                :key="tick"
                :x="AXIS_X0 + tick * (AXIS_X1 - AXIS_X0)"
                y="120"
                :text-anchor="tick === 0 ? 'start' : tick === 1 ? 'end' : 'middle'"
              >
                {{ Math.round(tick * wave) }}
              </text>
            </g>

            <!-- 参数警告 -->
            <text v-if="warnings.length" class="warn-text" :x="AXIS_X0" y="136">
              ⚠ {{ warnings.join(" · ") }}
            </text>

            <!-- ===== 第一段详情 ===== -->
            <g>
              <rect class="panel-bg" x="10" y="148" width="240" height="262" rx="10" />
              <text class="panel-title" x="26" y="170">第一段</text>
              <text class="panel-sub" x="26" y="186">
                pos {{ firstStart }} · len {{ firstLen }}（{{ firstLen }} ns）
              </text>

              <template v-if="firstLen > 0">
                <line class="base-line" x1="26" y1="330" x2="234" y2="330" />
                <text class="base-tag" x="30" y="344">M</text>
                <path
                  class="wave-path first"
                  :class="{ lit: phase >= 1 }"
                  :d="firstWavePath"
                />
                <text class="target-tag" x="126" y="222" text-anchor="middle">目标 1</text>
              </template>
              <text v-else class="panel-empty" x="130" y="260" text-anchor="middle">
                该段未使能
              </text>
            </g>

            <!-- ===== 8 点判定 ===== -->
            <g>
              <rect class="panel-bg" x="260" y="148" width="240" height="262" rx="10" />
              <text class="panel-title" x="276" y="170">8 点判定</text>
              <text class="panel-sub" x="276" y="186">滑动搜索 · 相对基线 M</text>

              <!-- 值阈值线 -->
              <line
                v-if="phase >= 2"
                class="value-threshold"
                x1="278"
                :y1="valueThresholdY"
                x2="482"
                :y2="valueThresholdY"
              />
              <line
                v-if="phase >= 2"
                class="value-threshold"
                x1="278"
                :y1="valueThresholdY"
                x2="482"
                :y2="valueThresholdY"
              />
              <g v-if="phase >= 2">
                <rect
                  class="threshold-label-bg"
                  :x="482 - thresholdLabelWidth"
                  :y="thresholdLabelY - 10"
                  :width="thresholdLabelWidth"
                  height="13"
                  rx="2"
                />
                <text
                  class="threshold-tag"
                  x="482"
                  :y="thresholdLabelY"
                  text-anchor="end"
                >
                  值阈值 M+{{ minLevel }}{{ thresholdOutOfRange ? "（超出量程）" : "" }}
                </text>
              </g>

              <g v-for="(bar, i) in bars" :key="i">
                <rect
                  class="bar"
                  :class="{ grown: phase >= 2, pass: phase >= 3 && hit, fail: phase >= 3 && !hit }"
                  :x="bar.x"
                  :y="bar.y"
                  width="18"
                  :height="bar.h"
                  rx="2"
                  :style="{ transitionDelay: `${i * 55}ms` }"
                />
              </g>
              <line class="base-line" x1="278" y1="330" x2="482" y2="330" />

              <text
                v-if="phase < 2"
                class="panel-empty"
                x="380"
                y="272"
                text-anchor="middle"
              >
                点击“播放”查看判定过程
              </text>

              <template v-if="phase >= 3">
                <text class="judge-line" x="276" y="356">
                  Σ8 − 8M =
                  <tspan class="judge-num">{{ sumDelta }}</tspan>
                  {{ sumPass ? ">" : "≤" }} 和阈值 {{ sumLevel }}
                  <tspan :class="sumPass ? 'ok' : 'ng'">{{ sumPass ? "✓" : "✗" }}</tspan>
                </text>
                <text class="judge-line" x="276" y="376">
                  max − M =
                  <tspan class="judge-num">{{ maxDelta }}</tspan>
                  {{ maxPass ? ">" : "≤" }} 值阈值 {{ minLevel }}
                  <tspan :class="maxPass ? 'ok' : 'ng'">{{ maxPass ? "✓" : "✗" }}</tspan>
                </text>
                <text class="judge-result" :class="hit ? 'ok' : 'ng'" x="276" y="398">
                  {{ hit ? "两项均满足 → 采集第二段" : "条件不满足 → 不采集第二段" }}
                </text>
              </template>
            </g>

            <!-- ===== 第二段详情 ===== -->
            <g>
              <rect class="panel-bg" x="510" y="148" width="240" height="262" rx="10" />
              <text class="panel-title" x="526" y="170">第二段</text>
              <text class="panel-sub" x="526" y="186">
                {{
                  phase >= 4 && hit
                    ? `起点 ${collectStart} · len ${secondLen}`
                    : `搜索起点 ${secondStart} · len ${secondLen}`
                }}
              </text>

              <template v-if="secondLen > 0">
                <template v-if="phase >= 4 && hit">
                  <line class="base-line" x1="526" y1="330" x2="734" y2="330" />
                  <text class="base-tag" x="530" y="344">M</text>
                  <path class="wave-path second" :d="secondWavePath" />
                  <text class="target-tag" x="626" y="226" text-anchor="middle">目标 2</text>
                </template>
                <template v-else>
                  <rect
                    class="pending-box"
                    :class="{ failed: phase >= 4 && !hit }"
                    x="526"
                    y="196"
                    width="208"
                    height="134"
                    rx="8"
                  />
                  <text class="panel-empty" x="630" y="258" text-anchor="middle">
                    {{ phase >= 4 && !hit ? "未采集第二段" : "等待判定" }}
                  </text>
                  <text class="panel-hint" x="630" y="278" text-anchor="middle">
                    {{ phase >= 4 && !hit ? "阈值条件未满足" : "8 点判定通过后采集" }}
                  </text>
                </template>
              </template>
              <text v-else class="panel-empty" x="630" y="260" text-anchor="middle">
                该段未使能
              </text>
            </g>
          </svg>
        </div>

        <footer class="modal-foot">
          <div class="strength">
            <span class="strength-label">回波强度</span>
            <div class="seg">
              <button
                type="button"
                :class="{ active: strength === 'strong' }"
                @click="setStrength('strong')"
              >
                强
              </button>
              <button
                type="button"
                :class="{ active: strength === 'weak' }"
                @click="setStrength('weak')"
              >
                弱
              </button>
            </div>
          </div>
          <span class="summary">
            采样 {{ wave }} 点（{{ micros }} µs）· 第一段 {{ firstStart }}+{{ firstLen }} ·
            第二段 搜索 {{ secondStart }} 起 · 采集 {{ secondLen }} 点
          </span>
          <button @click="emit('close')">关闭</button>
        </footer>

        <p class="foot-note">
          8 点数据为示例值，判定公式为 Σ8−8M &gt; 和阈值且 max−M &gt; 值阈值，实际判定由设备完成。
        </p>
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
  width: 820px;
  max-width: calc(100vw - 40px);
  max-height: 92vh;
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

/* ---------- 步骤指示器 ---------- */
.steps {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 18px 0;
  flex-shrink: 0;
}

.step {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 10px 0 6px;
  border: 1px solid var(--border-color);
  border-radius: 999px;
  background: var(--bg-inset);
  color: var(--text-tertiary);
  font-size: 11.5px;
  transition: background 0.2s ease, color 0.2s ease, border-color 0.2s ease;
}

.step-index {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 15px;
  height: 15px;
  border-radius: 50%;
  background: #fff;
  border: 1px solid var(--border-color);
  font-size: 9.5px;
  font-style: normal;
  font-weight: 600;
}

.step.active {
  border-color: #bcd0f8;
  background: var(--accent-soft);
  color: var(--accent-dim);
}

.step.active .step-index {
  border-color: var(--accent);
  color: var(--accent);
}

.step.done {
  color: var(--text-secondary);
}

.step.done .step-index {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

/* 手动播放按钮 */
.play-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  margin-left: auto;
  padding: 0 12px 0 9px;
  border: 1px solid var(--accent-dim);
  border-radius: 7px;
  background: linear-gradient(180deg, #3b74f5, var(--accent-dim));
  box-shadow: 0 1px 2px rgba(29, 78, 216, 0.24);
  color: #fff;
  font-size: 11.5px;
  font-weight: 550;
}

.play-btn svg {
  width: 11px;
  height: 11px;
  flex-shrink: 0;
}

.play-btn:hover {
  border-color: var(--accent-dim);
  background: linear-gradient(180deg, #4c80f7, #2358dd);
  color: #fff;
}

.play-btn:active {
  transform: none;
  box-shadow: 0 1px 2px rgba(29, 78, 216, 0.24);
}

.play-btn.playing {
  border-color: #bcd0f8;
  background: var(--accent-soft);
  box-shadow: none;
  color: var(--accent-dim);
}

.play-btn:disabled {
  opacity: 0.75;
  cursor: default;
}

/* ---------- 示意图 ---------- */
.diagram {
  padding: 2px 14px 0;
  overflow: auto;
  flex-shrink: 0;
}

.diagram svg {
  display: block;
  width: 100%;
  height: auto;
}

.zone-label {
  fill: var(--text-tertiary);
  font-size: 10.5px;
  letter-spacing: 0.06em;
}

.axis-line {
  stroke: #c3ccdb;
  stroke-width: 1.4;
}

.axis-tag {
  fill: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 9.5px;
}

.axis-ticks text {
  fill: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 9.5px;
}

.trigger-line {
  stroke: var(--accent);
  stroke-width: 1.6;
}

.trigger-mark {
  transform-box: fill-box;
}

.trigger-pulse {
  fill: var(--accent);
  transform-box: fill-box;
  transform-origin: center;
}

.trigger-pulse.animating {
  animation: trigger-burst 1.3s ease-out infinite;
}

@keyframes trigger-burst {
  0% {
    transform: scale(1);
    opacity: 1;
  }
  60%,
  100% {
    transform: scale(2.2);
    opacity: 0;
  }
}

/* 区间色块 */
.seg-first {
  fill: #c7d9fb;
  opacity: 0.25;
  transition: opacity 0.35s ease, fill 0.35s ease;
}

.seg-first.lit {
  fill: var(--accent);
  opacity: 0.6;
}

/* 第二段搜索范围 */
.search-range {
  fill: #f6f8fb;
  stroke: #d8dee9;
  stroke-width: 1;
  stroke-dasharray: 4 3;
}

.search-range.empty {
  stroke: var(--error);
}

.search-range-tag {
  fill: #b6c0cf;
  font-size: 8.5px;
}

/* 搜索窗口：在搜索范围内滑动 */
.search-window rect {
  fill: var(--accent);
  opacity: 0.28;
  animation: search-move 1.3s ease-out forwards;
}

.search-window.found rect {
  fill: var(--success);
  opacity: 0.45;
}

@keyframes search-move {
  from {
    transform: translateX(0);
  }
  to {
    transform: translateX(var(--travel, 0px));
  }
}

.hit-mark {
  stroke: var(--success);
  stroke-width: 1.4;
}

/* 第二段实际采集区间 */
.seg-second {
  fill: #8fd6ab;
  stroke: var(--success);
  stroke-width: 1.2;
  opacity: 0.8;
}

.over {
  stroke: var(--error);
  stroke-dasharray: 4 3;
}

.seg-tag {
  fill: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 9.5px;
}

.seg-tag.first {
  fill: var(--accent-dim);
}

.seg-tag.second {
  fill: #0e6b3c;
}

.seg-tag.muted {
  fill: var(--text-tertiary);
}

.scan-line line {
  stroke: var(--accent);
  stroke-width: 1.2;
  opacity: 0.55;
  animation: scan-move 5.2s linear;
}

@keyframes scan-move {
  from {
    transform: translateX(0);
  }
  to {
    transform: translateX(680px);
  }
}

.warn-text {
  fill: var(--error);
  font-size: 10.5px;
}

/* 面板 */
.panel-bg {
  fill: #fbfcfe;
  stroke: #e9eef5;
  stroke-width: 1.1;
}

.panel-title {
  fill: var(--text-primary);
  font-size: 12.5px;
  font-weight: 650;
}

.panel-sub {
  fill: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 9.5px;
}

.base-line {
  stroke: #d8dee9;
  stroke-width: 1;
  stroke-dasharray: 4 3;
}

.base-tag {
  fill: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 9px;
}

.panel-empty {
  fill: var(--text-tertiary);
  font-size: 11.5px;
}

.panel-hint {
  fill: #b6c0cf;
  font-size: 10px;
}

.pending-box {
  fill: none;
  stroke: #d8dee9;
  stroke-width: 1.2;
  stroke-dasharray: 5 4;
}

.pending-box.failed {
  stroke: #e3b4b4;
  stroke-dasharray: 5 4;
}

.target-tag {
  fill: var(--text-secondary);
  font-size: 10px;
}

/* 波形 */
.wave-path {
  fill: none;
  stroke-width: 2;
  stroke-linejoin: round;
  opacity: 0.22;
  transition: opacity 0.4s ease;
}

.wave-path.lit {
  opacity: 1;
}

.wave-path.first {
  stroke: var(--accent);
}

.wave-path.second {
  stroke: var(--success);
  opacity: 1;
}

/* 8 点柱状 */
.bar {
  fill: #bcd0f8;
  transform-box: fill-box;
  transform-origin: bottom;
  transform: scaleY(0);
  transition: transform 0.35s cubic-bezier(0.16, 1, 0.3, 1),
    fill 0.3s ease;
}

.bar.grown {
  transform: scaleY(1);
}

.bar.pass {
  fill: var(--success);
}

.bar.fail {
  fill: #cbd3e0;
}

.value-threshold {
  stroke: var(--warning);
  stroke-width: 1.2;
  stroke-dasharray: 5 4;
}

.threshold-tag {
  fill: var(--warning);
  font-family: var(--font-mono);
  font-size: 9px;
}

.threshold-label-bg {
  fill: #fff;
  opacity: 0.9;
}

.judge-line {
  fill: var(--text-secondary);
  font-size: 10px;
}

.judge-num {
  fill: var(--text-primary);
  font-family: var(--font-mono);
  font-weight: 600;
}

.judge-result {
  font-size: 10.5px;
  font-weight: 600;
}

.judge-result.ok {
  fill: #0e6b3c;
}

.judge-result.ng {
  fill: var(--error);
}

.ok {
  fill: var(--success);
}

.ng {
  fill: var(--error);
}

/* ---------- 底部 ---------- */
.modal-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 18px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-inset);
  flex-shrink: 0;
}

.strength {
  display: flex;
  align-items: center;
  gap: 8px;
}

.strength-label {
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.seg {
  display: inline-flex;
  padding: 2px;
  border: 1px solid var(--border-color);
  border-radius: 7px;
  background: #fff;
}

.seg button {
  height: 22px;
  padding: 0 12px;
  border: none;
  border-radius: 5px;
  background: transparent;
  box-shadow: none;
  font-size: 11.5px;
  color: var(--text-secondary);
}

.seg button:hover {
  background: var(--bg-inset);
  border: none;
}

.seg button:active {
  transform: none;
}

.seg button.active {
  background: var(--accent-soft);
  color: var(--accent-dim);
  font-weight: 600;
}

.summary {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: center;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-tertiary);
}

.foot-note {
  padding: 7px 18px 9px;
  border-top: 1px solid var(--border-color);
  background: #fff;
  font-size: 10.5px;
  color: var(--text-tertiary);
  flex-shrink: 0;
}

@media (prefers-reduced-motion: reduce) {
  .trigger-pulse.animating,
  .scan-line line {
    animation: none;
  }

  .scan-line,
  .search-window {
    display: none;
  }
}
</style>
