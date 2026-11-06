import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { NocoColumn, NocoRecord } from "../types";

/**
 * 设备档案（NocoDB）全局状态，只读。
 *
 * NocoDB 里是三张表：
 *   设备信息(SN) ── 关联 ──▶ 激光器 / 电机
 * 表结构由 NocoDB 决定，前端按字段动态展示；新增列无需改代码。
 */

/** 数据表 ID（与 Rust 端常量保持一致） */
export const LASER_TABLE_ID = "mmve483ctlcgjhu";
export const MOTOR_TABLE_ID = "mexv8ok3duce7s0";

const columns = ref<NocoColumn[]>([]);
const laserColumns = ref<NocoColumn[]>([]);
const motorColumns = ref<NocoColumn[]>([]);

const records = ref<NocoRecord[]>([]);
const lasers = ref<NocoRecord[]>([]);
const motors = ref<NocoRecord[]>([]);

const loading = ref(false);
const error = ref("");

/** 连接设备后自动匹配到的档案 */
const current = ref<NocoRecord | null>(null);
/** 当前设备 SN */
const currentSn = ref("");
/** 是否已尝试过匹配 */
const currentChecked = ref(false);
/** 匹配失败原因（网络/服务端错误） */
const currentError = ref("");

async function loadSchema(
    table: string,
    target: typeof columns,
    force: boolean
): Promise<void> {
    if (!force && target.value.length) return;
    target.value = await invoke<NocoColumn[]>("nocodb_columns", { table });
}

async function loadPools(): Promise<void> {
    const [l, m] = await Promise.all([
        invoke<NocoRecord[]>("nocodb_list", { table: "lasers", query: null }),
        invoke<NocoRecord[]>("nocodb_list", { table: "motors", query: null }),
    ]);
    lasers.value = l;
    motors.value = m;
}

async function loadRecords(query?: string, refreshColumns = false): Promise<void> {
    loading.value = true;
    error.value = "";
    try {
        // refreshColumns=true 时重新读取表结构，NocoDB 新增列后无需重启
        await Promise.all([
            loadSchema("devices", columns, refreshColumns),
            loadSchema("lasers", laserColumns, refreshColumns),
            loadSchema("motors", motorColumns, refreshColumns),
        ]);
        const q = query?.trim();
        const [devices, laserList, motorList] = await Promise.all([
            invoke<NocoRecord[]>("nocodb_list", {
                table: "devices",
                query: q ? q : null,
            }),
            invoke<NocoRecord[]>("nocodb_list", { table: "lasers", query: null }),
            invoke<NocoRecord[]>("nocodb_list", { table: "motors", query: null }),
        ]);
        records.value = devices;
        lasers.value = laserList;
        motors.value = motorList;
    } catch (e) {
        console.error("nocodb load failed:", e);
        error.value = String(e);
        records.value = [];
    } finally {
        loading.value = false;
    }
}

async function findBySn(sn: string): Promise<NocoRecord | null> {
    const value = sn.trim();
    if (!value) return null;
    return invoke<NocoRecord | null>("nocodb_find_by_sn", { sn: value });
}

/** 连接设备后调用：按 SN 匹配档案（失败不影响采集） */
async function refreshCurrent(): Promise<void> {
    const sn = currentSn.value.trim();
    if (!sn) {
        current.value = null;
        currentChecked.value = false;
        return;
    }
    currentError.value = "";
    try {
        current.value = await findBySn(sn);
        // 关联表数据用于展示激光器/电机详情
        if (current.value) {
            await Promise.all([
                loadSchema("lasers", laserColumns, false),
                loadSchema("motors", motorColumns, false),
            ]);
            await loadPools();
        }
    } catch (e) {
        console.error("nocodb_find_by_sn failed:", e);
        current.value = null;
        currentError.value = String(e);
    } finally {
        currentChecked.value = true;
    }
}

function setCurrentSn(sn: string) {
    const value = (sn ?? "").trim();
    if (value === currentSn.value) return;
    currentSn.value = value;
    current.value = null;
    currentError.value = "";
    currentChecked.value = false;
    if (value) void refreshCurrent();
}

/** 档案里需要展示的普通字段（过滤空值、跳过关联字段） */
const currentFields = computed(() => {
    const record = current.value;
    if (!record) return [] as Array<[string, string]>;
    return Object.entries(record.values)
        .filter(([, v]) => {
            if (Array.isArray(v)) return false; // 关联字段单独处理
            if (v === null || v === undefined) return false;
            if (typeof v === "string") return v.trim() !== "";
            return true;
        })
        .map(([k, v]) => [k, String(v)] as [string, string]);
});

/** 当前设备关联的激光器 / 电机名称（取自关联字段里的型号） */
const currentLinks = computed(() => {
    const record = current.value;
    if (!record) return [] as Array<[string, string]>;
    return Object.entries(record.values)
        .filter(([, v]) => Array.isArray(v) && v.length > 0)
        .map(([k, v]) => {
            const names = (v as Array<Record<string, unknown>>)
                .map((item) => {
                    const entry = Object.entries(item).find(([key]) => key !== "Id");
                    return entry ? String(entry[1]) : "";
                })
                .filter((s) => s !== "");
            return [k, names.join("、")] as [string, string];
        })
        .filter(([, names]) => names !== "");
});

export function useDeviceProfiles() {
    return {
        columns,
        laserColumns,
        motorColumns,
        records,
        lasers,
        motors,
        loading,
        error,
        current,
        currentSn,
        currentChecked,
        currentError,
        currentFields,
        currentLinks,
        loadRecords,
        findBySn,
        refreshCurrent,
        setCurrentSn,
    };
}
