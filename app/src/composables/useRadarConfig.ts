import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { RadarConfig } from "../types";

/**
 * 全局雷达配置：只从后端读取一次，Layout 和激光器面板共享。
 * config.toml 修改后需要重启上位机生效。
 */
const displayName = ref("雷达");
const laserType = ref("laser_connet");
const loaded = ref(false);
let loading: Promise<void> | null = null;

async function loadRadarConfig(): Promise<void> {
    if (loaded.value) return;
    if (loading) return loading;

    loading = (async () => {
        try {
            const cfg = await invoke<RadarConfig>("get_radar_config");
            displayName.value = (cfg.display_name || "").trim() || "雷达";
            laserType.value = (cfg.laser_type || "").trim();
        } catch (e) {
            // 老版本后端没有 get_radar_config 时保持默认值
            console.error("load radar config failed:", e);
        } finally {
            loaded.value = true;
            loading = null;
        }
    })();

    return loading;
}

export function useRadarConfig() {
    return {
        displayName,
        laserType,
        loaded,
        load: loadRadarConfig,
    };
}
