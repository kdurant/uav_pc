import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { RadarConfig } from "../types";

/**
 * 全局雷达配置：从后端读取，Layout 和激光器面板共享。
 * 设置界面保存后会调用 reload() 立即刷新，无需重启上位机。
 */
const displayName = ref("雷达");
const laserType = ref("connet_20k");
const loaded = ref(false);
let loading: Promise<void> | null = null;

async function loadRadarConfig(force = false): Promise<void> {
    if (loaded.value && !force) return;
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
        /** 强制重新读取（设置保存后调用） */
        reload: () => loadRadarConfig(true),
    };
}
