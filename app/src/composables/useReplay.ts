import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { decodePreviewFrame } from "../utils/previewCodec";
import { frameToSeries } from "../utils/previewSeries";
import { useToast } from "./useToast";
import type { ReplayGps, ReplayInfo } from "../types";

/**
 * 离线回放全局状态（单例）。
 *
 * 回放与实时预览共用中间的「数据预览」图表，二者互斥：
 * 进入回放模式后实时帧不再上屏（见 Layout 中的 pushPreviewFrame 守卫），
 * 帧数据统一通过 `setSink` 注册的出口写进同一个 ChartView。
 */

/** 图表数据出口：series 为 null 表示清空画布 */
type FrameSink = (series: number[][] | null) => void;

const active = ref(false);
const info = ref<ReplayInfo | null>(null);
const frameIndex = ref(0);
const intervalMs = ref(100);
const playing = ref(false);
const busy = ref(false);
/** 当前回放帧的 GPS 信息，供设备状态面板显示 */
const gps = ref<ReplayGps | null>(null);

const toast = useToast();

let sink: FrameSink | null = null;
/** 请求序号：快速拖动进度条时丢弃过期响应，避免画面回跳 */
let requestSeq = 0;
let playTimer: number | null = null;

const frameCount = computed(() => info.value?.frame_count ?? 0);
const frameCountText = computed(() => frameCount.value.toLocaleString("zh-CN"));

const fileName = computed(() => {
    const path = info.value?.path;
    if (!path) return "";
    const parts = path.split(/[\\/]/);
    return parts[parts.length - 1] || path;
});

const progress = computed(() => {
    if (frameCount.value <= 1) return 0;
    return (frameIndex.value / (frameCount.value - 1)) * 100;
});

function clampInterval(): number {
    const v = Math.round(Number(intervalMs.value));
    if (!Number.isFinite(v)) return 100;
    return Math.min(2000, Math.max(10, v));
}

function setSink(fn: FrameSink | null) {
    sink = fn;
}

function emitSeries(series: number[][] | null) {
    sink?.(series);
}

/** 进入回放模式（尚未选择文件时图表为空） */
function enter() {
    if (active.value) return;
    active.value = true;
    gps.value = null;
    emitSeries(null);
}

/** 退出回放模式：停止播放、释放文件句柄、清空回放状态 */
function exit() {
    if (!active.value) return;
    stopPlay();
    active.value = false;
    gps.value = null;
    info.value = null;
    frameIndex.value = 0;
    emitSeries(null);
    void invoke("close_replay_file").catch((e) => console.error("close_replay_file failed:", e));
}

async function chooseFile() {
    const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "采集数据", extensions: ["bin", "dat"] }],
    });
    if (!path || Array.isArray(path)) return;
    await loadFile(path);
}

async function loadFile(path: string) {
    stopPlay();
    busy.value = true;
    try {
        const result = await invoke<ReplayInfo>("open_replay_file", { path });
        info.value = result;
        frameIndex.value = 0;
        await showFrame(0);
        toast.success("回放文件已加载", `共 ${result.frame_count.toLocaleString("zh-CN")} 帧`);
    } catch (e) {
        console.error("open_replay_file failed:", e);
        info.value = null;
        emitSeries(null);
        toast.error("打开回放文件失败", String(e));
    } finally {
        busy.value = false;
    }
}

async function showFrame(index: number) {
    if (!info.value) return;
    const total = info.value.frame_count;
    const target = Math.min(Math.max(0, Math.trunc(index)), total - 1);
    frameIndex.value = target;

    const seq = ++requestSeq;
    try {
        const payload = await invoke<ArrayBuffer>("read_replay_frame", { index: target });
        if (seq !== requestSeq) return;
        const frame = decodePreviewFrame(new Uint8Array(payload));
        if (!frame) return;

        const series = frameToSeries(frame);
        emitSeries(series);

        // 空帧也要更新 GPS，避免状态面板残留上一帧信息
        gps.value = {
            frameIndex: target,
            frameCount: total,
            gpsWeek: frame.gps_week,
            gpsSecond: frame.gps_second,
            subTime: frame.sub_time,
            latitude: frame.latitude,
            longitude: frame.longitude,
            altitude: frame.altitude,
            azimuth: frame.azimuth,
            pitch: frame.pitch,
            roll: frame.roll,
        };
    } catch (e) {
        console.error(`read_replay_frame(${target}) failed:`, e);
    }
}

function step(delta: number) {
    if (!info.value) return;
    stopPlay();
    void showFrame(frameIndex.value + delta);
}

function scheduleNext() {
    if (playTimer !== null) window.clearTimeout(playTimer);
    playTimer = window.setTimeout(async () => {
        playTimer = null;
        if (!playing.value || !info.value) return;
        if (frameIndex.value >= frameCount.value - 1) {
            playing.value = false;
            return;
        }
        await showFrame(frameIndex.value + 1);
        if (playing.value) scheduleNext();
    }, clampInterval());
}

function startPlay() {
    if (!info.value) return;
    // 已在末帧时从头播放
    if (frameIndex.value >= frameCount.value - 1) void showFrame(0);
    playing.value = true;
    scheduleNext();
}

function stopPlay() {
    playing.value = false;
    if (playTimer !== null) {
        window.clearTimeout(playTimer);
        playTimer = null;
    }
}

function togglePlay() {
    if (!info.value) return;
    if (playing.value) stopPlay();
    else startPlay();
}

/** 进度条拖动/点击 */
function seekTo(value: number) {
    stopPlay();
    void showFrame(value);
}

/** 间隔改变后按新节拍重新排程 */
function reschedule() {
    if (playing.value) scheduleNext();
}

export function useReplay() {
    return {
        active,
        info,
        frameCount,
        frameCountText,
        fileName,
        frameIndex,
        intervalMs,
        playing,
        busy,
        gps,
        progress,
        setSink,
        enter,
        exit,
        chooseFile,
        loadFile,
        showFrame,
        step,
        togglePlay,
        stopPlay,
        seekTo,
        reschedule,
    };
}
