<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  SdFile,
  SsdDirectoryInfo,
  SsdDownloadProgress,
  SsdDownloadState,
  SsdFile,
  SystemStatus,
} from "../../types";
import StorageDrawer from "../StorageDrawer.vue";
import ConfirmDialog from "../ConfirmDialog.vue";
import { useToast } from "../../composables/useToast";
import { useSsdUsage } from "../../composables/useSsdUsage";

const props = defineProps<{
  connected: boolean;
  deviceIp?: string;
  status?: SystemStatus | null;
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
  // SATA 未连接时拦截：只提示用户，不向设备发送任何指令
  if (enable && props.status && props.status.sata_dev_identify_done !== 1) {
    storeEnabled.value = false;
    toast.warning("SATA 未连接", "请确认 SATA 硬盘已连接后再开始存储");
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
// 同步给设备状态面板，用于显示 SSD 剩余容量
const {
  maxEndSector: ssdMaxEnd,
  known: ssdUsageKnown,
  setFiles: setSsdUsage,
  clear: clearSsdUsage,
} = useSsdUsage();

// 断开连接或切换设备后，上一台设备的 SSD 占用不再有效
watch(
  () => [props.connected, props.deviceIp] as const,
  ([connected]) => {
    if (!connected) clearSsdUsage();
  }
);
watch(
  () => props.deviceIp,
  () => clearSsdUsage()
);

const ssdLoading = ref(false);
const ssdError = ref("");
const ssdDrawerVisible = ref(false);

const ssdTotalSize = computed(() => ssdFiles.value.reduce((sum, f) => sum + f.size, 0));

// ===== 文件下载（SSD / SD 卡共用进度 UI） =====
const downloadState = ref<SsdDownloadState | null>(null);
const downloadSource = ref<"ssd" | "sd" | null>(null);
const downloadError = ref("");
const downloadMessage = ref("");

/** SSD 下载条件：连接正常、状态读取开启、未存储、SATA 已连接、无并发下载 */
const canDownloadSsd = computed(() => {
  if (!props.connected || !props.deviceIp) return false;
  if (!props.status) return false;
  if (downloadState.value) return false;
  if (props.status.ssd_store_enable === 1) return false;
  if (props.status.sata_dev_identify_done !== 1) return false;
  return true;
});

/** SD 卡下载条件：连接正常、无并发下载 */
const canDownloadSd = computed(() => {
  return !!props.connected && !!props.deviceIp && !downloadState.value;
});

// ===== SSD 容量与写入速率估算 =====
/** 单次采集写入 SSD 的字节数（与设备端 bytes_of_frame 一致） */
const storageFrameBytes = computed(() => {
  const st = props.status;
  if (!st) return 0;
  return 128 + 8 * st.first_len + (st.second_len * 2 + 4) * 4;
});

/** SSD 写入速率（MB/s）：采集频率 × 每帧字节数 */
const storageRateMbps = computed(() => {
  const st = props.status;
  if (!st) return null;
  const freq = st.trg_cycle > 0 ? st.trg_cycle : st.laser_freq;
  if (!Number.isFinite(freq) || freq <= 0) return null;
  return (freq * storageFrameBytes.value) / (1024 * 1024);
});

/** SSD 总容量（字节），SATA 未连接时返回 null */
const totalBytes = computed(() => {
  const st = props.status;
  if (!st || st.sata_dev_identify_done !== 1 || st.sata_dev_tot_sec_num <= 0) return null;
  return st.sata_dev_tot_sec_num * 512;
});

/**
 * SSD 剩余容量（字节）。
 *
 * 优先用文件表里数据区的末尾扇区（max_end）计算：设备新文件总是从 max_end 继续写，
 * 删除文件留下的空洞不会回收，所以「总容量 − max_end」才是真实可用空间。
 * 未检索过文件表时退回到设备状态里的 sata_app_lba_next（设备重启后该值可能为 0，故仅作兜底）。
 */
const remainingBytes = computed(() => {
  const total = totalBytes.value;
  if (total === null) return null;
  if (ssdUsageKnown.value && ssdMaxEnd.value > 0) {
    return Math.max(0, total - ssdMaxEnd.value * 512);
  }
  const st = props.status;
  if (!st) return null;
  const used = Math.min(Math.max(st.sata_app_lba_next, 0), st.sata_dev_tot_sec_num);
  return (st.sata_dev_tot_sec_num - used) * 512;
});

/** 预计可存储时间（秒） */
const storageSeconds = computed(() => {
  const rate = storageRateMbps.value;
  const remain = remainingBytes.value;
  if (!rate || rate <= 0 || remain === null) return null;
  return remain / (rate * 1024 * 1024);
});

/** 秒数格式化为易读的剩余时间 */
function formatDuration(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds <= 0) return "--";
  const total = Math.floor(seconds);
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (days > 0) return `${days} 天 ${hours} 小时`;
  if (hours > 0) return `${hours} 小时 ${minutes} 分`;
  if (minutes > 0) return `${minutes} 分 ${total % 60} 秒`;
  return `${total} 秒`;
}

/** 弹出目录选择框；取消返回 null */
async function chooseSaveDir(title: string): Promise<string | null> {
  let selected: string | string[] | null = null;
  try {
    selected = await open({
      directory: true,
      multiple: false,
      title,
    });
  } catch (e) {
    downloadError.value = `打开目录选择框失败: ${e}`;
    return null;
  }
  if (!selected || Array.isArray(selected)) return null;
  if (!props.connected || !props.deviceIp) {
    downloadError.value = "设备已断开连接";
    return null;
  }
  return selected;
}

/** 执行下载：统一处理进度、速度、取消、提示 */
async function runDownload(
  source: "ssd" | "sd",
  name: string,
  total: number,
  command: string,
  args: Record<string, unknown>,
) {
  if (downloadState.value) return;
  downloadSource.value = source;
  downloadError.value = "";
  downloadMessage.value = "";
  downloadState.value = {
    name,
    transferred: 0,
    total,
    percent: 0,
    speed_bps: 0,
    chunk_mb: 0,
    cancelling: false,
  };

  const channel = new Channel<SsdDownloadProgress>();
  channel.onmessage = (progress) => {
    if (!downloadState.value) return;
    downloadState.value = {
      ...downloadState.value,
      ...progress,
      cancelling: downloadState.value.cancelling,
    };
  };

  try {
    const savedPath = await invoke<string>(command, { ...args, channel });
    downloadMessage.value = `已保存到 ${savedPath}`;
    toast.success("文件下载完成", name);
  } catch (e) {
    const msg = String(e);
    if (msg.includes("取消")) {
      downloadError.value = "已取消下载，临时文件已删除";
      toast.info("已取消下载", name);
    } else {
      downloadError.value = msg;
      toast.error("文件下载失败", msg);
    }
  } finally {
    downloadState.value = null;
    downloadSource.value = null;
  }
}

/** 下载 SSD 文件 */
async function downloadFile(file: {
  name: string;
  start?: number;
  end?: number;
  size?: number;
}) {
  const deviceIp = props.deviceIp;
  if (!props.connected || !deviceIp || downloadState.value) return;
  if (!props.status) {
    downloadError.value = "请先开启“读取设备状态”后再下载文件";
    return;
  }
  if (file.start === undefined || file.end === undefined || file.end <= file.start) {
    downloadError.value = "文件地址无效，无法下载";
    return;
  }
  if (props.status.ssd_store_enable === 1) {
    downloadError.value = "设备正在存储，请先停止存储后再下载";
    return;
  }
  if (props.status.sata_dev_identify_done !== 1) {
    downloadError.value = "SATA 未连接，无法下载";
    return;
  }

  const selected = await chooseSaveDir("选择 SSD 文件保存目录");
  if (!selected) return;

  await runDownload(
    "ssd",
    file.name,
    (file.end - file.start) * 512,
    "download_ssd_file",
    {
      deviceIp,
      startLba: file.start,
      totalSectors: file.end - file.start,
      filename: file.name,
      localDir: selected,
    },
  );
}

/** 下载 SD 卡文件 */
async function downloadSdFile(file: {
  name: string;
  size?: number;
  mtime?: number;
}) {
  const deviceIp = props.deviceIp;
  if (!props.connected || !deviceIp || downloadState.value) return;

  const selected = await chooseSaveDir("选择 SD 卡文件保存目录");
  if (!selected) return;

  await runDownload("sd", file.name, file.size ?? 0, "download_sd_file", {
    deviceIp,
    filename: file.name,
    localDir: selected,
  });
}

/** 取消下载：根据来源调用对应后端的取消命令 */
async function cancelDownload() {
  if (!downloadState.value) return;
  downloadState.value = { ...downloadState.value, cancelling: true };
  const command =
    downloadSource.value === "sd" ? "cancel_sd_download" : "cancel_ssd_download";
  try {
    await invoke(command);
  } catch (e) {
    downloadError.value = String(e);
  }
}

async function loadSsdFiles(openDrawer: boolean) {
  if (!props.connected || !props.deviceIp) return;
  if (openDrawer) ssdDrawerVisible.value = true;
  ssdLoading.value = true;
  ssdError.value = "";
  try {
    const dir = await invoke<SsdDirectoryInfo>("list_ssd_files", {
      deviceIp: props.deviceIp,
    });
    ssdFiles.value = dir.files;
    setSsdUsage(dir.files, dir.maxEnd);
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

/** 下载速度格式化 */
function formatSpeed(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "--";
  return `${(bytesPerSec / 1024 / 1024).toFixed(1)} MB/s`;
}

// ===== GPS / SD 卡数据文件列表（SFTP） =====
const gpsFiles = ref<SdFile[]>([]);
const gpsLoading = ref(false);
const gpsError = ref("");
const gpsQueried = ref(false);
const gpsDrawerVisible = ref(false);

const gpsTotalSize = computed(() => gpsFiles.value.reduce((sum, f) => sum + f.size, 0));
const gpsDrawerFiles = computed(() =>
  gpsFiles.value.map((f) => ({ name: f.name, size: f.size, mtime: f.mtime })),
);

async function refreshGpsFiles() {
  if (!props.connected || !props.deviceIp) return;
  gpsDrawerVisible.value = true;
  gpsLoading.value = true;
  gpsError.value = "";
  try {
    gpsFiles.value = await invoke<SdFile[]>("list_sd_files", {
      deviceIp: props.deviceIp,
    });
    gpsQueried.value = true;
  } catch (e) {
    gpsError.value = String(e);
    gpsFiles.value = [];
    console.error("list_sd_files failed:", e);
    if (!gpsDrawerVisible.value) toast.error("SD 卡文件检索失败", String(e));
  } finally {
    gpsLoading.value = false;
  }
}

// ===== GPS 文件删除 =====
const gpsDeleting = ref(false);

type PendingDelete =
  | { kind: "single"; name: string }
  | { kind: "all"; count: number; size: string };

/** 待确认的删除操作，null 表示没有弹窗 */
const pendingDelete = ref<PendingDelete | null>(null);

const deleteConfirmMessage = computed(() => {
  const pending = pendingDelete.value;
  if (!pending) return "";
  if (pending.kind === "all") {
    return `确定删除 SD 卡上的全部 ${pending.count} 个 GPS 文件（合计 ${pending.size}）？删除后无法恢复。`;
  }
  return `确定删除 ${pending.name}？删除后无法恢复。`;
});

/** 请求删除单个 GPS 文件（先弹确认框） */
function requestDeleteGpsFile(file: { name: string }) {
  if (!props.connected || !props.deviceIp || gpsDeleting.value || downloadState.value) {
    return;
  }
  pendingDelete.value = { kind: "single", name: file.name };
}

/** 请求删除全部 GPS 文件（先弹确认框） */
function requestDeleteAllGpsFiles() {
  if (!props.connected || !props.deviceIp || gpsDeleting.value || downloadState.value) {
    return;
  }
  const count = gpsFiles.value.length;
  if (count === 0) return;
  pendingDelete.value = {
    kind: "all",
    count,
    size: formatSize(gpsTotalSize.value),
  };
}

/** 确认后执行删除 */
async function confirmDelete() {
  const pending = pendingDelete.value;
  pendingDelete.value = null;
  if (!pending) return;

  if (pending.kind === "single") {
    await doDeleteGpsFile(pending.name);
  } else {
    await doDeleteAllGpsFiles();
  }
}

async function doDeleteGpsFile(name: string) {
  const deviceIp = props.deviceIp;
  if (!deviceIp) return;
  gpsDeleting.value = true;
  try {
    await invoke("delete_sd_file", { deviceIp, filename: name });
    toast.success("GPS 文件已删除", name);
    await refreshGpsFiles();
  } catch (e) {
    console.error("delete_sd_file failed:", e);
    toast.error("删除 GPS 文件失败", String(e));
  } finally {
    gpsDeleting.value = false;
  }
}

async function doDeleteAllGpsFiles() {
  const deviceIp = props.deviceIp;
  if (!deviceIp) return;
  gpsDeleting.value = true;
  try {
    const result = await invoke<{ deleted: number; failed: string[] }>(
      "delete_all_gps_files",
      { deviceIp }
    );
    if (result.failed.length > 0) {
      toast.warning(
        "部分 GPS 文件删除失败",
        `已删除 ${result.deleted} 个，失败 ${result.failed.length} 个：${result.failed
          .slice(0, 3)
          .join("；")}`
      );
    } else {
      toast.success("GPS 文件已全部删除", `共删除 ${result.deleted} 个文件`);
    }
    await refreshGpsFiles();
  } catch (e) {
    console.error("delete_all_gps_files failed:", e);
    toast.error("删除 GPS 文件失败", String(e));
  } finally {
    gpsDeleting.value = false;
  }
}
</script>

<template>
  <div class="tab-content">
    <section
      v-if="downloadState || downloadMessage || downloadError"
      class="ctrl-group"
    >
      <h4 class="group-title">文件下载</h4>
      <p v-if="downloadState" class="hint">
        正在下载 {{ downloadState.name }}：{{ downloadState.percent.toFixed(1) }}%
        · {{ formatSpeed(downloadState.speed_bps) }}
        <template v-if="downloadState.chunk_mb"> · 分块 {{ downloadState.chunk_mb }}MB</template>
      </p>
      <div v-if="downloadState" class="btn-row">
        <button :disabled="downloadState.cancelling" @click="cancelDownload">
          {{ downloadState.cancelling ? "取消中..." : "取消下载" }}
        </button>
      </div>
      <p v-if="downloadMessage" class="ssd-empty">{{ downloadMessage }}</p>
      <p v-if="downloadError" class="ssd-error">{{ downloadError }}</p>
    </section>

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
          <input
            type="checkbox"
            v-model="storeEnabled"
            :disabled="storeBusy || !!downloadState"
            @change="toggleStore"
          />
          <span class="switch-track"><span class="switch-thumb"></span></span>
        </label>
      </div>
      <p v-if="props.status && props.status.sata_dev_identify_done !== 1" class="hint">
        SATA 未连接，无法开始存储，请检查硬盘连接。
      </p>
      <label class="field block-field">
        <span class="field-label">文件名</span>
        <input
          type="text"
          v-model="storeFilename"
          maxlength="32"
          placeholder="留空自动生成"
          :disabled="storeBusy || storeEnabled || !!downloadState"
          @input="storeFilenameAuto = false"
        />
      </label>
      <p class="hint">写入的是采集数据；文件名最多 32 个 ASCII 字符，留空时自动生成。</p>

      <!-- 开启存储后按剩余容量与写入速率估算可存储时间 -->
      <div
        v-if="totalBytes !== null && storageRateMbps !== null"
        class="store-estimate"
        :class="{ active: storeEnabled || props.status?.ssd_store_enable === 1 }"
      >
        <div class="estimate-row">
          <span class="estimate-label">写入速率</span>
          <span class="estimate-value">{{ storageRateMbps.toFixed(2) }} MB/s</span>
        </div>
        <div class="estimate-row">
          <span class="estimate-label">剩余容量</span>
          <span
            class="estimate-value"
            :title="`总容量 ${formatSize(totalBytes ?? 0)}`"
          >
            {{ remainingBytes === null ? "-" : formatSize(remainingBytes) }}
          </span>
        </div>
        <div class="estimate-row">
          <span class="estimate-label">预计可存储</span>
          <span class="estimate-value strong">{{ formatDuration(storageSeconds) }}</span>
        </div>
        <p class="hint">按当前采集参数与剩余磁盘容量估算，实际受磁盘写入速度和文件系统开销影响。</p>
      </div>
    </section>

    <section class="ctrl-group">
      <h4 class="group-title">
        SSD 已存储文件
        <span v-if="ssdFiles.length" class="count-badge">{{ ssdFiles.length }}</span>
      </h4>
      <button
        class="accent block"
        :disabled="ssdLoading || !!downloadState"
        @click="refreshSsdFiles"
      >
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

      <p v-if="!props.status" class="hint">
        请先开启“读取设备状态”后再下载文件。
      </p>
      <p v-else-if="props.status.ssd_store_enable === 1" class="hint">
        设备正在存储，停止存储后才能下载文件。
      </p>

      <p class="ssd-unit">地址单位：扇区（512 B）</p>
    </section>

    <section class="ctrl-group">
      <h4 class="group-title">
        GPS 数据文件
        <span v-if="gpsFiles.length" class="count-badge">{{ gpsFiles.length }}</span>
      </h4>
      <button
        class="accent block"
        :disabled="gpsLoading || !!downloadState"
        @click="refreshGpsFiles"
      >
        {{ gpsLoading ? "检索中..." : "检索文件" }}
      </button>

      <template v-if="gpsFiles.length">
        <div class="file-summary">
          <span class="summary-main">{{ gpsFiles.length }} 个文件</span>
          <span class="summary-sub">合计 {{ formatSize(gpsTotalSize) }}</span>
        </div>
        <button class="block view-btn" @click="gpsDrawerVisible = true">查看文件列表</button>
      </template>
      <p v-else-if="gpsQueried" class="ssd-empty">无文件</p>

    </section>

    <StorageDrawer
      :visible="ssdDrawerVisible"
      title="SSD 已存储文件"
      mode="ssd"
      :loading="ssdLoading"
      :error="ssdError"
      :files="ssdFiles"
      :download="downloadState"
      :can-download="canDownloadSsd"
      :download-error="downloadError"
      :download-message="downloadMessage"
      @close="ssdDrawerVisible = false"
      @refresh="refreshSsdFiles"
      @download="downloadFile"
      @cancel-download="cancelDownload"
    />
    <StorageDrawer
      :visible="gpsDrawerVisible"
      title="SD 卡 GPS 数据文件"
      mode="gps"
      :loading="gpsLoading"
      :error="gpsError"
      :files="gpsDrawerFiles"
      :download="downloadState"
      :can-download="canDownloadSd && !gpsDeleting"
      :deleting="gpsDeleting"
      :download-error="downloadError"
      :download-message="downloadMessage"
      @close="gpsDrawerVisible = false"
      @refresh="refreshGpsFiles"
      @download="downloadSdFile"
      @cancel-download="cancelDownload"
      @delete="requestDeleteGpsFile"
      @delete-all="requestDeleteAllGpsFiles"
    />

    <ConfirmDialog
      :visible="pendingDelete !== null"
      title="删除 GPS 文件"
      :message="deleteConfirmMessage"
      confirm-text="删除"
      danger
      @cancel="pendingDelete = null"
      @confirm="confirmDelete"
    />
  </div>
</template>

<style scoped>
.store-estimate {
  margin-top: 10px;
  padding: 9px 11px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-inset);
}

.store-estimate.active {
  border-color: #bfe3cd;
  background: var(--success-soft);
}

.estimate-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
  font-size: 11.5px;
  line-height: 1.9;
}

.estimate-label {
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.estimate-value {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  color: var(--text-primary);
  text-align: right;
}

.estimate-value.strong {
  color: var(--success);
}

.store-estimate .hint {
  margin-top: 5px;
  font-size: 10.5px;
}
</style>
