<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SsdFile } from "../../types";
import StorageDrawer from "../StorageDrawer.vue";
import { useToast } from "../../composables/useToast";

const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
}>();

const toast = useToast();

// ===== 存储控制 =====
const storeEnabled = ref(false);
const storeFilename = ref("");
// 当前文件名是否由程序自动生成：自动生成的名字每次启动都重新生成，
// 手动输入的名字则保持复用
const storeFilenameAuto = ref(false);
const storeBusy = ref(false);

/** 默认文件名：uav_YYYYMMDD_HHMMSS */
function defaultStoreName(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `uav_${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}_${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

/** 打开/关闭 SSD 存储：开启时先写文件名与数据起始地址，再发开始命令 */
async function toggleStore() {
  const enable = storeEnabled.value;
  if (!props.connected || !props.deviceIp) {
    storeEnabled.value = !enable;
    toast.warning("请先连接设备", "存储开关需要设备在线后才能操作");
    return;
  }
  if (enable && (!storeFilename.value.trim() || storeFilenameAuto.value)) {
    storeFilename.value = defaultStoreName();
    storeFilenameAuto.value = true;
  }

  storeBusy.value = true;
  try {
    await invoke("set_ssd_store", {
      deviceIp: props.deviceIp,
      enable,
      filename: storeFilename.value.trim(),
    });
    if (!enable) {
      // 停止后稍等设备写完结束地址，再静默刷新文件摘要
      await new Promise((r) => setTimeout(r, 400));
      await loadSsdFiles(false);
    }
  } catch (e) {
    storeEnabled.value = !enable;
    console.error("set_ssd_store failed:", e);
    toast.error(enable ? "开始存储失败" : "停止存储失败", String(e));
  } finally {
    storeBusy.value = false;
  }
}

// ===== SSD 已存储文件检索 =====
const ssdFiles = ref<SsdFile[]>([]);
const ssdLoading = ref(false);
const ssdError = ref("");
const ssdDrawerVisible = ref(false);

const ssdTotalSize = computed(() => ssdFiles.value.reduce((sum, f) => sum + f.size, 0));

async function loadSsdFiles(openDrawer: boolean) {
  if (!props.connected || !props.deviceIp) return;
  if (openDrawer) ssdDrawerVisible.value = true;
  ssdLoading.value = true;
  ssdError.value = "";
  try {
    ssdFiles.value = await invoke<SsdFile[]>("list_ssd_files", {
      deviceIp: props.deviceIp,
    });
  } catch (e) {
    ssdError.value = String(e);
    console.error("list_ssd_files failed:", e);
    // 抽屉未打开时错误无处展示（例如停止存储后的静默刷新），用通知兜底
    if (!ssdDrawerVisible.value) toast.error("SSD 文件检索失败", String(e));
  } finally {
    ssdLoading.value = false;
  }
}

function refreshSsdFiles() {
  return loadSsdFiles(true);
}

/** 字节数格式化为易读单位 */
function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return (bytes / 1024 ** 3).toFixed(2) + " GB";
  if (bytes >= 1024 ** 2) return (bytes / 1024 ** 2).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
  return bytes + " B";
}

// ===== GPS 数据文件列表（0x0110 系统指令 + ls） =====
const gpsFiles = ref<string[]>([]);
const gpsLoading = ref(false);
const gpsError = ref("");
const gpsQueried = ref(false);
const gpsDrawerVisible = ref(false);

const gpsDrawerFiles = computed(() => gpsFiles.value.map((name) => ({ name })));

async function refreshGpsFiles() {
  if (!props.connected || !props.deviceIp) return;
  gpsDrawerVisible.value = true;
  gpsLoading.value = true;
  gpsError.value = "";
  try {
    gpsFiles.value = await invoke<string[]>("query_gps_files", {
      deviceIp: props.deviceIp,
    });
    gpsQueried.value = true;
  } catch (e) {
    gpsError.value = String(e);
    console.error("query_gps_files failed:", e);
    if (!gpsDrawerVisible.value) toast.error("GPS 文件检索失败", String(e));
  } finally {
    gpsLoading.value = false;
  }
}
</script>

<template>
  <div class="tab-content">
    <section class="ctrl-group">
      <h4 class="group-title">存储控制</h4>
      <div class="switch-row card-row">
        <span class="switch-row-label">
          存储开关
          <i class="switch-hint">
            {{ storeBusy ? '处理中...' : storeEnabled ? '写入中' : '已关闭' }}
          </i>
        </span>
        <label class="switch">
          <input type="checkbox" v-model="storeEnabled" :disabled="storeBusy" @change="toggleStore" />
          <span class="switch-track"><span class="switch-thumb"></span></span>
        </label>
      </div>
      <label class="field block-field">
        <span class="field-label">文件名</span>
        <input
          type="text"
          v-model="storeFilename"
          maxlength="32"
          placeholder="留空自动生成"
          :disabled="storeBusy || storeEnabled"
          @input="storeFilenameAuto = false"
        />
      </label>
      <p class="hint">写入的是采集数据；文件名最多 32 个 ASCII 字符，留空时自动生成。</p>
    </section>

    <section class="ctrl-group">
      <h4 class="group-title">
        SSD 已存储文件
        <span v-if="ssdFiles.length" class="count-badge">{{ ssdFiles.length }}</span>
      </h4>
      <button class="accent block" :disabled="ssdLoading" @click="refreshSsdFiles">
        {{ ssdLoading ? "检索中..." : "检索文件" }}
      </button>

      <template v-if="ssdFiles.length">
        <div class="file-summary">
          <span class="summary-main">{{ ssdFiles.length }} 个文件</span>
          <span class="summary-sub">合计 {{ formatSize(ssdTotalSize) }}</span>
        </div>
        <button class="block view-btn" @click="ssdDrawerVisible = true">查看文件列表</button>
      </template>
      <p v-else class="ssd-empty">暂无文件，点击“检索文件”</p>

      <p class="ssd-unit">地址单位：扇区（512 B）</p>
    </section>

    <section class="ctrl-group">
      <h4 class="group-title">
        GPS 数据文件
        <span v-if="gpsFiles.length" class="count-badge">{{ gpsFiles.length }}</span>
      </h4>
      <button class="accent block" :disabled="gpsLoading" @click="refreshGpsFiles">
        {{ gpsLoading ? "检索中..." : "检索文件" }}
      </button>

      <template v-if="gpsFiles.length">
        <div class="file-summary">
          <span class="summary-main">{{ gpsFiles.length }} 个文件</span>
        </div>
        <button class="block view-btn" @click="gpsDrawerVisible = true">查看文件列表</button>
      </template>
      <p v-else-if="gpsQueried" class="ssd-empty">无文件</p>

      <p class="ssd-unit">路径：/run/media/mmcblk1p1/（可在 config.toml [gps].dir 修改）</p>
    </section>

    <StorageDrawer
      :visible="ssdDrawerVisible"
      title="SSD 已存储文件"
      mode="ssd"
      :loading="ssdLoading"
      :error="ssdError"
      :files="ssdFiles"
      @close="ssdDrawerVisible = false"
      @refresh="refreshSsdFiles"
    />
    <StorageDrawer
      :visible="gpsDrawerVisible"
      title="GPS 数据文件"
      mode="gps"
      :loading="gpsLoading"
      :error="gpsError"
      :files="gpsDrawerFiles"
      @close="gpsDrawerVisible = false"
      @refresh="refreshGpsFiles"
    />
  </div>
</template>
