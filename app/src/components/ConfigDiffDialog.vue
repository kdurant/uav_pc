<script setup lang="ts">
import type { ConfigDiffItem } from "../types";

defineProps<{
  visible: boolean;
  sn?: string;
  diffs: ConfigDiffItem[];
}>();

const emit = defineEmits<{
  (e: "confirm"): void;
  (e: "cancel"): void;
}>();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="diff-overlay" @click.self="emit('cancel')">
      <div class="diff-modal" role="dialog" aria-modal="true" aria-label="配置差异">
        <h3>配置差异</h3>
        <p class="diff-sub">
          设备 {{ sn || "-" }} 的本地配置与服务器配置不一致
        </p>

        <div class="diff-table-wrap">
          <table class="diff-table">
            <thead>
              <tr>
                <th>配置项</th>
                <th>本地</th>
                <th>服务器</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in diffs" :key="item.label">
                <td class="col-label">{{ item.label }}</td>
                <td class="col-value">{{ item.local }}</td>
                <td class="col-value remote">{{ item.remote }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="diff-actions">
          <button @click="emit('cancel')">取消</button>
          <button class="accent" @click="emit('confirm')">下载并更新</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.diff-overlay {
  position: fixed;
  inset: 0;
  z-index: 1100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(15, 23, 42, 0.32);
}

.diff-modal {
  width: 560px;
  max-width: calc(100vw - 40px);
  max-height: calc(100vh - 80px);
  display: flex;
  flex-direction: column;
  padding: 18px;
  background: #fff;
  border: 1px solid var(--border-color);
  border-radius: 14px;
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.12);
}

.diff-modal h3 {
  font-size: 14.5px;
  font-weight: 650;
  color: var(--text-primary);
}

.diff-sub {
  margin-top: 6px;
  font-size: 12px;
  color: var(--text-tertiary);
}

.diff-table-wrap {
  margin-top: 12px;
  overflow: auto;
  border: 1px solid var(--border-color);
  border-radius: 8px;
}

.diff-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.diff-table th,
.diff-table td {
  padding: 6px 10px;
  text-align: left;
  border-bottom: 1px solid var(--border-color);
}

.diff-table th {
  position: sticky;
  top: 0;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  font-weight: 600;
}

.diff-table tr:last-child td {
  border-bottom: none;
}

.col-label {
  color: var(--text-secondary);
}

.col-value {
  font-family: var(--font-mono);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.col-value.remote {
  color: #b45309;
  font-weight: 600;
}

.diff-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.diff-actions button {
  height: 30px;
  padding: 0 14px;
  font-size: 12.5px;
}
</style>
