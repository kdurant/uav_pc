import { listen } from "@tauri-apps/api/event";
import { onUnmounted } from "vue";
import type { PreviewFrame } from "../types";

export interface PreviewDataCallback {
    (data: number[][]): void;
}

export function usePreviewData(onData: PreviewDataCallback) {
    let unlistenFn: (() => void) | null = null;
    let active = false;

    async function start() {
        if (active) return;
        active = true;

        unlistenFn = await listen<PreviewFrame>("preview-data", (event) => {
            const frame = event.payload;
            const allData: number[][] = [];

            for (const ch of frame.channels) {
                // 合并第一段和第二段数据
                const combined = [...ch.seg0_data, ...ch.seg1_data];
                allData.push(combined);
            }

            // 所有通道数据对齐到最大长度
            const maxLen = Math.max(0, ...allData.map(a => a.length));
            if (maxLen === 0) return;

            // 输出每批数据（每次 push 4 通道各一个值）
            const batches: number[][] = [];
            for (let i = 0; i < maxLen; i++) {
                const batch: number[] = [];
                for (let ch = 0; ch < 4; ch++) {
                    batch.push(allData[ch]?.[i] ?? 0);
                }
                batches.push(batch);
            }

            if (batches.length > 0) {
                onData(batches);
            }
        });
    }

    function stop() {
        active = false;
        if (unlistenFn) {
            unlistenFn();
            unlistenFn = null;
        }
    }

    onUnmounted(() => stop());

    return { start, stop, isActive: () => active };
}
