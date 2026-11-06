import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** 上位机网络端点（对应 Rust `LocalEndpoint`） */
export interface LocalEndpoint {
    /** config.toml 中配置的本机 IP */
    configuredIp: string;
    /** config.toml 中配置的本机端口 */
    configuredPort: number;
    /** 实际绑定成功的本地地址，未初始化或绑定失败时为空 */
    boundAddr: string;
}

const endpoint = ref<LocalEndpoint | null>(null);
const loading = ref(false);

export function useLocalEndpoint() {
    /** 重新读取本机端点（配置改动或首次绑定后调用） */
    async function refresh() {
        if (loading.value) return;
        loading.value = true;
        try {
            endpoint.value = await invoke<LocalEndpoint>("get_local_endpoint");
        } catch (e) {
            console.error("get_local_endpoint failed:", e);
        } finally {
            loading.value = false;
        }
    }

    /** 简短文本：优先显示实际绑定地址 */
    const label = computed(() => {
        const e = endpoint.value;
        if (!e) return "未知";
        const configured = `${e.configuredIp}:${e.configuredPort}`;
        if (!e.boundAddr) return `${configured}（未绑定）`;
        if (e.boundAddr !== configured) return `${e.boundAddr}（配置 ${configured}）`;
        return e.boundAddr;
    });

    /** 纯 IP 文本（不含端口和“本机”前缀），用于「本机 -> 设备」路径展示 */
    const address = computed(() => {
        const e = endpoint.value;
        if (!e) return "未知";
        // 实际绑定地址可能带端口，这里只取 IP 部分
        const bound = e.boundAddr ? e.boundAddr.replace(/:\d+$/, "") : "";
        return bound || e.configuredIp;
    });

    /** 本机 IP 未绑定成功（多半是 config.toml 里的 IP 不属于本机） */
    const unbound = computed(() => {
        const e = endpoint.value;
        return e !== null && !e.boundAddr;
    });

    /** 连接异常时给用户的排查提示 */
    const hint = computed(() => {
        const e = endpoint.value;
        if (!e) return "本机 IP 未知";
        if (!e.boundAddr) {
            return `本机 ${e.configuredIp} 未绑定成功，请确认该 IP 属于本机`;
        }
        return `本机 ${address.value}`;
    });

    return { endpoint, loading, label, address, unbound, hint, refresh };
}
