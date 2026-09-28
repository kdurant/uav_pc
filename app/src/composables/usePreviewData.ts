import { listen } from "@tauri-apps/api/event";
import { onUnmounted } from "vue";
import type { PreviewFrame } from "../types";

/**
 * 一帧预览数据：
 * series[ch][i] 对应绝对采样位置 i 的值（数组索引 = 采样位置，从 0 开始），NaN 表示无数据。
 * 数组长度为两段提取区间的最大终点，X 轴默认范围由前端在此基础上扩展。
 */
export interface PreviewFrameCallback {
    (series: number[][]): void;
}

export function usePreviewData(onFrame: PreviewFrameCallback) {
    let unlistenFn: (() => void) | null = null;
    let active = false;

    async function start() {
        if (active) return;
        active = true;

        unlistenFn = await listen<PreviewFrame>("preview-data", (event) => {
            const frame = event.payload;
            if (!frame.channels.length) return;

            // 最大终点（第二段提取位置 + 第二段提取长度）
            let dataEnd = 0;
            for (const ch of frame.channels) {
                if (ch.seg0_data.length > 0) {
                    dataEnd = Math.max(dataEnd, ch.seg0_start + ch.seg0_data.length);
                }
                if (ch.seg1_data.length > 0) {
                    dataEnd = Math.max(dataEnd, ch.seg1_start + ch.seg1_data.length);
                }
            }
            if (dataEnd <= 0) return;

            // 数组索引 = 绝对采样位置：从 0 到 dataEnd，无数据处为 NaN
            const len = dataEnd;
            const series: number[][] = [];
            for (let c = 0; c < 4; c++) {
                const ch = frame.channels[c];
                const arr = new Array<number>(len).fill(NaN);
                if (ch) {
                    for (let i = 0; i < ch.seg0_data.length; i++) {
                        const pos = ch.seg0_start + i;
                        if (pos >= 0 && pos < len) arr[pos] = ch.seg0_data[i];
                    }
                    for (let i = 0; i < ch.seg1_data.length; i++) {
                        const pos = ch.seg1_start + i;
                        if (pos >= 0 && pos < len) arr[pos] = ch.seg1_data[i];
                    }
                }
                series.push(arr);
            }

            onFrame(series);
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
