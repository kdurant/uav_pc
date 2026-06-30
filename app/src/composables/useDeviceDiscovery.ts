import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ref, onUnmounted } from "vue";
import type { DiscoveredDevice } from "../types";

export function useDeviceDiscovery() {
    const devices = ref<DiscoveredDevice[]>([]);
    const isSearching = ref(false);
    const selectedDevice = ref<DiscoveredDevice | null>(null);

    async function searchDevices() {
        isSearching.value = true;
        devices.value = [];
        try {
            const result = await invoke<DiscoveredDevice[]>("discover_devices");
            devices.value = result;
        } catch (err) {
            console.error("Device discovery failed:", err);
        } finally {
            isSearching.value = false;
        }
    }

    function selectDevice(device: DiscoveredDevice) {
        selectedDevice.value = device;
    }

    function clearSelection() {
        selectedDevice.value = null;
    }

    // 监听后端推送的设备发现事件
    let unlistenFn: (() => void) | null = null;
    listen<DiscoveredDevice[]>("devices-found", (event) => {
        devices.value = event.payload;
    }).then((fn) => { unlistenFn = fn; });

    onUnmounted(() => {
        if (unlistenFn) unlistenFn();
    });

    return {
        devices,
        isSearching,
        selectedDevice,
        searchDevices,
        selectDevice,
        clearSelection,
    };
}
