import { reactive } from "vue";

export type ToastType = "success" | "info" | "warning" | "error";

export interface ToastItem {
    id: number;
    type: ToastType;
    /** 主标题，一行说清结果 */
    title: string;
    /** 可选详情，例如后端返回的错误内容 */
    message?: string;
    /** 同一条消息重复出现的次数（同类消息不会刷屏） */
    count: number;
    /** 创建时间，用于按时间排序/调试 */
    createdAt: number;
}

/** 同屏最多显示条数，超出时丢弃最旧的一条 */
const MAX_VISIBLE = 4;

/** 各类型默认停留时间（ms），0 表示常驻需手动关闭 */
const DEFAULT_DURATION: Record<ToastType, number> = {
    success: 2500,
    info: 3500,
    warning: 5000,
    error: 6000,
};

/** 全局单例：所有组件共享同一个队列，无需 provide/inject */
const items = reactive<ToastItem[]>([]);
const timers = new Map<number, number>();
let nextId = 1;

function clearTimer(id: number) {
    const timer = timers.get(id);
    if (timer !== undefined) {
        window.clearTimeout(timer);
        timers.delete(id);
    }
}

function dismiss(id: number) {
    clearTimer(id);
    const index = items.findIndex((item) => item.id === id);
    if (index !== -1) items.splice(index, 1);
}

function schedule(item: ToastItem, duration: number) {
    clearTimer(item.id);
    if (duration > 0) {
        timers.set(
            item.id,
            window.setTimeout(() => dismiss(item.id), duration)
        );
    }
}

function sameMessage(item: ToastItem, type: ToastType, title: string, message?: string) {
    return (
        item.type === type &&
        item.title === title &&
        (item.message ?? "") === (message ?? "")
    );
}

/**
 * 弹出通知。
 * 同类型、同标题、同详情已存在时不新增，只累加次数并重新计时，
 * 避免轮询/重试类错误在界面上刷屏。
 */
function show(
    type: ToastType,
    title: string,
    message?: string,
    duration?: number
): number {
    const ms = duration ?? DEFAULT_DURATION[type];

    const existing = items.find((item) => sameMessage(item, type, title, message));
    if (existing) {
        existing.count += 1;
        schedule(existing, ms);
        return existing.id;
    }

    const item: ToastItem = {
        id: nextId++,
        type,
        title,
        message,
        count: 1,
        createdAt: Date.now(),
    };
    items.push(item);

    while (items.length > MAX_VISIBLE) {
        dismiss(items[0].id);
    }

    schedule(item, ms);
    return item.id;
}

/** 清空所有通知 */
function clear() {
    for (const item of [...items]) dismiss(item.id);
}

export function useToast() {
    return {
        toasts: items,
        show,
        dismiss,
        clear,
        success: (title: string, message?: string, duration?: number) =>
            show("success", title, message, duration),
        info: (title: string, message?: string, duration?: number) =>
            show("info", title, message, duration),
        warning: (title: string, message?: string, duration?: number) =>
            show("warning", title, message, duration),
        error: (title: string, message?: string, duration?: number) =>
            show("error", title, message, duration),
    };
}
