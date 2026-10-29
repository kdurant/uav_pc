import type { PreviewFrame } from "../types";

/**
 * 把一帧原始预览数据转换为按绝对采样位置对齐的序列。
 *
 * series[ch][i] 对应绝对采样位置 i 的值（索引 = 采样位置，从 0 开始），NaN 表示无数据。
 * 数组长度为两段提取区间的最大终点。返回 null 表示该帧没有可用数据。
 */
export function frameToSeries(frame: PreviewFrame, channelCount = 4): number[][] | null {
    // 最大终点（第二段提取位置 + 第二段提取长度）
    let dataEnd = 0;
    for (const ch of frame.channels) {
        if (ch.seg0_data.length > 0) {
            dataEnd = Math.max(dataEnd, ch.seg0_start + ch.seg0_data.length);
        }
        if (ch.seg1_data.length > 0) {
            dataEnd = Math.max(dataEnd, ch.seg1_start + ch.seg1_data.length);
        }
    }
    if (dataEnd <= 0) return null;

    // 数组索引 = 绝对采样位置：从 0 到 dataEnd，无数据处为 NaN
    const series: number[][] = [];
    for (let c = 0; c < channelCount; c++) {
        const ch = frame.channels[c];
        const arr = new Array<number>(dataEnd).fill(NaN);
        if (ch) {
            for (let i = 0; i < ch.seg0_data.length; i++) {
                const pos = ch.seg0_start + i;
                if (pos >= 0 && pos < dataEnd) arr[pos] = ch.seg0_data[i];
            }
            for (let i = 0; i < ch.seg1_data.length; i++) {
                const pos = ch.seg1_start + i;
                if (pos >= 0 && pos < dataEnd) arr[pos] = ch.seg1_data[i];
            }
        }
        series.push(arr);
    }

    return series;
}
