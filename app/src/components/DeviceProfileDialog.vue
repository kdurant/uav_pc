<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
    useDeviceProfiles,
    LASER_TABLE_ID,
} from "../composables/useDeviceProfiles";
import type { NocoColumn, NocoRecord } from "../types";

/**
 * 设备档案查看器（只读）。
 *
 * 数据来自云端 NocoDB：左侧为设备列表，右侧展示该设备在表里的全部字段。
 * 字段由 NocoDB 表结构决定，新增列无需改代码；不提供新增/修改/删除。
 */
const props = defineProps<{
    visible: boolean;
    /** 当前连接设备的 SN：列表中高亮并默认选中 */
    initialSn?: string;
}>();

const emit = defineEmits<{
    (e: "update:visible", value: boolean): void;
}>();

const {
    columns,
    laserColumns,
    motorColumns,
    records,
    lasers,
    motors,
    loading,
    error,
    loadRecords,
} = useDeviceProfiles();

const search = ref("");
const selectedId = ref<number | null>(null);

/** 当前连接设备的 SN */
const currentSn = computed(() => (props.initialSn ?? "").trim());

/** SN 列（表里没有 SN 时退回第一列） */
const snColumn = computed(
    () =>
        columns.value.find((c) => c.title.trim().toUpperCase() === "SN")?.title ??
        columns.value[0]?.title ??
        "SN"
);

const selected = computed(
    () => records.value.find((r) => r.id === selectedId.value) ?? null
);

function isCurrentRecord(record: NocoRecord): boolean {
    const sn = String(record.values[snColumn.value] ?? "").trim();
    return !!currentSn.value && sn === currentSn.value;
}

function recordSubtitle(record: NocoRecord): string {
    const other = columns.value.find((c) => c.title !== snColumn.value);
    if (!other) return "";
    const value = record.values[other.title];
    return value === null || value === undefined ? "" : String(value);
}

function selectRecord(record: NocoRecord) {
    selectedId.value = record.id;
}

/** 单张表的字段值（空值显示为 “—”，长文本独占整行） */
interface DetailField {
    title: string;
    value: string;
    empty: boolean;
    long: boolean;
}

function fieldsOf(record: NocoRecord, cols: NocoColumn[]): DetailField[] {
    return cols
        .filter((col) => col.uidt !== "LinkToAnotherRecord")
        .map((col) => {
            const raw = record.values[col.title];
            const empty =
                raw === null ||
                raw === undefined ||
                (typeof raw === "string" && raw.trim() === "");
            const value = empty ? "—" : String(raw);
            return {
                title: col.title,
                value,
                empty,
                long: col.uidt === "LongText" || value.length > 40 || value.includes("\n"),
            };
        });
}

/** 设备本身的字段 */
const deviceFields = computed(() => {
    const record = selected.value;
    return record ? fieldsOf(record, columns.value) : [];
});

/** 关联的激光器 / 电机：按关联字段解析出具体记录 */
interface LinkedGroup {
    title: string;
    items: Array<{ id: number; fields: DetailField[] }>;
}

const linkedGroups = computed<LinkedGroup[]>(() => {
    const record = selected.value;
    if (!record) return [];
    return columns.value
        .filter((col) => col.uidt === "LinkToAnotherRecord")
        .map((col) => {
            const raw = record.values[col.title];
            const ids = Array.isArray(raw)
                ? raw
                      .map((item) => Number((item as Record<string, unknown>)?.Id))
                      .filter((id) => Number.isFinite(id))
                : [];
            const isLaser = col.relatedTableId === LASER_TABLE_ID;
            const pool = isLaser ? lasers.value : motors.value;
            const cols = isLaser ? laserColumns.value : motorColumns.value;
            const items = ids
                .map((id) => pool.find((r) => r.id === id))
                .filter((r): r is NocoRecord => !!r)
                .map((r) => ({ id: r.id, fields: fieldsOf(r, cols) }));
            return { title: col.title, items };
        });
});

function refreshList() {
    // 连同表结构一起刷新，NocoDB 新增列后点刷新即可看到
    void loadRecords(search.value, true);
}

let searchTimer: number | null = null;
/** 打开弹窗时程序化清空搜索词，避免触发一次重复查询 */
let skipNextSearch = false;

watch(search, () => {
    if (skipNextSearch) {
        skipNextSearch = false;
        return;
    }
    if (searchTimer !== null) window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => loadRecords(search.value), 300);
});

watch(
    () => props.visible,
    async (visible) => {
        if (!visible) return;
        // 搜索框默认留空，加载全部档案；当前设备高亮并自动选中
        if (search.value !== "") {
            skipNextSearch = true;
            search.value = "";
        }
        await loadRecords("", true);

        const hit = records.value.find((r) => isCurrentRecord(r));
        if (hit) selectRecord(hit);
        else if (records.value.length) selectRecord(records.value[0]);
        else selectedId.value = null;
    }
);

function close() {
    emit("update:visible", false);
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-overlay" @click.self="close">
      <div class="modal">
        <header class="modal-head">
          <div class="modal-titles">
            <h3>设备档案</h3>
            <p>数据来自云端 NocoDB，字段随表结构自动生成</p>
          </div>
          <button class="close-btn" title="关闭" @click="close">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
              stroke-linecap="round">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </header>

        <div class="modal-body">
          <!-- 左侧设备列表 -->
          <aside class="list-pane">
            <div class="search-row">
              <input v-model="search" type="text" placeholder="搜索 SN" />
              <button class="refresh-btn" title="重新从云端加载" @click="refreshList">刷新</button>
            </div>

            <div v-if="loading" class="list-tip">加载中...</div>
            <div v-else-if="error" class="list-tip error">{{ error }}</div>
            <div v-else-if="!records.length" class="list-tip">没有匹配的设备</div>
            <ul v-else class="record-list">
              <li
                v-for="r in records"
                :key="r.id"
                :class="{ active: r.id === selectedId, current: isCurrentRecord(r) }"
                @click="selectRecord(r)"
              >
                <span class="sn">
                  {{ r.values[snColumn] ?? "-" }}
                  <em v-if="isCurrentRecord(r)" class="current-badge">当前设备</em>
                </span>
                <span class="sub">{{ recordSubtitle(r) }}</span>
              </li>
            </ul>
          </aside>

          <!-- 右侧只读详情 -->
          <section class="detail-pane">
            <div v-if="!selected" class="detail-empty">
              <p>{{ error ? "云端数据读取失败" : "请选择左侧设备档案" }}</p>
              <p v-if="error" class="detail-error">{{ error }}</p>
              <button v-if="error" @click="refreshList">重试</button>
            </div>

            <template v-else>
              <div class="detail-head">
                <span class="detail-title">
                  设备 {{ selected.values[snColumn] ?? "-" }}
                  <em v-if="isCurrentRecord(selected)" class="current-badge">当前设备</em>
                </span>
                <span class="detail-meta">{{ deviceFields.length }} 项信息</span>
              </div>

              <div class="detail-scroll">
                <dl class="detail-list">
                  <div
                    v-for="field in deviceFields"
                    :key="field.title"
                    class="detail-row"
                    :class="{ long: field.long, empty: field.empty }"
                  >
                    <dt>{{ field.title }}</dt>
                    <dd :title="field.value">{{ field.value }}</dd>
                  </div>
                </dl>

                <section
                  v-for="group in linkedGroups"
                  :key="group.title"
                  class="linked-group"
                >
                  <h4 class="linked-title">
                    {{ group.title }}
                    <span v-if="group.items.length" class="linked-count">{{ group.items.length }}</span>
                  </h4>

                  <p v-if="!group.items.length" class="linked-empty">未关联</p>

                  <div v-for="item in group.items" :key="item.id" class="linked-card">
                    <dl class="detail-list compact">
                      <div
                        v-for="field in item.fields"
                        :key="field.title"
                        class="detail-row"
                        :class="{ long: field.long, empty: field.empty }"
                      >
                        <dt>{{ field.title }}</dt>
                        <dd :title="field.value">{{ field.value }}</dd>
                      </div>
                    </dl>
                  </div>
                </section>
              </div>
            </template>
          </section>
        </div>

        <footer class="modal-foot">
          <span class="foot-tip">只读视图，如需修改请到 NocoDB 网页端操作</span>
          <button @click="close">关闭</button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: rgba(15, 23, 42, 0.32);
  z-index: 1000;
  animation: overlay-in 0.16s ease-out;
}

@keyframes overlay-in {
  from {
    opacity: 0;
  }
}

.modal {
  display: flex;
  flex-direction: column;
  width: 820px;
  max-width: calc(100vw - 40px);
  height: 600px;
  max-height: 90vh;
  background: #fff;
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: 0 8px 28px rgba(16, 24, 40, 0.16);
  overflow: hidden;
  animation: modal-in 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes modal-in {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.985);
  }
}

.modal-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 18px 12px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.modal-titles h3 {
  font-size: 15px;
  font-weight: 650;
  color: var(--text-primary);
}

.modal-titles p {
  margin-top: 3px;
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.close-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  border-radius: 7px;
  background: transparent;
  box-shadow: none;
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.close-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.close-btn svg {
  width: 15px;
  height: 15px;
}

.modal-body {
  flex: 1;
  display: flex;
  min-height: 0;
}

/* ---------- 左侧列表 ---------- */
.list-pane {
  width: 250px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--border-color);
  background: var(--bg-inset);
  min-height: 0;
}

.search-row {
  display: flex;
  gap: 8px;
  padding: 12px;
}

.search-row input {
  flex: 1;
  min-width: 0;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: 12.5px;
  background: #fff;
}

.search-row input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-glow);
}

.refresh-btn {
  height: 30px;
  padding: 0 10px;
  font-size: 12.5px;
  flex-shrink: 0;
}

.record-list {
  list-style: none;
  margin: 0;
  padding: 0 8px 8px;
  overflow-y: auto;
  flex: 1;
}

.record-list li {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: 8px 10px;
  border-radius: 8px;
  cursor: pointer;
  border: 1px solid transparent;
}

.record-list li:hover {
  background: #fff;
  border-color: var(--border-color);
}

.record-list li.active {
  background: #fff;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-glow);
}

/* 本机连接的设备：左侧色条 + 浅绿底 */
.record-list li.current {
  border-left: 3px solid var(--success, #12884a);
  background: rgba(18, 136, 74, 0.06);
}

.record-list li.current.active {
  background: #fff;
}

.record-list .sn {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12.5px;
  color: var(--text-primary);
  font-weight: 600;
}

.record-list .sub {
  font-size: 11.5px;
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.current-badge {
  margin-left: 6px;
  padding: 0 5px;
  border-radius: 4px;
  background: rgba(18, 136, 74, 0.12);
  color: var(--success, #12884a);
  font-size: 9.5px;
  font-style: normal;
  font-weight: 600;
  vertical-align: 1px;
}

.list-tip {
  padding: 18px 14px;
  font-size: 12.5px;
  color: var(--text-tertiary);
}

.list-tip.error {
  color: var(--danger, #dc2626);
  word-break: break-all;
}

/* ---------- 右侧详情 ---------- */
.detail-pane {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.detail-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
  padding: 12px 18px;
  border-bottom: 1px solid var(--border-color);
}

.detail-title {
  font-size: 13px;
  font-weight: 650;
  color: var(--text-primary);
}

.detail-meta {
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.detail-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 18px 16px;
}

.detail-list {
  margin: 0;
  padding: 0;
}

.detail-list.compact {
  padding: 0;
}

.linked-group {
  margin-top: 14px;
}

.linked-title {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 4px;
  font-size: 11px;
  font-weight: 650;
  letter-spacing: 0.08em;
  color: var(--text-tertiary);
  text-transform: uppercase;
}

.linked-count {
  padding: 0 5px;
  border-radius: 4px;
  background: var(--bg-inset);
  color: var(--text-secondary);
  font-size: 10px;
  letter-spacing: 0;
}

.linked-empty {
  margin: 0;
  padding: 6px 0;
  font-size: 12px;
  color: var(--text-tertiary);
}

.linked-card {
  padding: 4px 12px;
  margin-bottom: 6px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--bg-inset);
}

.linked-card .detail-row {
  border-bottom: none;
  padding: 5px 0;
}

.detail-row {
  display: grid;
  grid-template-columns: 118px minmax(0, 1fr);
  gap: 12px;
  align-items: baseline;
  padding: 8px 0;
  border-bottom: 1px dashed var(--border-color);
}

.detail-row:last-child {
  border-bottom: none;
}

.detail-row dt {
  font-size: 12px;
  color: var(--text-secondary);
  font-weight: 600;
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail-row dd {
  margin: 0;
  min-width: 0;
  font-size: 12.5px;
  color: var(--text-primary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  word-break: break-all;
  white-space: pre-wrap;
}

/* 长文本（备注 / 维修记录等）：标签在上，内容整行显示并保留换行 */
.detail-row.long {
  grid-template-columns: minmax(0, 1fr);
  gap: 4px;
  align-items: start;
}

.detail-row.long dt {
  text-align: left;
}

/* 空值弱化显示，避免抢注意力 */
.detail-row.empty dd {
  color: var(--text-tertiary);
}

.detail-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 30px;
  color: var(--text-tertiary);
  font-size: 13px;
}

.detail-error {
  margin: 0;
  max-width: 100%;
  font-size: 11.5px;
  color: var(--danger, #dc2626);
  word-break: break-all;
}

.modal-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 18px;
  border-top: 1px solid var(--border-color);
  flex-shrink: 0;
}

.foot-tip {
  font-size: 11.5px;
  color: var(--text-tertiary);
}
</style>
