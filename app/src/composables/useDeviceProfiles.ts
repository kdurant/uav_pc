import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { NocoColumn, NocoRecord } from "../types";

/**
 * 设备档案（NocoDB）全局状态，只读。
 *
 * NocoDB 里是「设备信息 + 若干关联表」的结构（激光器 / 电机 / 增益 / 采集参数 …），
 * 关联关系完全由表结构决定：读取设备表的字段后，按关联字段指向的表动态加载，
 * 因此后续新增关联表或字段都不需要改代码。
 */

/** 设备信息表 ID（与 Rust 端常量保持一致） */
export const DEVICES_TABLE_ID = "mibklfb9ybn96lx";

/** 关联表数据：字段定义 + 记录 */
export interface LinkTable {
    columns: NocoColumn[];
    records: NocoRecord[];
}

const columns = ref<NocoColumn[]>([]);
const records = ref<NocoRecord[]>([]);
/** 以表 ID 为键的关联表缓存 */
const linkTables = ref<Record<string, LinkTable>>({});

const loading = ref(false);
const error = ref("");

async function loadRecords(query?: string, refreshColumns = false): Promise<void> {
    loading.value = true;
    error.value = "";
    try {
        // 设备表结构：首次加载或手动刷新时读取
        if (refreshColumns || !columns.value.length) {
            columns.value = await invoke<NocoColumn[]>("nocodb_columns", {
                table: DEVICES_TABLE_ID,
            });
        }

        // 关联字段指向的表（去重）
        const relatedIds = Array.from(
            new Set(
                columns.value
                    .filter((c) => c.uidt === "LinkToAnotherRecord" && c.relatedTableId)
                    .map((c) => c.relatedTableId as string)
            )
        );

        // 关联表的记录每次都重新拉取，表结构可复用缓存
        const nextTables: Record<string, LinkTable> = {};
        await Promise.all(
            relatedIds.map(async (tableId) => {
                const cached = linkTables.value[tableId];
                const [cols, rows] = await Promise.all([
                    cached && !refreshColumns
                        ? Promise.resolve(cached.columns)
                        : invoke<NocoColumn[]>("nocodb_columns", { table: tableId }),
                    invoke<NocoRecord[]>("nocodb_list", { table: tableId, query: null }),
                ]);
                nextTables[tableId] = { columns: cols, records: rows };
            })
        );
        linkTables.value = nextTables;

        const q = query?.trim();
        records.value = await invoke<NocoRecord[]>("nocodb_list", {
            table: DEVICES_TABLE_ID,
            query: q ? q : null,
        });
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
        records,
        linkTables,
        loading,
        error,
        loadRecords,
    };
}
