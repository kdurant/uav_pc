import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "../types";

/**
 * 全局采集参数配置：从 config.toml 读取一次，供左侧采集面板和设置向导共享。
 * 仅用于初始化界面输入框，不会自动下发给设备；修改 config.toml 后需重启上位机生效。
 */
const capture = ref<AppConfig["capture"] | null>(null);
const loaded = ref(false);
let loading: Promise<void> | null = null;

async function loadCaptureConfig(): Promise<void> {
    if (loaded.value) return;
    if (loading) return loading;

    loading = (async () => {
        try {
            const cfg = await invoke<AppConfig>("get_app_config");
            capture.value = { ...cfg.capture };
        } catch (e) {
            // 后端不可用或读取失败时保持界面默认值
            console.error("load capture config failed:", e);
        } finally {
            loaded.value = true;
            loading = null;
        }
    })();

    return loading;
}

export function useCaptureConfig() {
    return {
        capture,
        loaded,
        load: loadCaptureConfig,
    };
}
