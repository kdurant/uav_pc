import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useToast } from "./useToast";
import { decodeCloud, type CloudData } from "../utils/cloudCodec";

/**
 * 点云 / DEM 全局状态（单例）。
 *
 * 与实时预览、离线回放共用中间的图表区域，三选一互斥：
 * 进入点云模式后，回放控制条与实时帧都不再显示。
 */
const active = ref(false);
const busy = ref(false);
/** 生成进度 0~100 */
const progress = ref(0);
const progressText = ref("");
const data = ref<CloudData | null>(null);

/** 生成参数 */
const startFrame = ref(0);
const frameCount = ref(20000);
const voxel = ref(0.3);
const demCell = ref(1.0);
const scanAngleDeg = ref(15);
const refraction = ref(true);
/** 垂直放大倍数 */
const zScale = ref(1);

export type CloudDisplay = "points" | "dem" | "both";
const display = ref<CloudDisplay>("both");

const toast = useToast();
let unlisten: (() => void) | null = null;

const pointCountText = computed(() =>
    (data.value?.pointCount ?? 0).toLocaleString("zh-CN")
);

async function ensureProgressListener() {
    if (unlisten) return;
    unlisten = await listen<{ processed: number; total: number; percent: number }>(
        "cloud-progress",
        (event) => {
            progress.value = event.payload.percent;
            progressText.value = event.payload.total
                ? `${event.payload.processed.toLocaleString("zh-CN")} / ${event.payload.total.toLocaleString("zh-CN")} 帧`
                : `${event.payload.processed.toLocaleString("zh-CN")} 帧`;
        }
    );
}

/** 选择数据文件（与回放互不影响） */
async function chooseFile() {
    const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "采集数据", extensions: ["bin", "dat"] }],
    });
    if (!path || Array.isArray(path)) return;
    currentPath.value = path;
}

async function build() {
    if (busy.value) return;
    busy.value = true;
    progress.value = 0;
    progressText.value = "准备中...";
    await ensureProgressListener();

    try {
        const payload = await invoke<ArrayBuffer>("build_point_cloud", {
            path: currentPath.value,
            params: {
                startFrame: startFrame.value,
                frameCount: frameCount.value,
                voxel: voxel.value,
                demCell: demCell.value,
                scanAngleDeg: scanAngleDeg.value,
                refraction: refraction.value,
            },
        });
        const decoded = decodeCloud(new Uint8Array(payload));
        if (!decoded) throw new Error("点云数据解析失败");
        data.value = decoded;
        progress.value = 100;
        toast.success(
            "点云生成完成",
            `${decoded.pointCount.toLocaleString("zh-CN")} 点 · ${decoded.frameCount.toLocaleString("zh-CN")} 帧`
        );
    } catch (e) {
        console.error("build_point_cloud failed:", e);
        toast.error("点云生成失败", String(e));
    } finally {
        busy.value = false;
    }
}

/** 当前回放会话的文件路径（点云基于同一份文件生成） */
const currentPath = ref("");

function setPath(path: string) {
    currentPath.value = path;
}

function enter() {
    active.value = true;
}

function exit() {
    active.value = false;
}

export function usePointCloud() {
    return {
        active,
        busy,
        progress,
        progressText,
        data,
        pointCountText,
        startFrame,
        frameCount,
        voxel,
        demCell,
        scanAngleDeg,
        refraction,
        display,
        zScale,
        currentPath,
        chooseFile,
        setPath,
        enter,
        exit,
        build,
    };
}
