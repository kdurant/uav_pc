<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
import type { SsdDownloadState } from "../types";

interface DrawerFile {
    name: string;
    start?: number;
    end?: number;
    size?: number;
    /** Unix 时间戳（秒） */
    mtime?: number;
}

const props = defineProps<{
    visible: boolean;
    title: string;
    mode: "ssd" | "gps";
    loading: boolean;
    error: string;
    files: DrawerFile[];
    /** 当前下载状态；null 表示没有下载任务 */
    download?: SsdDownloadState | null;
    /** 是否允许下载（连接正常、未存储、无并发任务） */
    canDownload?: boolean;
    /** 下载失败/取消提示 */
    downloadError?: string;
    /** 下载完成提示 */
    downloadMessage?: string;
}>();

const emit = defineEmits<{
    (e: "close"): void;
    (e: "refresh"): void;
    (e: "download", file: DrawerFile): void;
    (e: "cancel-download"): void;
}>();

/** 字节数格式化为易读单位 */
function formatSize(bytes: number): string {
    if (bytes >= 1024 ** 3) return (bytes / 1024 ** 3).toFixed(2) + " GB";
    if (bytes >= 1024 ** 2) return (bytes / 1024 ** 2).toFixed(1) + " MB";
    if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
    return bytes + " B";
}

/** 下载速度格式化 */
function formatSpeed(bytesPerSec: number): string {
    if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "--";
    return `${(bytesPerSec / 1024 / 1024).toFixed(1)} MB/s`;
}

/** Unix 秒 → 本地时间字符串 */
function formatMtime(sec?: number): string {
    if (!sec) return "-";
    const d = new Date(sec * 1000);
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

const totalSize = computed(() =>
    props.files.reduce((sum, f) => sum + (f.size ?? 0), 0)
);

function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && props.visible) emit("close");
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
    <Teleport to="body">
        <Transition name="drawer">
            <div v-if="visible" class="drawer-layer" @click.self="emit('close')">
                <aside class="drawer" role="dialog" aria-modal="true" :aria-label="title">
                    <header class="drawer-head">
                        <div class="head-text">
                            <h3>{{ title }}</h3>
                            <p v-if="loading">正在检索文件...</p>
                            <p v-else>
                                共 {{ files.length }} 个文件
                                <template v-if="files.length">
                                    · 合计 {{ formatSize(totalSize) }}
                                </template>
                            </p>
                        </div>
                        <button class="close-btn" aria-label="关闭" @click="emit('close')">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                                stroke-linecap="round">
                                <path d="M6 6l12 12M18 6L6 18" />
                            </svg>
                        </button>
                    </header>

                    <div class="drawer-body">
                        <!-- 下载进度 -->
                        <div v-if="download" class="download-panel">
                            <div class="download-head">
                                <span class="download-name" :title="download.name">{{ download.name }}</span>
                                <span class="download-size">
                                    {{ formatSize(download.transferred) }} / {{ formatSize(download.total) }}
                                </span>
                            </div>
                            <div class="progress-track">
                                <div
                                    class="progress-fill"
                                    :style="{ width: Math.min(100, Math.max(0, download.percent)) + '%' }"
                                ></div>
                            </div>
                            <div class="download-foot">
                                <span>{{ download.percent.toFixed(1) }}%</span>
                                <span class="download-speed">
                                    {{ formatSpeed(download.speed_bps) }}
                                    <template v-if="download.chunk_mb"> · {{ download.chunk_mb }}MB/块</template>
                                </span>
                                <button
                                    class="row-btn"
                                    :disabled="download.cancelling"
                                    @click="emit('cancel-download')"
                                >
                                    {{ download.cancelling ? "取消中..." : "取消" }}
                                </button>
                            </div>
                        </div>
                        <p v-if="downloadError" class="download-msg error">{{ downloadError }}</p>
                        <p v-if="downloadMessage" class="download-msg success">{{ downloadMessage }}</p>

                        <!-- 加载中 -->
                        <div v-if="loading" class="state">
                            <span class="spinner"></span>
                            <p class="state-title">正在检索文件...</p>
                            <p class="state-desc">需要通过设备逐扇区读取目录，请稍候</p>
                        </div>

                        <!-- 错误 -->
                        <div v-else-if="error" class="state">
                            <div class="state-icon error-icon">!</div>
                            <p class="state-title">检索失败</p>
                            <p class="state-desc error-text">{{ error }}</p>
                            <button class="retry-btn" @click="emit('refresh')">重试</button>
                        </div>

                        <!-- 空 -->
                        <div v-else-if="!files.length" class="state">
                            <div class="state-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"
                                    stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V7z" />
                                </svg>
                            </div>
                            <p class="state-title">暂无文件</p>
                            <p class="state-desc">设备当前目录下没有可显示的文件</p>
                        </div>

                        <!-- 文件表格 -->
                        <div v-else class="table-wrap">
                            <table class="file-table">
                                <thead>
                                    <tr>
                                        <th class="col-name">文件名</th>
                                        <template v-if="mode === 'ssd'">
                                            <th class="col-num">起始地址</th>
                                            <th class="col-num">结束地址</th>
                                            <th class="col-num">大小</th>
                                        </template>
                                        <template v-else>
                                            <th class="col-num">大小</th>
                                            <th class="col-time">修改时间</th>
                                        </template>
                                        <th class="col-action">操作</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    <tr v-for="f in files" :key="f.name + '-' + (f.start ?? 0)">
                                        <td class="col-name" :title="f.name">{{ f.name }}</td>
                                        <template v-if="mode === 'ssd'">
                                            <td class="col-num">{{ f.start }}</td>
                                            <td class="col-num">{{ f.end }}</td>
                                            <td class="col-num">{{ formatSize(f.size ?? 0) }}</td>
                                        </template>
                                        <template v-else>
                                            <td class="col-num">{{ formatSize(f.size ?? 0) }}</td>
                                            <td class="col-time">{{ formatMtime(f.mtime) }}</td>
                                        </template>
                                        <td class="col-action">
                                            <button
                                                class="row-btn"
                                                :disabled="!canDownload || !!download || (mode === 'ssd' && (f.end ?? 0) <= (f.start ?? 0))"
                                                @click="emit('download', f)"
                                            >
                                                下载
                                            </button>
                                        </td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </div>

                    <footer class="drawer-foot">
                        <span class="unit">
                            <template v-if="mode === 'ssd'">地址单位：扇区（512 B）</template>
                            <template v-else>路径：/run/media/mmcblk1p1/（可在 config.toml [gps].dir 修改）</template>
                        </span>
                        <div class="foot-actions">
                            <button :disabled="loading || !!download" @click="emit('refresh')">重新检索</button>
                            <button class="accent" @click="emit('close')">关闭</button>
                        </div>
                    </footer>
                </aside>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.drawer-layer {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: flex;
    justify-content: flex-end;
    background: rgba(15, 23, 42, 0.3);
}

.drawer {
    display: flex;
    flex-direction: column;
    width: min(720px, 92vw);
    height: 100%;
    background: #fff;
    border-left: 1px solid #dde3ec;
    overflow: hidden;
}

.drawer-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 16px 18px 14px;
    border-bottom: 1px solid var(--border-color);
    flex-shrink: 0;
}

.head-text h3 {
    font-size: 15px;
    font-weight: 650;
    color: var(--text-primary);
    letter-spacing: 0.01em;
}

.head-text p {
    margin-top: 3px;
    font-size: 11.5px;
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
}

.close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
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

.drawer-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

/* 下载进度 */
.download-panel {
    flex-shrink: 0;
    margin: 12px 16px 0;
    padding: 10px 12px;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    background: var(--bg-inset);
}

.download-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
}

.download-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
    color: var(--text-primary);
}

.download-size {
    flex-shrink: 0;
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
    font-size: 11.5px;
}

.progress-track {
    height: 6px;
    margin: 8px 0 6px;
    background: #e4e9f2;
    border-radius: 999px;
    overflow: hidden;
}

.progress-fill {
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, #6ea0ff, var(--accent));
    transition: width 0.15s linear;
}

.download-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 11.5px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
}

.download-speed {
    color: var(--text-tertiary);
}

.download-msg {
    flex-shrink: 0;
    margin: 10px 16px 0;
    font-size: 12px;
    word-break: break-all;
}

.download-msg.error {
    color: #b42318;
}

.download-msg.success {
    color: #0e6b3c;
}

.file-table .col-action {
    width: 72px;
    text-align: right;
}

.file-table .col-time {
    width: 150px;
    color: var(--text-secondary);
}

.row-btn {
    padding: 3px 10px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    background: #fff;
    color: var(--accent);
    font-size: 11.5px;
    font-family: inherit;
    line-height: 1.5;
    cursor: pointer;
}

.row-btn:hover:not(:disabled) {
    background: var(--accent-soft);
    border-color: #bcd0f8;
}

.row-btn:disabled {
    background: var(--bg-inset);
    color: var(--text-tertiary);
    cursor: not-allowed;
}

/* 状态占位 */
.state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 40px 24px;
    text-align: center;
}

.state-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 46px;
    height: 46px;
    margin-bottom: 6px;
    border-radius: 13px;
    background: var(--bg-tertiary);
    color: var(--text-tertiary);
    font-size: 18px;
    font-weight: 700;
}

.state-icon svg {
    width: 23px;
    height: 23px;
}

.error-icon {
    background: var(--error-soft);
    color: var(--error);
}

.state-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-secondary);
}

.state-desc {
    max-width: 320px;
    font-size: 11.5px;
    line-height: 1.6;
    color: var(--text-tertiary);
}

.error-text {
    color: var(--error);
    word-break: break-all;
}

.retry-btn {
    margin-top: 10px;
}

.spinner {
    width: 26px;
    height: 26px;
    margin-bottom: 8px;
    border: 2.5px solid var(--border-color);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

/* 文件表格 */
.table-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px 16px 16px;
}

.file-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
}

.file-table th,
.file-table td {
    padding: 8px 10px;
    text-align: left;
    white-space: nowrap;
    border-bottom: 1px solid var(--border-color);
    color: var(--text-primary);
}

.file-table th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    font-size: 10.5px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
}

.file-table th:first-child {
    border-top-left-radius: var(--radius-xs);
}

.file-table th:last-child {
    border-top-right-radius: var(--radius-xs);
}

.file-table td {
    font-variant-numeric: tabular-nums;
    font-family: var(--font-mono);
}

.file-table .col-name {
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: inherit;
    font-weight: 500;
}

.file-table .col-num {
    text-align: right;
}

.file-table th.col-num {
    text-align: right;
}

.file-table tbody tr:last-child td {
    border-bottom: none;
}

.file-table tbody tr:hover {
    background: var(--bg-inset);
}

/* 底部 */
.drawer-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 16px;
    border-top: 1px solid var(--border-color);
    background: var(--bg-inset);
    flex-shrink: 0;
}

.unit {
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-tertiary);
}

.foot-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
}

/* 进出场：只做位移/透明度，不触发布局 */
.drawer-enter-active,
.drawer-leave-active {
    transition: opacity 0.18s ease;
}

.drawer-enter-active .drawer,
.drawer-leave-active .drawer {
    transition: transform 0.22s cubic-bezier(0.16, 1, 0.3, 1);
}

.drawer-enter-from,
.drawer-leave-to {
    opacity: 0;
}

.drawer-enter-from .drawer,
.drawer-leave-to .drawer {
    transform: translateX(28px);
}

@media (max-width: 720px) {
    .drawer {
        width: 100vw;
    }

    .drawer-foot {
        flex-direction: column;
        align-items: stretch;
    }

    .foot-actions {
        justify-content: flex-end;
    }
}
</style>
