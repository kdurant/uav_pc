import { ref } from "vue";

/** 弹窗可见性（全局单例：设备状态与控制面板共用同一个弹窗） */
const visible = ref(false);
/** 打开时选中的模式：0 内触发 / 1 外触发 / 2 PIN 触发 */
const activeMode = ref(0);

/** 把任意输入归一化到 0~2 */
function normalizeMode(mode: number): number {
    if (!Number.isFinite(mode)) return 0;
    return Math.min(Math.max(Math.trunc(mode), 0), 2);
}

export function useTriggerHelp() {
    function open(mode: number) {
        activeMode.value = normalizeMode(mode);
        visible.value = true;
    }

    function close() {
        visible.value = false;
    }

    return { visible, activeMode, open, close };
}
