import { ref, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { SystemStatus } from "../types";

export function useSystemStatus() {
    const status = ref<SystemStatus | null>(null);
    const lastUpdate = ref<number>(0);
    const isActive = ref(false);
    /** 超过 STATUS_TIMEOUT_MS 未收到设备状态上传 */
    const stale = ref(false);

    let unlistenFn: (() => void) | null = null;
    let watchdogTimer: number | null = null;

    /** 设备状态上传超时时间（ms） */
    const STATUS_TIMEOUT_MS = 3000;
    const WATCHDOG_INTERVAL_MS = 500;

    function startWatchdog() {
        if (watchdogTimer !== null) return;
        watchdogTimer = window.setInterval(() => {
            if (!isActive.value) return;
            stale.value = Date.now() - lastUpdate.value > STATUS_TIMEOUT_MS;
        }, WATCHDOG_INTERVAL_MS);
    }

    function stopWatchdog() {
        if (watchdogTimer !== null) {
            clearInterval(watchdogTimer);
            watchdogTimer = null;
        }
    }

    async function start() {
        if (isActive.value) return;
        isActive.value = true;

        unlistenFn = await listen<SystemStatus>("sys-status-update", (event) => {
            status.value = event.payload;
            lastUpdate.value = Date.now();
            stale.value = false;
        });

        // 启用后开始计时，3 秒内没有数据即判定为中断
        lastUpdate.value = Date.now();
        stale.value = false;
        startWatchdog();
    }

    function stop() {
        isActive.value = false;
        stopWatchdog();
        if (unlistenFn) {
            unlistenFn();
            unlistenFn = null;
        }
        status.value = null;
        stale.value = false;
        lastUpdate.value = 0;
    }

    onUnmounted(() => {
        stop();
    });

    /** 格式化秒数为 hh:mm:ss */
    function formatUptime(seconds: number): string {
        const h = Math.floor(seconds / 3600);
        const m = Math.floor((seconds % 3600) / 60);
        const s = seconds % 60;
        return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }

    /** 格式化硬盘容量：单位 GB，按 1GB = 1024MB（1024³ 字节）换算 */
    function formatDiskSize(sectors: number): string {
        const bytes = sectors * 512;
        return (bytes / 1024 ** 3).toFixed(2) + " GB";
    }

    return {
        status,
        lastUpdate,
        isActive,
        stale,
        start,
        stop,
        formatUptime,
        formatDiskSize,
    };
}
