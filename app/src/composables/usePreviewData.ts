import { Channel, invoke } from "@tauri-apps/api/core";
import { onUnmounted } from "vue";
import type { PreviewFrame } from "../types";
import { decodePreviewFrame } from "../utils/previewCodec";
import { useToast } from "./useToast";

/**
 * 一帧预览数据：
 * series[ch][i] 对应绝对采样位置 i 的值（数组索引 = 采样位置，从 0 开始），NaN 表示无数据。
 * 数组长度为两段提取区间的最大终点，X 轴默认范围由前端在此基础上扩展。
 * frame 为解码后的完整预览帧，包含 GPS 周、细分时间、姿态等元数据。
 */
export interface PreviewFrameCallback {
    (series: number[][], frame: PreviewFrame): void;
}

export function usePreviewData(onFrame: PreviewFrameCallback) {
    const toast = useToast();
    // Tauri IPC Channel：Rust 端推送紧凑二进制，前端解码
    let channel: Channel<number[]> | null = null;
    let active = false;

    // 帧合并：事件到达频率高于屏幕刷新率时，只处理最新一帧，避免主线程做无意义的转换
    let pendingFrame: PreviewFrame | null = null;
    let rafId = 0;

    /** 把一帧原始数据转换为按采样位置对齐的序列 */
    function emitFrame(frame: PreviewFrame) {
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

        onFrame(series, frame);
    }

    /** 每帧最多处理一次，始终使用最新数据 */
    function flush() {
        rafId = 0;
        const frame = pendingFrame;
        pendingFrame = null;
        if (frame) emitFrame(frame);
    }

    function handleFrame(frame: PreviewFrame) {
        pendingFrame = frame;
        if (!rafId) rafId = requestAnimationFrame(flush);
    }

    async function start() {
        if (active) return;
        active = true;

        const ch = new Channel<number[]>();
        ch.onmessage = (payload) => {
            if (!active) return;
            const frame = decodePreviewFrame(new Uint8Array(payload));
            if (frame) handleFrame(frame);
        };
        channel = ch;

        try {
            await invoke("register_preview_channel", { channel: ch });
        } catch (e) {
            console.error("register preview channel failed:", e);
            active = false;
            channel = null;
            toast.error("预览通道注册失败", String(e));
        }
    }

    function stop() {
        active = false;
        pendingFrame = null;
        if (rafId) {
            cancelAnimationFrame(rafId);
            rafId = 0;
        }
        channel = null;
        void invoke("unregister_preview_channel").catch((e) =>
            console.error("unregister preview channel failed:", e)
        );
    }

    onUnmounted(() => stop());

    return { start, stop, isActive: () => active };
}
