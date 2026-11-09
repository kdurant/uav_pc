<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onUnmounted } from "vue";

/**
 * 调试菜单：集中放置离线回放、点云地形等调试入口。
 * 首次使用需输入密码，解锁后本次运行（会话）内不再重复校验。
 *
 * 注意：密码写在前端只能起到"防误触"作用，不能作为安全手段；
 * 默认密码 123456，如需修改请改动此常量。
 */
const DEBUG_PASSWORD = "123456";
const UNLOCK_KEY = "uav_pc_debug_unlocked";

const props = defineProps<{
    replayActive: boolean;
    cloudActive: boolean;
}>();

const emit = defineEmits<{
    (e: "replay" | "cloud" | "wizard" | "passthrough" | "profile"): void;
}>();

const rootRef = ref<HTMLDivElement | null>(null);
const menuOpen = ref(false);
const unlocked = ref(sessionStorage.getItem(UNLOCK_KEY) === "1");

// 密码弹窗状态
const askPassword = ref(false);
const password = ref("");
const errorText = ref("");
const passwordInputRef = ref<HTMLInputElement | null>(null);

/** 密码通过后要执行的动作：解锁本身或直接进入某个调试功能 */
let pendingAction: (() => void) | null = null;

function openPasswordDialog(action?: () => void) {
    pendingAction = action ?? null;
    password.value = "";
    errorText.value = "";
    menuOpen.value = false;
    askPassword.value = true;
    nextTick(() => passwordInputRef.value?.focus());
}

function closePasswordDialog() {
    askPassword.value = false;
    pendingAction = null;
    password.value = "";
    errorText.value = "";
}

/** 提交密码：正确则解锁并执行待办动作 */
function submitPassword() {
    if (password.value !== DEBUG_PASSWORD) {
        errorText.value = "密码错误，请重新输入";
        password.value = "";
        nextTick(() => passwordInputRef.value?.focus());
        return;
    }
    unlocked.value = true;
    sessionStorage.setItem(UNLOCK_KEY, "1");
    const action = pendingAction;
    closePasswordDialog();
    if (action) action();
    else menuOpen.value = true;
}

/** 点击「调试」按钮：已解锁直接展开菜单，未解锁先校验密码 */
function toggleMenu() {
    if (menuOpen.value) {
        menuOpen.value = false;
        return;
    }
    if (!unlocked.value) {
        openPasswordDialog();
        return;
    }
    menuOpen.value = true;
}

/** 菜单项点击：先收起菜单，再执行对应操作 */
function choose(action: "replay" | "cloud" | "wizard" | "passthrough" | "profile") {
    menuOpen.value = false;
    emit(action);
}

/** 点击组件之外关闭菜单（仅左键；右键/中键不关闭，避免误触） */
function onDocumentPointerDown(event: PointerEvent) {
    if (!menuOpen.value) return;
    if (event.button !== 0) return;
    const root = rootRef.value;
    if (root && !root.contains(event.target as Node)) menuOpen.value = false;
}

/** Esc 关闭菜单 / 密码框 */
function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    if (askPassword.value) closePasswordDialog();
    else if (menuOpen.value) menuOpen.value = false;
}

watch(askPassword, (open) => {
    if (open) nextTick(() => passwordInputRef.value?.focus());
});

onMounted(() => {
    document.addEventListener("pointerdown", onDocumentPointerDown);
    window.addEventListener("keydown", onKeydown);
});
onUnmounted(() => {
    document.removeEventListener("pointerdown", onDocumentPointerDown);
    window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div ref="rootRef" class="debug-menu">
    <button
      class="tool-btn"
      :class="{ active: menuOpen || replayActive || cloudActive }"
      title="调试菜单（需密码）"
      @click="toggleMenu"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M4 6h16M4 12h16M4 18h16" />
      </svg>
      调试
      <svg class="caret" :class="{ open: menuOpen }" viewBox="0 0 24 24" fill="none"
        stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M6 9l6 6 6-6" />
      </svg>
    </button>

    <!-- 下拉菜单 -->
    <div v-if="menuOpen" class="menu-panel">
      <button
        class="menu-item"
        @click="choose('wizard')"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="9" />
          <path d="M15.5 8.5l-2 5-5 2 2-5 5-2z" />
        </svg>
        <span>设置向导</span>
      </button>

      <button
        class="menu-item"
        @click="choose('passthrough')"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="4" width="18" height="16" rx="3" />
          <path d="M7 9l3 3-3 3" />
          <path d="M13 15h4" />
        </svg>
        <span>命令透传</span>
      </button>

      <button
        class="menu-item"
        :class="{ active: props.replayActive }"
        @click="choose('replay')"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 5v14l9-7-9-7z" />
          <path d="M14 5h2v14h-2z" />
        </svg>
        <span>{{ props.replayActive ? "退出离线回放" : "离线回放" }}</span>
        <span v-if="props.replayActive" class="menu-dot"></span>
      </button>

      <button
        class="menu-item"
        @click="choose('profile')"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 5h16v14H4z" />
          <path d="M8 9h8M8 13h5" />
        </svg>
        <span>设备档案</span>
      </button>

      <button
        class="menu-item"
        :class="{ active: props.cloudActive }"
        @click="choose('cloud')"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9L12 3z" />
          <path d="M4 7.5l8 4.5 8-4.5" />
          <path d="M12 12v9" />
        </svg>
        <span>{{ props.cloudActive ? "退出点云" : "点云地形" }}</span>
        <span v-if="props.cloudActive" class="menu-dot"></span>
      </button>
    </div>

    <!-- 密码弹窗 -->
    <Teleport to="body">
      <div v-if="askPassword" class="modal-overlay" @click.self="closePasswordDialog">
        <div class="modal">
          <header class="modal-head">
            <div class="modal-title-wrap">
              <div class="modal-icon">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
                  stroke-linecap="round" stroke-linejoin="round">
                  <rect x="4" y="10" width="16" height="10" rx="2.5" />
                  <path d="M8 10V7a4 4 0 018 0v3" />
                  <circle cx="12" cy="15" r="1.4" />
                </svg>
              </div>
              <div class="modal-titles">
                <h3>调试菜单</h3>
                <p>请输入密码以打开调试功能</p>
              </div>
            </div>
            <button class="close-btn" aria-label="关闭" @click="closePasswordDialog">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
            </button>
          </header>

          <div class="modal-body">
            <input
              ref="passwordInputRef"
              v-model="password"
              class="pwd-input"
              :class="{ invalid: !!errorText }"
              type="password"
              placeholder="请输入密码"
              autocomplete="off"
              @keydown.enter.prevent="submitPassword"
              @input="errorText = ''"
            />
            <p class="pwd-error">{{ errorText || ' ' }}</p>
          </div>

          <footer class="modal-actions">
            <button @click="closePasswordDialog">取消</button>
            <button class="accent" @click="submitPassword">确定</button>
          </footer>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.debug-menu {
  position: relative;
}

.tool-btn {
  gap: 6px;
  height: 30px;
  padding: 0 12px;
  font-size: 12.5px;
}

.tool-btn.active {
  border-color: var(--accent);
  color: var(--accent-dim);
  background: var(--accent-glow, #eff4ff);
}

.tool-btn svg {
  width: 13px;
  height: 13px;
  color: var(--text-tertiary);
}

.tool-btn:hover svg {
  color: var(--accent-dim);
}

.tool-btn .caret {
  width: 11px;
  height: 11px;
  margin-left: 1px;
  transition: transform 0.16s ease;
}

.tool-btn .caret.open {
  transform: rotate(180deg);
}

/* 下拉面板 */
.menu-panel {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  z-index: 200;
  min-width: 168px;
  padding: 6px;
  background: #fff;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  box-shadow: var(--shadow-md);
  animation: menu-in 0.14s ease-out;
}

@keyframes menu-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 34px;
  padding: 0 10px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  box-shadow: none;
  font-size: 12.5px;
  font-weight: 500;
  color: var(--text-primary);
  justify-content: flex-start;
}

.menu-item:hover {
  background: var(--bg-hover);
  color: var(--accent-dim);
  border-color: transparent;
}

.menu-item.active {
  background: var(--accent-soft);
  color: var(--accent-dim);
}

.menu-item svg {
  width: 14px;
  height: 14px;
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.menu-item:hover svg,
.menu-item.active svg {
  color: var(--accent-dim);
}

.menu-dot {
  margin-left: auto;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--accent);
}

/* 密码弹窗 */
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

.modal-body {
  padding: 16px 18px 8px;
}

.pwd-input {
  width: 100%;
  height: 36px;
  padding: 0 12px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: 13px;
  color: var(--text-primary);
  background: #fff;
  letter-spacing: 0.08em;
}

.pwd-input::placeholder {
  letter-spacing: 0;
  color: var(--text-tertiary);
}

.pwd-input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-glow);
}

.pwd-input.invalid {
  border-color: var(--error);
  box-shadow: 0 0 0 3px var(--error-soft);
}

.pwd-error {
  margin-top: 6px;
  min-height: 15px;
  font-size: 11.5px;
  color: var(--error);
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
