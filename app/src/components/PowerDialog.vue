<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  visible: boolean;
  action: "reboot" | "shutdown";
  phase: "confirm" | "countdown" | "done";
  remaining: number;
  error?: string;
}>();

const emit = defineEmits<{
  (e: "confirm"): void;
  (e: "cancel"): void;
  (e: "close"): void;
}>();

const isReboot = computed(() => props.action === "reboot");
const total = computed(() => (isReboot.value ? 25 : 15));

const title = computed(() => {
  if (props.phase === "confirm") {
    return isReboot.value ? "重启设备" : "关闭设备";
  }
  if (props.phase === "countdown") {
    return isReboot.value ? "设备重启中" : "设备关机中";
  }
  return isReboot.value ? "重启完成" : "已关机";
});

const subtitle = computed(() => {
  if (props.phase === "confirm") {
    return `预计耗时约 ${total.value} 秒`;
  }
  if (props.phase === "countdown") {
    return "请勿断电，等待操作完成";
  }
  return isReboot.value ? "设备已恢复，可重新搜索连接" : "设备已关闭，需人工上电";
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="modal-overlay"
      @click.self="phase === 'confirm' ? emit('cancel') : emit('close')"
    >
      <div class="modal">
        <header class="modal-head">
          <div class="modal-icon" :class="{ danger: phase !== 'done' }">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"
              stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 3v9" />
              <path d="M6.3 6.3a8 8 0 1011.4 0" />
            </svg>
          </div>
          <div class="modal-titles">
            <h3>{{ title }}</h3>
            <p>{{ subtitle }}</p>
          </div>
          <button
            v-if="phase !== 'countdown'"
            class="close-btn"
            aria-label="关闭"
            @click="emit('close')"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
              stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </header>

        <div class="modal-body">
          <template v-if="phase === 'confirm'">
            <p class="desc">
              设备将立即{{ isReboot ? "重启" : "关机" }}并断开连接，
              预计耗时约 {{ total }} 秒。
            </p>
            <p v-if="error" class="err">{{ error }}</p>
          </template>

          <template v-else-if="phase === 'countdown'">
            <div class="countdown">
              <span class="num">{{ remaining }}</span>
              <span class="unit">秒</span>
            </div>
            <p class="desc">
              {{ isReboot ? "重启" : "关机" }}过程中请勿断电。
            </p>
          </template>

          <template v-else>
            <p class="desc">
              {{ isReboot
                ? "设备已重启，请点击“搜索设备”重新连接。"
                : "设备已完成关机，需人工上电后才能再次连接。" }}
            </p>
          </template>
        </div>

        <footer class="modal-actions">
          <template v-if="phase === 'confirm'">
            <button @click="emit('cancel')">取消</button>
            <button class="danger" @click="emit('confirm')">
              确认{{ isReboot ? "重启" : "关机" }}
            </button>
          </template>
          <button v-else @click="emit('close')">关闭</button>
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
  width: 380px;
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
  gap: 11px;
  padding: 18px 18px 14px;
  border-bottom: 1px solid var(--border-color);
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

.modal-icon.danger {
  background: var(--error-soft);
  color: #b42318;
}

.modal-icon svg {
  width: 19px;
  height: 19px;
}

.modal-titles {
  flex: 1;
  min-width: 0;
  padding-top: 2px;
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
}

.close-btn svg {
  width: 15px;
  height: 15px;
}

.modal-body {
  padding: 16px 18px;
}

.desc {
  font-size: 12.5px;
  line-height: 1.65;
  color: var(--text-secondary);
}

.err {
  margin-top: 10px;
  padding: 7px 10px;
  border-radius: var(--radius-sm);
  background: var(--error-soft);
  color: #b42318;
  font-size: 12px;
  word-break: break-all;
}

.countdown {
  display: flex;
  align-items: baseline;
  justify-content: center;
  gap: 4px;
  margin-bottom: 10px;
}

.countdown .num {
  font-size: 40px;
  font-weight: 700;
  line-height: 1;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}

.countdown .unit {
  font-size: 13px;
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

.modal-actions button {
  height: 30px;
  padding: 0 14px;
  font-size: 12.5px;
}
</style>
