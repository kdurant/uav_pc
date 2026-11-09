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
        loadRecords,
    };
}
