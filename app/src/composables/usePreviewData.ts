import { Channel, invoke } from "@tauri-apps/api/core";
import { onUnmounted } from "vue";
import type { PreviewFrame } from "../types";
import { decodePreviewFrame } from "../utils/previewCodec";
import { frameToSeries } from "../utils/previewSeries";
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
        const series = frameToSeries(frame);
        if (!series) return;
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
