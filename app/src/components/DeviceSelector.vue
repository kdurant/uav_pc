<script setup lang="ts">
import type { DiscoveredDevice } from "../types";

const props = defineProps<{
  devices: DiscoveredDevice[];
  visible: boolean;
}>();

const emit = defineEmits<{
  (e: "select", device: DiscoveredDevice): void;
  (e: "cancel"): void;
}>();

function selectDevice(device: DiscoveredDevice) {
  emit("select", device);
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-overlay" @click.self="emit('cancel')">
      <div class="modal">
        <header class="modal-head">
          <div class="modal-title-wrap">
            <div class="modal-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
                stroke-linecap="round" stroke-linejoin="round">
                <rect x="2.5" y="5" width="19" height="12" rx="2.5" />
                <path d="M6 9h5M6 12.5h3" />
                <circle cx="17" cy="11" r="1.6" />
                <path d="M8 20h8" />
              </svg>
            </div>
            <div class="modal-titles">
              <h3>选择采集卡设备</h3>
              <p>发现 {{ props.devices.length }} 台可用设备，点击连接</p>
            </div>
          </div>
          <button class="close-btn" aria-label="关闭" @click="emit('cancel')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
              stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </header>

        <div v-if="devices.length === 0" class="empty">
          <div class="empty-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"
              stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="2.5" />
              <path d="M7.5 7.5a6.4 6.4 0 000 9M16.5 16.5a6.4 6.4 0 000-9M4.5 4.5a10.6 10.6 0 000 15M19.5 19.5a10.6 10.6 0 000-15" />
            </svg>
          </div>
          <p class="empty-title">未发现设备</p>
          <p class="empty-desc">请检查采集卡电源与网络连接后重试</p>
        </div>

        <ul v-else class="device-list">
          <li
            v-for="device in devices"
            :key="device.ip"
            class="device-item"
            @click="selectDevice(device)"
          >
            <div class="device-avatar">
              <span class="device-pulse"></span>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
                stroke-linecap="round" stroke-linejoin="round">
                <rect x="3" y="4" width="18" height="12" rx="2.5" />
                <path d="M7 20h10M12 16v4M7 8h4" />
              </svg>
            </div>
            <div class="device-info">
              <span class="device-ip">{{ device.ip }}</span>
              <span class="device-meta">UDP · 端口 {{ device.port }}</span>
            </div>
            <span class="device-connect">连接</span>
          </li>
        </ul>

        <footer class="modal-actions">
          <button @click="emit('cancel')">取消</button>
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
  width: 420px;
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

.modal-title-wrap {
  display: flex;
  align-items: center;
  gap: 11px;
}

.modal-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  background: var(--accent-soft);
  color: var(--accent);
  flex-shrink: 0;
}

.modal-icon svg {
  width: 19px;
  height: 19px;
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
  border-color: transparent;
}

.close-btn svg {
  width: 15px;
  height: 15px;
}

/* 设备列表 */
.device-list {
  list-style: none;
  padding: 10px;
  max-height: 300px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.device-item {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 11px 12px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  cursor: pointer;
  transition: border-color 0.16s ease, background 0.16s ease, box-shadow 0.16s ease;
}

.device-item:hover {
  background: var(--bg-inset);
  border-color: #bcd0f8;
  box-shadow: 0 2px 8px rgba(37, 99, 235, 0.08);
}

.device-avatar {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border-radius: 9px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  flex-shrink: 0;
}

.device-avatar svg {
  width: 18px;
  height: 18px;
}

.device-pulse {
  position: absolute;
  top: -2px;
  right: -2px;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--success);
  border: 1.5px solid #fff;
}

.device-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.device-ip {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.device-meta {
  font-size: 11px;
  color: var(--text-tertiary);
}

.device-connect {
  margin-left: auto;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--accent);
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--accent-soft);
  flex-shrink: 0;
}

/* 空状态 */
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  padding: 36px 20px;
  text-align: center;
}

.empty-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  margin-bottom: 6px;
  border-radius: 12px;
  background: var(--bg-tertiary);
  color: var(--text-tertiary);
}

.empty-icon svg {
  width: 23px;
  height: 23px;
}

.empty-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
}

.empty-desc {
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 18px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-inset);
}
</style>
