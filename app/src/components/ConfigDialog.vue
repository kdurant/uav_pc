<script setup lang="ts">
import { reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, SaveConfigResult } from "../types";
import { useToast } from "../composables/useToast";
import { useRadarConfig } from "../composables/useRadarConfig";
import { useCaptureConfig } from "../composables/useCaptureConfig";

/**
 * 配置文件编辑界面：直接读写可执行文件目录下的 config.toml。
 * 保存后立即同步到内存并生效；仅本机 IP / 端口需要重启上位机重新绑定。
 */
const props = defineProps<{
    visible: boolean;
}>();

const emit = defineEmits<{
    (e: "update:visible", value: boolean): void;
}>();

const toast = useToast();
const { reload: reloadRadarConfig } = useRadarConfig();
const { reload: reloadCaptureConfig } = useCaptureConfig();

/** 可选激光器类型（与后端 config.rs 校验、control.rs 协议分派保持一致） */
const LASER_TYPES = [
    { value: "nanjing_5k", label: "南京中科神光 5kHz（nanjing_5k）" },
    { value: "nanjing_20k", label: "南京中科神光 20kHz（nanjing_20k，协议待确认）" },
    { value: "connet_20k", label: "上海翰宇 20kHz（connet_20k）" },
    { value: "keopsys_20k", label: "进口 Keopsys 20kHz（keopsys_20k，协议待确认）" },
];

/** 采集参数：启动时会填入采集面板；pin_threshold 暂未在界面使用 */
const CAPTURE_FIELDS: { key: keyof AppConfig["capture"]; label: string }[] = [
    { key: "preview_coe", label: "预览系数 preview_coe" },
    { key: "ref_channal", label: "参考通道 ref_channal（0~3）" },
    { key: "save_channal", label: "保存通道掩码 save_channal（0~255）" },
    { key: "wave_len", label: "波形长度 wave_len" },
    { key: "first_pos", label: "第一段起点 first_pos" },
    { key: "first_len", label: "第一段长度 first_len" },
    { key: "second_pos", label: "第二段起点 second_pos" },
    { key: "second_len", label: "第二段长度 second_len" },
    { key: "sum_value", label: "和阈值 sum_value" },
    { key: "max_value", label: "值阈值 max_value" },
    { key: "pin_threshold", label: "PIN 阈值 pin_threshold（暂未使用）" },
];

/** 空配置：加载失败时保证表单结构完整，避免模板访问 undefined */
function emptyConfig(): AppConfig {
    return {
        basic: {
            local_ip: "",
            local_port: 12345,
            remote_port: 6666,
            broadcast_search: true,
            device_ip: "192.168.1.10",
        },
        radar: { display_name: "", laser_type: "connet_20k", laser_info: "" },
        capture: {
            preview_coe: 1000,
            ref_channal: 1,
            save_channal: 255,
            wave_len: 6000,
            first_pos: 100,
            first_len: 200,
            second_pos: 400,
            second_len: 400,
            sum_value: 0,
            max_value: 0,
            pin_threshold: 1000,
        },
        gps: { dir: "/run/media/mmcblk1p1/" },
        ssd: { chunk_mb: 1 },
        temperature: { save_points: 180, display_points: 180 },
        laser: { current_ma: 4300, freq_hz: 20000 },
        motor: { speed_rpm: 100 },
        gain: { pmt0: 0, pmt1: 0, pmt2: 0, apd: 270 },
    };
}

const form = reactive<AppConfig>(emptyConfig());
const configPath = ref("");
const loading = ref(false);
const saving = ref(false);
const loadError = ref("");
const formError = ref("");

/** 深拷贝，避免表单直接修改后端返回的对象 */
function cloneConfig(cfg: AppConfig): AppConfig {
    return JSON.parse(JSON.stringify(cfg)) as AppConfig;
}

/** 读取最新配置与配置文件路径（每次打开都重新读取，避免展示过期值） */
async function loadConfig() {
    loading.value = true;
    loadError.value = "";
    formError.value = "";
    try {
        const [cfg, path] = await Promise.all([
            invoke<AppConfig>("get_app_config"),
            invoke<string>("get_config_path"),
        ]);
        Object.assign(form, cloneConfig(cfg));
        configPath.value = path;
    } catch (e) {
        loadError.value = `读取配置失败：${String(e)}`;
    } finally {
        loading.value = false;
    }
}

/** 简单 IPv4 校验（后端会再做一次权威校验） */
function isValidIp(ip: string): boolean {
    const parts = ip.trim().split(".");
    return (
        parts.length === 4 &&
        parts.every((p) => /^\d{1,3}$/.test(p) && Number(p) <= 255)
    );
}

function isInt(value: unknown): value is number {
    return typeof value === "number" && Number.isInteger(value);
}

/** 保存前校验，返回空字符串表示通过 */
function validateConfig(cfg: AppConfig): string {
    if (!isValidIp(cfg.basic.local_ip)) return "本机 IP 格式不正确";
    if (!isInt(cfg.basic.local_port) || cfg.basic.local_port < 1 || cfg.basic.local_port > 65535) {
        return "本机端口必须在 1~65535 之间";
    }
    if (!isInt(cfg.basic.remote_port) || cfg.basic.remote_port < 1 || cfg.basic.remote_port > 65535) {
        return "设备端口必须在 1~65535 之间";
    }
    if (!cfg.basic.broadcast_search && !isValidIp(cfg.basic.device_ip)) {
        return "关闭广播搜索时，必须填写合法的设备 IP";
    }
    if (!LASER_TYPES.some((t) => t.value === cfg.radar.laser_type)) {
        return "请选择支持的激光器类型";
    }

    if (!isInt(cfg.capture.ref_channal) || cfg.capture.ref_channal < 0 || cfg.capture.ref_channal > 3) {
        return "参考通道必须在 0~3 之间";
    }
    if (!isInt(cfg.capture.save_channal) || cfg.capture.save_channal < 0 || cfg.capture.save_channal > 255) {
        return "保存通道掩码必须在 0~255 之间";
    }
    if (!isInt(cfg.capture.wave_len) || cfg.capture.wave_len <= 0) {
        return "波形长度必须大于 0";
    }
    for (const field of CAPTURE_FIELDS) {
        if (!isInt(cfg.capture[field.key]) || cfg.capture[field.key] < 0) {
            return `${field.label} 必须是非负整数`;
        }
    }

    if (!cfg.gps.dir.trim()) return "GPS 目录不能为空";

    if (![1, 2, 4].includes(cfg.ssd.chunk_mb)) {
        return "SSD 分块大小只能是 1、2 或 4 MB";
    }

    if (!isInt(cfg.temperature.save_points) || cfg.temperature.save_points < 1 || cfg.temperature.save_points > 100000) {
        return "温度历史保存点数必须在 1~100000 之间";
    }
    if (!isInt(cfg.temperature.display_points) || cfg.temperature.display_points < 1 || cfg.temperature.display_points > 100000) {
        return "温度曲线显示点数必须在 1~100000 之间";
    }

    return "";
}

/** 保存：校验通过后写入 config.toml，并同步到内存立即生效 */
async function saveConfig() {
    formError.value = validateConfig(form);
    if (formError.value) return;

    const payload = cloneConfig(form);
    // 去掉用户误输入的首尾空格
    payload.basic.local_ip = payload.basic.local_ip.trim();
    payload.basic.device_ip = payload.basic.device_ip.trim();
    payload.radar.display_name = payload.radar.display_name.trim();
    payload.radar.laser_type = payload.radar.laser_type.trim();
    payload.gps.dir = payload.gps.dir.trim();

    saving.value = true;
    try {
        const result = await invoke<SaveConfigResult>("save_app_config", { config: payload });
        // 同步刷新前端缓存（显示名称、激光器类型、采集参数等）
        await Promise.all([reloadRadarConfig(), reloadCaptureConfig()]);
        if (result.restart_required) {
            toast.success("配置已保存", "其余配置已生效；本机 IP / 端口需重启上位机后生效");
        } else {
            toast.success("配置已保存", "配置已立即生效");
        }
        close();
    } catch (e) {
        formError.value = String(e);
    } finally {
        saving.value = false;
    }
}

function close() {
    if (saving.value) return;
    emit("update:visible", false);
}

watch(
    () => props.visible,
    (visible) => {
        if (visible) loadConfig();
    }
);
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
                <circle cx="12" cy="12" r="3.2" />
                <path d="M19.4 15a1.7 1.7 0 00.34 1.87l.06.06a2 2 0 11-2.83 2.83l-.06-.06A1.7 1.7 0 0015 19.4a1.7 1.7 0 00-1 .6 1.7 1.7 0 00-.4 1.1V21a2 2 0 11-4 0v-.09A1.7 1.7 0 009 19.4a1.7 1.7 0 00-1.87.34l-.06.06a2 2 0 11-2.83-2.83l.06-.06A1.7 1.7 0 004.6 15a1.7 1.7 0 00-.6-1 1.7 1.7 0 00-1.1-.4H2.9a2 2 0 110-4h.09A1.7 1.7 0 004.6 9a1.7 1.7 0 00-.34-1.87l-.06-.06a2 2 0 112.83-2.83l.06.06A1.7 1.7 0 009 4.6a1.7 1.7 0 001-.6 1.7 1.7 0 00.4-1.1V2.9a2 2 0 114 0v.09A1.7 1.7 0 0015 4.6a1.7 1.7 0 001.87-.34l.06-.06a2 2 0 112.83 2.83l-.06.06A1.7 1.7 0 0019.4 9a1.7 1.7 0 00.6 1 1.7 1.7 0 001.1.4h.09a2 2 0 110 4h-.09a1.7 1.7 0 00-1.1.4z" />
              </svg>
            </div>
            <div class="modal-titles">
              <h3>配置设置</h3>
              <p>保存后立即生效，本机 IP / 端口除外</p>
            </div>
          </div>
          <button class="close-btn" aria-label="关闭" @click="close">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
              stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </header>

        <div class="modal-body">
          <p v-if="loading" class="state-text">正在读取配置...</p>
          <p v-else-if="loadError" class="state-text error">{{ loadError }}</p>

          <template v-else>
            <!-- 网络 -->
            <section class="cfg-section">
              <h4 class="cfg-title">网络</h4>
              <div class="cfg-grid">
                <label class="cfg-field restart-field">
                  <span>本机 IP <i class="restart-tag">需重启</i></span>
                  <input v-model="form.basic.local_ip" class="cfg-input" type="text"
                    placeholder="192.168.93.44" />
                </label>
                <label class="cfg-field restart-field">
                  <span>本机端口 <i class="restart-tag">需重启</i></span>
                  <input v-model.number="form.basic.local_port" class="cfg-input" type="number"
                    min="1" max="65535" />
                </label>
                <label class="cfg-field">
                  <span>设备端口</span>
                  <input v-model.number="form.basic.remote_port" class="cfg-input" type="number"
                    min="1" max="65535" />
                </label>
                <label class="cfg-field cfg-check">
                  <input v-model="form.basic.broadcast_search" type="checkbox" />
                  <span>通过广播搜索设备</span>
                </label>
                <label class="cfg-field" :class="{ disabled: form.basic.broadcast_search }">
                  <span>固定设备 IP</span>
                  <input v-model="form.basic.device_ip" class="cfg-input" type="text"
                    placeholder="关闭广播搜索时必填" :disabled="form.basic.broadcast_search" />
                </label>
              </div>
              <p class="cfg-hint">
                橙色背景的参数（本机 IP / 端口）修改后需重启上位机才能重新绑定网络，其余参数保存后立即生效。
              </p>
            </section>

            <!-- 设备信息 -->
            <section class="cfg-section">
              <h4 class="cfg-title">设备信息</h4>
              <div class="cfg-grid">
                <label class="cfg-field">
                  <span>显示名称</span>
                  <input v-model="form.radar.display_name" class="cfg-input" type="text"
                    placeholder="雷达" />
                </label>
                <label class="cfg-field">
                  <span>激光器类型</span>
                  <select v-model="form.radar.laser_type" class="cfg-input">
                    <option v-for="t in LASER_TYPES" :key="t.value" :value="t.value">
                      {{ t.label }}
                    </option>
                  </select>
                </label>
                <label class="cfg-field span-2">
                  <span>激光器信息</span>
                  <input v-model="form.radar.laser_info" class="cfg-input" type="text"
                    placeholder="仅用于界面展示，可留空" />
                </label>
              </div>
            </section>

            <!-- 采集参数 -->
            <section class="cfg-section">
              <h4 class="cfg-title">采集参数</h4>
              <p class="cfg-hint">
                采集面板会立即使用这里的参数刷新输入框（pin_threshold 暂未使用），不会自动下发设备。
              </p>
              <div class="cfg-grid">
                <label v-for="field in CAPTURE_FIELDS" :key="field.key" class="cfg-field">
                  <span>{{ field.label }}</span>
                  <input v-model.number="form.capture[field.key]" class="cfg-input" type="number"
                    min="0" />
                </label>
              </div>
            </section>

            <!-- 存储与历史 -->
            <section class="cfg-section">
              <h4 class="cfg-title">存储与历史</h4>
              <div class="cfg-grid">
                <label class="cfg-field span-2">
                  <span>GPS 数据目录（设备端）</span>
                  <input v-model="form.gps.dir" class="cfg-input" type="text"
                    placeholder="/run/media/mmcblk1p1/" />
                </label>
                <label class="cfg-field">
                  <span>SSD 下载分块</span>
                  <select v-model.number="form.ssd.chunk_mb" class="cfg-input">
                    <option :value="1">1 MB</option>
                    <option :value="2">2 MB</option>
                    <option :value="4">4 MB</option>
                  </select>
                </label>
                <div class="cfg-field"></div>
                <label class="cfg-field">
                  <span>温度历史保存点数</span>
                  <input v-model.number="form.temperature.save_points" class="cfg-input" type="number"
                    min="1" max="100000" />
                </label>
                <label class="cfg-field">
                  <span>温度曲线显示点数</span>
                  <input v-model.number="form.temperature.display_points" class="cfg-input" type="number"
                    min="1" max="100000" />
                </label>
              </div>
            </section>

            <p v-if="formError" class="form-error">{{ formError }}</p>
          </template>
        </div>

        <footer class="modal-actions">
          <span class="cfg-path" :title="configPath">{{ configPath || "config.toml" }}</span>
          <button :disabled="saving" @click="close">取消</button>
          <button class="accent" :disabled="loading || saving || !!loadError" @click="saveConfig">
            {{ saving ? "保存中..." : "保存" }}
          </button>
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
  width: 680px;
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
  padding: 14px 18px;
}

.state-text {
  padding: 12px 0;
  font-size: 12.5px;
  color: var(--text-tertiary);
}

.state-text.error {
  color: var(--error);
}

/* 配置分组 */
.cfg-section {
  padding-bottom: 14px;
  margin-bottom: 14px;
  border-bottom: 1px dashed var(--border-color);
}

.cfg-section:last-of-type {
  margin-bottom: 6px;
  border-bottom: none;
}

.cfg-title {
  margin-bottom: 10px;
  font-size: 12.5px;
  font-weight: 650;
  color: var(--text-secondary);
  letter-spacing: 0.02em;
}

.cfg-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px 14px;
}

.cfg-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11.5px;
  color: var(--text-tertiary);
  min-width: 0;
}

.cfg-field.span-2 {
  grid-column: span 2;
}

.cfg-field.disabled {
  opacity: 0.55;
}

/* 需要重启上位机才能生效的参数，用橙色背景突出显示 */
.cfg-field.restart-field {
  padding: 6px 8px;
  border: 1px solid #fde3b8;
  border-radius: var(--radius-sm);
  background: var(--warning-soft);
}

.cfg-field.restart-field > span {
  color: var(--warning);
  font-weight: 600;
}

.restart-tag {
  margin-left: 4px;
  padding: 0 5px;
  border-radius: 999px;
  background: #f7d9a8;
  color: #8a5d0a;
  font-size: 9.5px;
  font-style: normal;
  font-weight: 600;
}

.cfg-check {
  flex-direction: row;
  align-items: center;
  gap: 8px;
  height: 30px;
  font-size: 12px;
  color: var(--text-secondary);
}

.cfg-check input {
  width: 15px;
  height: 15px;
  accent-color: var(--accent);
}

.cfg-input {
  height: 30px;
  padding: 0 9px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: 12.5px;
  color: var(--text-primary);
  background: #fff;
  min-width: 0;
}

.cfg-input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-glow);
}

.cfg-input:disabled {
  background: var(--bg-tertiary);
  color: var(--text-tertiary);
  cursor: not-allowed;
}

.cfg-hint {
  margin-top: 8px;
  font-size: 11px;
  color: var(--text-tertiary);
}

.form-error {
  padding: 8px 10px;
  border: 1px solid #f3c1c1;
  border-radius: var(--radius-sm);
  background: var(--error-soft);
  font-size: 12px;
  color: var(--error);
}

/* 底部操作栏 */
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

.cfg-path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-tertiary);
}
</style>
