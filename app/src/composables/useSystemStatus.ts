import { ref, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { SystemStatus } from "../types";

export function useSystemStatus() {
    const status = ref<SystemStatus | null>(null);
    const lastUpdate = ref<number>(0);
    const isActive = ref(false);

    let unlistenFn: (() => void) | null = null;

    async function start() {
        if (isActive.value) return;
        isActive.value = true;

        unlistenFn = await listen<SystemStatus>("sys-status-update", (event) => {
            status.value = event.payload;
            lastUpdate.value = Date.now();
        });
    }

    function stop() {
        isActive.value = false;
        if (unlistenFn) {
            unlistenFn();
            unlistenFn = null;
        }
        status.value = null;
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

    /** 格式化硬盘容量 */
    function formatDiskSize(sectors: number): string {
        const bytes = sectors * 512;
        if (bytes >= 1e12) return (bytes / 1e12).toFixed(2) + " TB";
        if (bytes >= 1e9) return (bytes / 1e9).toFixed(2) + " GB";
        if (bytes >= 1e6) return (bytes / 1e6).toFixed(1) + " MB";
        return bytes + " B";
    }

    return {
        status,
        lastUpdate,
        isActive,
        start,
        stop,
        formatUptime,
        formatDiskSize,
    };
}
