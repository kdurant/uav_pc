<script setup lang="ts">
withDefaults(
  defineProps<{
    visible: boolean;
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    danger?: boolean;
  }>(),
  {
    confirmText: "确定",
    cancelText: "取消",
    danger: false,
  }
);

const emit = defineEmits<{
  (e: "confirm"): void;
  (e: "cancel"): void;
}>();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="confirm-overlay" @click.self="emit('cancel')">
      <div class="confirm-modal" role="dialog" aria-modal="true" :aria-label="title">
        <h3>{{ title }}</h3>
        <p>{{ message }}</p>
        <div class="confirm-actions">
          <button @click="emit('cancel')">{{ cancelText }}</button>
          <button
            class="confirm-danger"
            :class="{ danger }"
            @click="emit('confirm')"
          >
            {{ confirmText }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.confirm-overlay {
  position: fixed;
  inset: 0;
  z-index: 1100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(15, 23, 42, 0.32);
  animation: confirm-in 0.16s ease-out;
}

@keyframes confirm-in {
  from {
    opacity: 0;
  }
}

.confirm-modal {
  width: 400px;
  max-width: calc(100vw - 40px);
  padding: 18px;
  background: #fff;
  border: 1px solid var(--border-color);
  border-radius: 14px;
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.12);
}

.confirm-modal h3 {
  font-size: 14.5px;
  font-weight: 650;
  color: var(--text-primary);
}

.confirm-modal p {
  margin-top: 8px;
  font-size: 12.5px;
  line-height: 1.7;
  color: var(--text-secondary);
  word-break: break-all;
}

.confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.confirm-actions button {
  height: 30px;
  padding: 0 14px;
  font-size: 12.5px;
}

.confirm-danger.danger {
  color: #b42318;
  border-color: #f1c9c5;
  background: #fff;
}

.confirm-danger.danger:hover:not(:disabled) {
  background: #fef3f2;
  border-color: #e9a9a3;
}
</style>
