/**
 * 高性能折线图数据管理 — 环形缓冲区 + requestAnimationFrame 渲染
 */

export interface ChartOptions {
    /** 最大数据点数 */
    maxPoints: number;
    /** 通道数 */
    channels: number;
    /** 渲染回调 */
    onRender: (buffers: Float64Array[], width: number, height: number) => void;
}

export function useChart(options: ChartOptions) {
    const { maxPoints, channels, onRender } = options;

    // 环形缓冲区（每个通道一个 Float64Array）
    const buffers: Float64Array[] = [];
    for (let i = 0; i < channels; i++) {
        buffers.push(new Float64Array(maxPoints));
    }

    // 写入指针
    let writeIdx = 0;
    // 数据计数
    let count = 0;
    // 帧动画 ID
    let rafId = 0;
    // 是否有新数据需要渲染
    let dirty = false;
    // 是否暂停刷新（冻结画面）
    let paused = false;
    // 暂停时的冻结数据快照
    let frozenData: Float64Array[] | null = null;
    // 位置对齐模式：整帧替换，缓冲区索引对应采样点位置（xStart 为起始位置）
    let frameMode = false;
    let frameStart = 0;

    /** 添加一组数据点（每个通道一个值） */
    function pushData(values: number[]) {
        for (let ch = 0; ch < Math.min(channels, values.length); ch++) {
            buffers[ch][writeIdx] = values[ch];
        }
        writeIdx = (writeIdx + 1) % maxPoints;
        if (count < maxPoints) count++;
        if (!paused) dirty = true;
    }

    /**
     * 用一整帧数据替换缓冲区（位置对齐模式）。
     * series[ch][i] 对应该通道采样点位置 xStart + i 的值，NaN 表示该位置无数据（空档）。
     */
    function setFrame(series: number[][], xStart: number) {
        frameMode = true;
        frameStart = xStart;
        const len = series.reduce((m, s) => Math.max(m, s.length), 0);
        for (let ch = 0; ch < channels; ch++) {
            if (buffers[ch].length < len) buffers[ch] = new Float64Array(len);
            const src = series[ch] || [];
            for (let i = 0; i < len; i++) {
                buffers[ch][i] = i < src.length ? src[i] : NaN;
            }
        }
        count = len;
        writeIdx = 0;
        if (!paused) dirty = true;
    }

    /** 获取按时间顺序排列的指定通道数据（从旧到新） */
    function getChannelData(ch: number): Float64Array {
        if (frameMode) {
            return buffers[ch].subarray(0, Math.min(count, buffers[ch].length));
        }
        if (count < maxPoints) {
            return buffers[ch].subarray(0, count);
        }
        // 环形缓冲：先 tail 部分，再 head 部分
        const result = new Float64Array(maxPoints);
        const tail = maxPoints - writeIdx;
        result.set(buffers[ch].subarray(writeIdx), 0);
        result.set(buffers[ch].subarray(0, writeIdx), tail);
        return result;
    }

    /** 清空数据 */
    function clear() {
        for (let i = 0; i < channels; i++) {
            buffers[i].fill(0);
        }
        writeIdx = 0;
        count = 0;
        frameMode = false;
        dirty = true;
    }

    /** 设置通道数 */
    function setChannelCount(n: number) {
        while (buffers.length < n) {
            buffers.push(new Float64Array(maxPoints));
        }
        buffers.length = n;
        clear();
    }

    /** 渲染循环 */
    function renderLoop(canvas: HTMLCanvasElement, offscreen: HTMLCanvasElement) {
        const ctx = canvas.getContext("2d")!;
        const offCtx = offscreen.getContext("2d")!;

        function frame() {
            if (dirty) {
                dirty = false;
                overlayDirty = false;

                // 收集所有通道数据（暂停时使用冻结快照）
                const allData: Float64Array[] = [];
                for (let ch = 0; ch < channels; ch++) {
                    allData.push(frozenData ? frozenData[ch] : getChannelData(ch));
                }

                // 在离屏 canvas 上渲染（包括数据和 overlay）
                onRender(allData, offscreen.width, offscreen.height);

                // 复制到主 canvas
                ctx.clearRect(0, 0, canvas.width, canvas.height);
                ctx.drawImage(offscreen, 0, 0);
            } else if (overlayDirty) {
                overlayDirty = false;

                // 暂停时仅刷新 overlay（数据不变，重新绘制光标/框选）
                const allData: Float64Array[] = [];
                for (let ch = 0; ch < channels; ch++) {
                    allData.push(frozenData ? frozenData[ch] : getChannelData(ch));
                }
                onRender(allData, offscreen.width, offscreen.height);

                ctx.clearRect(0, 0, canvas.width, canvas.height);
                ctx.drawImage(offscreen, 0, 0);
            }
            rafId = requestAnimationFrame(frame);
        }

        rafId = requestAnimationFrame(frame);
    }

    /** 停止渲染 */
    function stop() {
        if (rafId) {
            cancelAnimationFrame(rafId);
            rafId = 0;
        }
    }

    /** 标记脏数据，触发重绘（用于 resize 后） */
    function markDirty() {
        dirty = true;
    }

    /** 仅标记 overlay 脏（鼠标移动等，暂停时不触发数据重绘） */
    function markOverlayDirty() {
        overlayDirty = true;
    }
    let overlayDirty = false;

    /** 暂停/恢复刷新。暂停时冻结数据快照，画面冻结 */
    function setPaused(value: boolean) {
        paused = value;
        if (paused) {
            // 冻结当前数据快照
            frozenData = [];
            for (let ch = 0; ch < channels; ch++) {
                frozenData.push(new Float64Array(getChannelData(ch)));
            }
        } else {
            frozenData = null;
            dirty = true;
        }
    }

    function isPaused() { return paused; }

    /** 手动触发一次渲染（暂停模式下使用） */
    function forceRender() {
        dirty = true;
    }

    return {
        pushData,
        setFrame,
        clear,
        setChannelCount,
        renderLoop,
        stop,
        markDirty,
        markOverlayDirty,
        getChannelData,
        setPaused,
        forceRender,
    };
}
