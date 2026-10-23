import { ref, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { SystemStatus } from "../types";

export function useSystemStatus() {
    const status = ref<SystemStatus | null>(null);
    const lastUpdate = ref<number>(0);
    const isActive = ref(false);
    /** 设备运行时间（uptime）连续多次采样未增长 */
    const stale = ref(false);

    let unlistenFn: (() => void) | null = null;
    let watchdogTimer: number | null = null;

    /** uptime 采样周期（ms） */
    const STATUS_SAMPLE_MS = 1000;
    /** uptime 连续多少次未增长判定为中断 */
    const STALE_SAMPLE_COUNT = 3;

    /** 上次观察到的 uptime 及连续未增长次数 */
    let lastUptime: number | null = null;
    let unchangedSamples = 0;

    function resetUptimeBaseline() {
        lastUptime = null;
        unchangedSamples = 0;
    }

    /**
     * 每秒采样一次设备运行时间：
     * - 增加：设备状态正常，清除中断标记
     * - 相同：累计未增长次数，达到 3 次判定中断
     * - 回退：设备重启，重置基准并视为正常
     * - 从未收到状态：同样按未增长累计
     */
    function startWatchdog() {
        if (watchdogTimer !== null) return;
        watchdogTimer = window.setInterval(() => {
            if (!isActive.value) return;

            const uptime = status.value?.uptime;
            if (uptime === undefined) {
                unchangedSamples++;
                if (unchangedSamples >= STALE_SAMPLE_COUNT) stale.value = true;
                return;
            }

            if (lastUptime === null) {
                lastUptime = uptime;
                unchangedSamples = 0;
                stale.value = false;
                return;
            }

            if (uptime > lastUptime) {
                lastUptime = uptime;
                unchangedSamples = 0;
                stale.value = false;
            } else if (uptime === lastUptime) {
                unchangedSamples++;
                if (unchangedSamples >= STALE_SAMPLE_COUNT) stale.value = true;
            } else {
                // 设备重启后 uptime 归零/回退，按重新开始处理
                lastUptime = uptime;
                unchangedSamples = 0;
                stale.value = false;
            }
        }, STATUS_SAMPLE_MS);
    }

    function stopWatchdog() {
        if (watchdogTimer !== null) {
            clearInterval(watchdogTimer);
            watchdogTimer = null;
        }
    }

    async function start() {
        // 重复调用（如重新连接同一设备）时重置计时与 uptime 基准，
        // 避免沿用上一次的中断状态
        if (isActive.value) {
            lastUpdate.value = Date.now();
            resetUptimeBaseline();
            stale.value = false;
            return;
        }
        isActive.value = true;

        unlistenFn = await listen<SystemStatus>("sys-status-update", (event) => {
            status.value = event.payload;
            lastUpdate.value = Date.now();
        });

        // 启用后开始采样，由 uptime 是否增长判定连接状态
        resetUptimeBaseline();
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
        resetUptimeBaseline();
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
