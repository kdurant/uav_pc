import { computed, ref } from "vue";
import type { SsdFile } from "../types";

/**
 * SSD 文件占用情况（全局单例）。
 *
 * 由「存储」面板检索文件后写入，设备状态面板据此显示剩余容量；
 * 断开连接或切换设备时清空，避免显示上一台设备的占用。
 */
const files = ref<SsdFile[]>([]);
/** 数据区已分配到的末尾扇区（设备按此顺序分配，删除文件留下的空洞不会被复用） */
const maxEndSector = ref(0);
/** 最近一次检索完成时间（0 表示尚未检索） */
const updatedAt = ref(0);

export function useSsdUsage() {
    function setFiles(list: SsdFile[], maxEnd = 0) {
        files.value = list;
        maxEndSector.value = maxEnd;
        updatedAt.value = Date.now();
    }

    function clear() {
        files.value = [];
        maxEndSector.value = 0;
        updatedAt.value = 0;
    }

    /**
     * 已用字节数。
     * 设备新文件总是从数据区末尾（max_end）继续写，因此用 max_end 而不是文件大小之和，
     * 否则删除文件后会被误判为空间已回收。
     */
    const usedBytes = computed(() => {
        if (maxEndSector.value > 0) return maxEndSector.value * 512;
        return files.value.reduce((sum, f) => sum + (Number(f.size) || 0), 0);
    });

    const fileCount = computed(() => files.value.length);

    /** 是否已经检索过文件列表 */
    const known = computed(() => updatedAt.value > 0);

    return { files, maxEndSector, updatedAt, usedBytes, fileCount, known, setFiles, clear };
}
