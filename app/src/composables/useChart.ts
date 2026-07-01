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

    /** 添加一组数据点（每个通道一个值） */
    function pushData(values: number[]) {
        for (let ch = 0; ch < Math.min(channels, values.length); ch++) {
            buffers[ch][writeIdx] = values[ch];
        }
        writeIdx = (writeIdx + 1) % maxPoints;
        if (count < maxPoints) count++;
        if (!paused) dirty = true;
    }

    /** 获取按时间顺序排列的指定通道数据（从旧到新） */
    function getChannelData(ch: number): Float64Array {
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

                // 收集所有通道数据
                const allData: Float64Array[] = [];
                for (let ch = 0; ch < channels; ch++) {
                    allData.push(getChannelData(ch));
                }

                // 在离屏 canvas 上渲染
                onRender(allData, offscreen.width, offscreen.height);

                // 复制到主 canvas
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

    /** 暂停/恢复刷新。暂停时数据仍在写入，但画面冻结 */
    function setPaused(value: boolean) {
        paused = value;
    }

    function isPaused() { return paused; }

    /** 手动触发一次渲染（暂停模式下使用） */
    function forceRender() {
        dirty = true;
    }

    return {
        pushData,
        clear,
        setChannelCount,
        renderLoop,
        stop,
        markDirty,
        getChannelData,
        setPaused,
        forceRender,
    };
}
