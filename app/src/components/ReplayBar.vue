<script setup lang="ts">
import { onMounted, onUnmounted, watch } from "vue";
import { useReplay } from "../composables/useReplay";

/**
 * 离线回放控制条：挂在「数据预览」面板下方，与实时预览共用同一个图表。
 * 组件只在回放模式激活时挂载，键盘事件因此不会影响实时模式。
 */
const {
    info,
    frameCount,
    frameCountText,
    fileName,
    frameIndex,
    intervalMs,
    playing,
    busy,
    progress,
    chooseFile,
    step,
    togglePlay,
    seekTo,
    reschedule,
    exit,
} = useReplay();

watch(intervalMs, reschedule);

function onSeek(event: Event) {
    seekTo(Number((event.target as HTMLInputElement).value));
}

function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    // 输入框/按钮聚焦时不劫持按键，避免和原生交互冲突
    if (target && /^(INPUT|TEXTAREA|SELECT|BUTTON)$/.test(target.tagName)) return;

    if (event.key === "ArrowLeft") {
        event.preventDefault();
        step(-1);
    } else if (event.key === "ArrowRight") {
        event.preventDefault();
        step(1);
    } else if (event.key === " ") {
        event.preventDefault();
        togglePlay();
    }
}

onMounted(() => {
    // 进入回放后让出焦点，避免空格键触发工具栏按钮的再次点击
    (document.activeElement as HTMLElement | null)?.blur();
    window.addEventListener("keydown", onKeydown);
});
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="replay-bar">
    <div class="toolbar-row">
      <button class="file-btn" :disabled="busy" @click="chooseFile">
        {{ info ? "更换文件" : "选择数据文件" }}
      </button>

      <span class="file-name" :title="info?.path">
        {{ info ? fileName : busy ? "正在解析文件..." : "未选择文件" }}
      </span>

      <div class="transport">
        <button title="上一帧（←）" :disabled="!info || frameIndex <= 0" @click="step(-1)">
          上一帧
        </button>
        <button
          class="play"
          :class="{ active: playing }"
          :disabled="!info"
          :title="playing ? '暂停（空格）' : '播放（空格）'"
          @click="togglePlay"
        >
          {{ playing ? "暂停" : "播放" }}
        </button>
        <button
          title="下一帧（→）"
          :disabled="!info || frameIndex >= frameCount - 1"
          @click="step(1)"
        >
          下一帧
        </button>
      </div>

      <label class="interval">
        间隔
        <input
          v-model.number="intervalMs"
          type="number"
          min="10"
          max="2000"
          step="10"
          :disabled="!info"
        />
        ms
      </label>

      <span class="counter">
        <b>{{ info ? frameIndex + 1 : 0 }}</b> / {{ frameCountText }} 帧
      </span>

      <button class="exit-btn" title="退出回放，恢复实时预览" @click="exit">退出回放</button>
    </div>

    <div class="slider-row">
      <input
        class="slider"
        type="range"
        min="0"
        :max="Math.max(0, frameCount - 1)"
        step="1"
        :value="frameIndex"
        :disabled="!info"
        :style="{ '--progress': progress + '%' }"
        @input="onSeek"
      />
    </div>
  </div>
</template>

<style scoped>
.replay-bar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 14px 12px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-secondary);
  flex-shrink: 0;
}

.toolbar-row {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.file-btn {
  flex-shrink: 0;
  height: 30px;
  padding: 0 12px;
  font-size: 12.5px;
}

.file-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.transport {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.transport button {
  height: 30px;
  padding: 0 10px;
  font-size: 12.5px;
}

.play {
  min-width: 62px;
}

.play.active {
  border-color: var(--accent);
  color: var(--accent-dim);
  background: var(--accent-glow, #eff4ff);
}

.interval {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-secondary);
}

.interval input {
  width: 66px;
  height: 30px;
  padding: 0 8px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: 12.5px;
  color: var(--text-primary);
  background: #fff;
  text-align: right;
}

.interval input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-glow);
}

.counter {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.counter b {
  color: var(--text-primary);
  font-weight: 650;
}

.exit-btn {
  flex-shrink: 0;
  height: 30px;
  padding: 0 12px;
  font-size: 12.5px;
}

.slider-row {
  display: flex;
}

.slider {
  flex: 1;
  height: 20px;
  margin: 0;
  cursor: pointer;
  appearance: none;
  background: transparent;
}

.slider:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.slider::-webkit-slider-runnable-track {
  height: 5px;
  border-radius: 3px;
  background: linear-gradient(
    to right,
    var(--accent, #2f6fed) 0%,
    var(--accent, #2f6fed) var(--progress, 0%),
    var(--border-color) var(--progress, 0%),
    var(--border-color) 100%
  );
}

.slider::-webkit-slider-thumb {
  appearance: none;
  width: 14px;
  height: 14px;
  margin-top: -4.5px;
  border-radius: 50%;
  background: #fff;
  border: 2px solid var(--accent, #2f6fed);
  box-shadow: 0 1px 3px rgba(16, 24, 40, 0.24);
}

.slider::-moz-range-track {
  height: 5px;
  border-radius: 3px;
  background: var(--border-color);
}

.slider::-moz-range-progress {
  height: 5px;
  border-radius: 3px;
  background: var(--accent, #2f6fed);
}

.slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #fff;
  border: 2px solid var(--accent, #2f6fed);
}

@media (max-width: 1280px) {
  .file-name {
    display: none;
  }
}
</style>
