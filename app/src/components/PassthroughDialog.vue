<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * 命令透传：把用户输入的字符原样下发给设备的串口。
 * 测试时需在设备端外部短接串口 RX/TX，发出的数据会被回环回来并显示在日志中。
 */
const props = defineProps<{
    visible: boolean;
    connected: boolean;
    deviceIp?: string;
}>();

const emit = defineEmits<{
    (e: "update:visible", value: boolean): void;
}>();

/** 支持透传的命令码、设备端串口以及测试时需要短接的引脚 */
const COMMANDS = [
    { value: 0x0200, port: "/dev/ttyS1", short: "J3.1 ↔ J3.3" },
    { value: 0x0203, port: "/dev/ttyS3", short: "J7.1 ↔ J7.3" },
    { value: 0x0eff, port: "/dev/ttyS3", short: "J7.1 ↔ J7.3" },
    { value: 0x0f00, port: "/dev/ttyS2", short: "J5.1 ↔ J5.3" },
];

type LogDir = "tx" | "rx" | "info";

interface LogEntry {
    id: number;
    time: string;
    dir: LogDir;
    cmd: string;
    text: string;
    hex: string;
}

const selectedCmd = ref(0x0200);
const inputText = ref("");
const timeoutMs = ref(600);
const sending = ref(false);
const entries = ref<LogEntry[]>([]);
const logRef = ref<HTMLDivElement | null>(null);
let entryId = 0;

const selectedPort = computed(
    () => COMMANDS.find((c) => c.value === selectedCmd.value)?.port ?? "-"
);
const selectedShort = computed(
    () => COMMANDS.find((c) => c.value === selectedCmd.value)?.short ?? "-"
);

function formatCmd(cmd: number): string {
    return `0x${cmd.toString(16).padStart(4, "0")}`;
}

function formatTime(date = new Date()): string {
    const p = (n: number, width = 2) => String(n).padStart(width, "0");
    return `${p(date.getHours())}:${p(date.getMinutes())}:${p(date.getSeconds())}.${p(date.getMilliseconds(), 3)}`;
}

function toHex(bytes: number[]): string {
    return bytes.map((b) => b.toString(16).padStart(2, "0")).join(" ");
}

function decodeText(bytes: number[]): string {
    try {
        return new TextDecoder("utf-8", { fatal: false }).decode(new Uint8Array(bytes));
    } catch {
        return "";
    }
}

function scrollToBottom() {
    nextTick(() => {
        const el = logRef.value;
        if (el) el.scrollTop = el.scrollHeight;
    });
}

function pushEntry(dir: LogDir, text: string, hex = "", cmd = formatCmd(selectedCmd.value)) {
    entries.value.push({
        id: ++entryId,
        time: formatTime(),
        dir,
        cmd,
        text,
        hex,
    });
    // 日志只保留最近 100 条，避免长时间测试占用内存
    if (entries.value.length > 100) entries.value.splice(0, entries.value.length - 100);
    scrollToBottom();
}

async function send() {
    if (sending.value) return;
    if (!props.connected || !props.deviceIp) {
        pushEntry("info", "设备未连接，无法发送");
        return;
    }
    const text = inputText.value;
    if (!text) {
        pushEntry("info", "请输入要发送的字符");
        return;
    }

    const bytes = Array.from(new TextEncoder().encode(text));
    pushEntry("tx", text, toHex(bytes));
    sending.value = true;
    try {
        const responses = await invoke<number[][]>("passthrough_command", {
            deviceIp: props.deviceIp,
            cmd: selectedCmd.value,
            data: bytes,
            timeoutMs: timeoutMs.value,
        });
        if (!responses.length) {
            pushEntry(
                "info",
                `等待 ${timeoutMs.value}ms 未收到回环数据，请检查设备端是否已短接 ${selectedShort.value}（${selectedPort.value} 的 RX/TX）`
            );
        } else {
            for (const data of responses) {
                pushEntry("rx", decodeText(data), toHex(data));
            }
        }
    } catch (e) {
        pushEntry("info", `透传失败：${String(e)}`);
    } finally {
        sending.value = false;
    }
}

function clearLog() {
    entries.value = [];
    entryId = 0;
}

function close() {
    if (sending.value) return;
    emit("update:visible", false);
}

function onKeydown(event: KeyboardEvent) {
    if (!props.visible) return;
    if (event.key === "Escape") close();
}

watch(
    () => props.visible,
    (visible) => {
        if (visible) scrollToBottom();
    }
);

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="props.visible" class="modal-overlay" @click.self="close">
      <div class="modal">
        <header class="modal-head">
          <div class="modal-title-wrap">
            <div class="modal-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
                stroke-linecap="round" stroke-linejoin="round">
                <rect x="3" y="4" width="18" height="16" rx="3" />
                <path d="M7 9l3 3-3 3" />
                <path d="M13 15h4" />
              </svg>
            </div>
            <div class="modal-titles">
              <h3>命令透传</h3>
              <p>字符原样下发到设备串口，回环数据在此显示</p>
            </div>
          </div>
          <button class="close-btn" aria-label="关闭" @click="close">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
              stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </header>

        <div class="modal-body">
          <label class="field">
            <span class="field-label">透传命令</span>
            <select v-model.number="selectedCmd" class="input" :disabled="sending">
              <option v-for="c in COMMANDS" :key="c.value" :value="c.value">
                {{ formatCmd(c.value) }} · {{ c.port }}
              </option>
            </select>
          </label>

          <div class="port-bar">
            <span class="port-label">当前串口</span>
            <span class="port-value">{{ selectedPort }}</span>
            <span class="port-short">短接 {{ selectedShort }}</span>
            <span class="port-cmd">{{ formatCmd(selectedCmd) }}</span>
          </div>

          <div class="send-row">
            <input
              v-model="inputText"
              class="input text-input"
              type="text"
              maxlength="256"
              placeholder="输入要发送的字符，例如：hello 或 1234"
              :disabled="sending"
              @keydown.enter.prevent="send"
            />
            <label class="timeout">
              等待
              <input
                v-model.number="timeoutMs"
                class="input timeout-input"
                type="number"
                min="100"
                max="5000"
                step="100"
                :disabled="sending"
              />
              ms
            </label>
            <button class="accent send-btn" :disabled="sending || !props.connected" @click="send">
              {{ sending ? "等待回环..." : "发送" }}
            </button>
          </div>

          <p v-if="!props.connected" class="hint warn">
            设备未连接，请先在主界面搜索并连接采集卡。
          </p>
          <p v-else class="hint">
            测试前请确认设备端已外部短接对应串口的 RX/TX；命令、串口与短接引脚对应关系：
            0x0200 → /dev/ttyS1（J3.1、J3.3）· 0x0203 → /dev/ttyS3（J7.1、J7.3）·
            0x0eff → /dev/ttyS3（J7.1、J7.3）· 0x0f00 → /dev/ttyS2（J5.1、J5.3）
          </p>

          <div class="log-head">
            <span>收发日志</span>
            <button class="clear-btn" :disabled="!entries.length" @click="clearLog">清空</button>
          </div>
          <div ref="logRef" class="log">
            <p v-if="!entries.length" class="log-empty">发送后，回环数据将显示在这里</p>
            <div v-for="entry in entries" :key="entry.id" class="log-row" :class="entry.dir">
              <span class="log-time">{{ entry.time }}</span>
              <span class="log-dir">
                {{ entry.dir === "tx" ? "TX" : entry.dir === "rx" ? "RX" : "--" }}
              </span>
              <span class="log-cmd">{{ entry.cmd }}</span>
              <div class="log-payload">
                <div v-if="entry.text" class="log-text">{{ entry.text }}</div>
                <div v-if="entry.hex" class="log-hex">{{ entry.hex }}</div>
              </div>
            </div>
          </div>
        </div>

        <footer class="modal-actions">
          <span class="foot-hint">数据以 UTF-8 编码发送，日志同时显示文本与十六进制</span>
          <button :disabled="sending" @click="close">关闭</button>
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
  display: flex;
  flex-direction: column;
  width: 720px;
  max-width: calc(100vw - 40px);
  max-height: calc(100vh - 80px);
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
  flex-shrink: 0;
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
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 14px 18px 16px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 10px;
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.input {
  height: 32px;
  padding: 0 10px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: 12.5px;
  color: var(--text-primary);
  background: #fff;
  min-width: 0;
}

.input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-glow);
}

.input:disabled {
  background: var(--bg-tertiary);
  color: var(--text-tertiary);
  cursor: not-allowed;
}

.port-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  margin-bottom: 10px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-inset);
  font-size: 12px;
}

.port-label {
  color: var(--text-tertiary);
}

.port-value {
  font-family: var(--font-mono);
  font-weight: 650;
  color: var(--accent-dim);
}

.port-cmd {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-tertiary);
}

.port-short {
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent-dim);
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 600;
  white-space: nowrap;
}

.send-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.text-input {
  flex: 1;
}

.timeout {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
  font-size: 11.5px;
  color: var(--text-secondary);
}

.timeout-input {
  width: 68px;
  text-align: right;
}

.send-btn {
  flex-shrink: 0;
  height: 32px;
  padding: 0 16px;
}

.hint {
  margin-top: 10px;
  font-size: 11px;
  line-height: 1.55;
  color: var(--text-tertiary);
}

.hint.warn {
  padding: 8px 10px;
  border: 1px solid #fde3b8;
  border-radius: var(--radius-sm);
  background: var(--warning-soft);
  color: var(--warning);
}

.log-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 14px 0 6px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
}

.clear-btn {
  height: 24px;
  padding: 0 10px;
  font-size: 11.5px;
}

.log {
  height: 220px;
  overflow-y: auto;
  padding: 6px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: #fbfcfe;
}

.log-empty {
  padding: 12px;
  font-size: 11.5px;
  color: var(--text-tertiary);
  text-align: center;
}

.log-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 5px 6px;
  border-radius: 6px;
  font-size: 11.5px;
}

.log-row + .log-row {
  margin-top: 2px;
}

.log-row.tx {
  background: rgba(37, 99, 235, 0.05);
}

.log-row.rx {
  background: rgba(18, 136, 74, 0.06);
}

.log-time {
  flex-shrink: 0;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-tertiary);
  padding-top: 1px;
}

.log-dir {
  flex-shrink: 0;
  width: 22px;
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
}

.log-row.tx .log-dir {
  color: var(--accent);
}

.log-row.rx .log-dir {
  color: var(--success);
}

.log-row.info .log-dir {
  color: var(--text-tertiary);
}

.log-cmd {
  flex-shrink: 0;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-tertiary);
  padding-top: 1px;
}

.log-payload {
  flex: 1;
  min-width: 0;
}

.log-text {
  color: var(--text-primary);
  white-space: pre-wrap;
  word-break: break-all;
}

.log-row.info .log-text {
  color: var(--warning);
}

.log-hex {
  margin-top: 1px;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-tertiary);
  word-break: break-all;
}

.modal-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 18px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-inset);
  flex-shrink: 0;
}

.foot-hint {
  flex: 1;
  min-width: 0;
  font-size: 10.5px;
  color: var(--text-tertiary);
}
</style>
