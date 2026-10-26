<script setup lang="ts">
import { useToast } from "../composables/useToast";
import type { ToastType } from "../composables/useToast";

const { toasts, dismiss } = useToast();

/** 20×20 线性图标路径，与项目内现有 SVG 图标风格保持一致 */
const ICON_PATHS: Record<ToastType, string> = {
  success: "M4 10.5l4 4 8-8",
  info: "M10 2.5a7.5 7.5 0 1 0 0 15 7.5 7.5 0 0 0 0-15zM10 9.2v4.6M10 6.4h.01",
  warning: "M10 3.2l7 12.8H3l7-12.8zM10 8v4.2M10 14.4h.01",
  error: "M10 2.5a7.5 7.5 0 1 0 0 15 7.5 7.5 0 0 0 0-15zM7.4 7.4l5.2 5.2M12.6 7.4l-5.2 5.2",
};
</script>

<template>
  <Teleport to="body">
    <div class="toast-host" aria-label="操作通知">
      <TransitionGroup name="toast" tag="div" class="toast-stack">
        <div
          v-for="item in toasts"
          :key="item.id"
          class="toast"
          :class="`toast-${item.type}`"
          :role="item.type === 'error' ? 'alert' : 'status'"
          aria-live="polite"
        >
          <span class="toast-icon" aria-hidden="true">
            <svg
              viewBox="0 0 20 20"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path :d="ICON_PATHS[item.type]" />
            </svg>
          </span>

          <div class="toast-content">
            <div class="toast-title">
              <span class="toast-title-text">{{ item.title }}</span>
              <span v-if="item.count > 1" class="toast-count">×{{ item.count }}</span>
            </div>
            <p v-if="item.message" class="toast-message">{{ item.message }}</p>
          </div>

          <button
            type="button"
            class="toast-close"
            aria-label="关闭通知"
            @click="dismiss(item.id)"
          >
            ×
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-host {
  position: fixed;
  left: 50%;
  bottom: 22px;
  transform: translateX(-50%);
  width: min(460px, calc(100vw - 40px));
  z-index: 2000;
  pointer-events: none;
}

.toast-stack {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}

.toast {
  pointer-events: auto;
  display: flex;
  align-items: flex-start;
  gap: 9px;
  width: fit-content;
  min-width: 260px;
  max-width: 100%;
  padding: 9px 8px 9px 11px;
  background: #fff;
  border: 1px solid var(--border-color);
  border-left: 3px solid var(--accent);
  border-radius: var(--radius);
  box-shadow: var(--shadow-pop);
}

.toast-icon {
  display: flex;
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  margin-top: 1px;
  color: var(--accent);
}

.toast-icon svg {
  width: 100%;
  height: 100%;
}

.toast-content {
  flex: 1;
  min-width: 0;
}

.toast-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  line-height: 1.35;
  color: var(--text-primary);
}

.toast-title-text {
  overflow-wrap: break-word;
  word-break: break-word;
}

.toast-count {
  flex-shrink: 0;
  padding: 0 5px;
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 600;
  line-height: 15px;
  color: var(--text-secondary);
  background: var(--bg-inset);
  border: 1px solid var(--border-color);
  border-radius: 999px;
}

.toast-message {
  margin-top: 2px;
  font-size: 12px;
  line-height: 1.45;
  color: var(--text-secondary);
  max-height: 108px;
  overflow-y: auto;
  overflow-wrap: break-word;
  word-break: break-word;
  white-space: pre-wrap;
}

.toast-close {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  box-shadow: none;
  font-size: 15px;
  line-height: 1;
  color: var(--text-tertiary);
  cursor: pointer;
}

.toast-close:hover {
  background: var(--bg-hover);
  border: none;
  color: var(--text-primary);
}

.toast-close:active {
  transform: none;
  box-shadow: none;
}

.toast-success {
  border-left-color: var(--success);
}

.toast-success .toast-icon {
  color: var(--success);
}

.toast-warning {
  border-left-color: var(--warning);
}

.toast-warning .toast-icon {
  color: var(--warning);
}

.toast-error {
  border-left-color: var(--error);
}

.toast-error .toast-icon {
  color: var(--error);
}

/* 进出场：底部滑入，多条同时存在时自动让位 */
.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px) scale(0.98);
}

.toast-move {
  transition: transform 0.18s ease;
}
</style>
