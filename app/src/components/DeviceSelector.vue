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
        <h3>选择采集卡设备</h3>
        <div v-if="devices.length === 0" class="empty">
          未发现设备
        </div>
        <ul v-else class="device-list">
          <li
            v-for="device in devices"
            :key="device.ip"
            class="device-item"
            @click="selectDevice(device)"
          >
            <span class="device-ip">{{ device.ip }}</span>
            <span class="device-port">:{{ device.port }}</span>
          </li>
        </ul>
        <div class="modal-actions">
          <button @click="emit('cancel')">取消</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.modal {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: var(--panel-radius);
  padding: 24px;
  min-width: 360px;
  max-width: 480px;
}
.modal h3 {
  margin-bottom: 16px;
  font-size: 16px;
  color: var(--accent);
}
.empty {
  padding: 20px;
  text-align: center;
  color: var(--text-secondary);
}
.device-list {
  list-style: none;
  max-height: 240px;
  overflow-y: auto;
}
.device-item {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.15s;
}
.device-item:hover {
  background: var(--bg-tertiary);
}
.device-ip {
  font-weight: 600;
  color: var(--text-primary);
}
.device-port {
  color: var(--text-secondary);
  margin-left: 4px;
  font-size: 12px;
}
.modal-actions {
  margin-top: 16px;
  display: flex;
  justify-content: flex-end;
}
</style>
