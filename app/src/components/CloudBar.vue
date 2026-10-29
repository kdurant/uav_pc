<script setup lang="ts">
import { computed } from "vue";
import { usePointCloud, type CloudDisplay } from "../composables/usePointCloud";

/**
 * 点云控制条：挂在「数据预览」面板下方，只在点云模式下显示。
 */
const {
    busy,
    progress,
    progressText,
    data,
    pointCountText,
    startFrame,
    frameCount,
    voxel,
    demCell,
    scanAngleDeg,
    refraction,
    display,
    zScale,
    currentPath,
    chooseFile,
    build,
    exit,
} = usePointCloud();

const fileName = computed(() => {
    const p = currentPath.value;
    if (!p) return "未选择文件";
    const parts = p.split(/[\\/]/);
    return parts[parts.length - 1] || p;
});

const depthText = computed(() => {
    const d = data.value;
    if (!d) return "-";
    return `${(d.zMax - d.zMin).toFixed(1)} m`;
});

const t0Text = computed(() => (data.value ? data.value.t0.toFixed(1) : "-"));

const displays: Array<{ key: CloudDisplay; label: string }> = [
    { key: "points", label: "点云" },
    { key: "dem", label: "DEM" },
    { key: "both", label: "叠加" },
];
</script>

<template>
  <div class="cloud-bar">
    <div class="row">
      <button class="file-btn" :disabled="busy" @click="chooseFile">选择文件</button>
      <span class="file-name" :title="currentPath">{{ fileName }}</span>

      <label class="field">起始帧<input v-model.number="startFrame" type="number" min="0" /></label>
      <label class="field">帧数<input v-model.number="frameCount" type="number" min="0" step="1000" /></label>
      <label class="field">体素<input v-model.number="voxel" type="number" min="0.05" max="5" step="0.05" /></label>
      <label class="field">格网<input v-model.number="demCell" type="number" min="0.2" max="10" step="0.2" /></label>
      <label class="field">扫描角<input v-model.number="scanAngleDeg" type="number" min="0" max="45" step="1" />°</label>
      <label class="check"><input v-model="refraction" type="checkbox" />折射</label>

      <button class="accent" :disabled="busy || !currentPath" @click="build">
        {{ busy ? "生成中..." : "生成" }}
      </button>
      <button class="exit-btn" title="退出点云模式" @click="exit">退出</button>
    </div>

    <div class="row second">
      <div class="seg-group">
        <button
          v-for="item in displays"
          :key="item.key"
          class="seg"
          :class="{ active: display === item.key }"
          @click="display = item.key"
        >
          {{ item.label }}
        </button>
      </div>

      <label class="field">垂直放大<input v-model.number="zScale" type="number" min="1" max="20" step="1" />×</label>

      <span class="stats">
        <span v-if="data">
          点数 <b>{{ pointCountText }}</b> · 帧数 <b>{{ data.frameCount.toLocaleString("zh-CN") }}</b> · 高差 <b>{{ depthText }}</b> · T0 <b>{{ t0Text }}</b> 样点
          <b v-if="data.truncated" class="warn">（点数已达上限，已截断）</b>
        </span>
        <span v-else>尚未生成点云</span>
      </span>

      <div v-if="busy" class="progress">
        <div class="progress-fill" :style="{ width: progress + '%' }"></div>
        <span class="progress-text">{{ progressText }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.cloud-bar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 14px 12px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-secondary);
  flex-shrink: 0;
}

.row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.second {
  gap: 14px;
}

.file-btn,
.exit-btn {
  height: 30px;
  padding: 0 12px;
  font-size: 12.5px;
}

.file-name {
  flex: 1;
  min-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--text-secondary);
}

.field {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}

.field input {
  width: 70px;
  height: 28px;
  padding: 0 6px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: 12px;
  color: var(--text-primary);
  background: #fff;
  text-align: right;
}

.field input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-glow);
}

.check {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--text-secondary);
}

.check input {
  margin: 0;
}

.seg-group {
  display: inline-flex;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  overflow: hidden;
  background: #fff;
}

.seg {
  height: 28px;
  padding: 0 10px;
  border: none;
  border-radius: 0;
  box-shadow: none;
  font-size: 12px;
  background: transparent;
  color: var(--text-secondary);
}

.seg + .seg {
  border-left: 1px solid var(--border-color);
}

.seg.active {
  background: var(--accent-glow, #eff4ff);
  color: var(--accent-dim, #2563eb);
  font-weight: 600;
}

.stats {
  margin-left: auto;
  font-size: 12px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.stats b {
  color: var(--text-primary);
  font-weight: 650;
}

.stats .warn {
  color: var(--danger, #dc2626);
}

.progress {
  position: relative;
  width: 180px;
  height: 20px;
  border-radius: 5px;
  background: var(--bg-inset, #eef2f7);
  overflow: hidden;
}

.progress-fill {
  position: absolute;
  inset: 0 auto 0 0;
  background: var(--accent-glow, #c9dbff);
  transition: width 0.2s linear;
}

.progress-text {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  font-size: 11px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
</style>
