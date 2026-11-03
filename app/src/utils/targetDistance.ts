import type { PreviewFrame } from "../types";

/** 通道 ID：高增益 0xF0F0 */
const CH_HIGH_GAIN = 0xf0f0;
/** 通道 ID：PIN 主波 0xFFFF */
const CH_PIN = 0xffff;

/** 1 个采样点 = 1 ns；往返距离 c/2 = 0.15 m */
const METERS_PER_NS = 0.15;
/** 峰值至少高于空闲平均值 20% 才视为有效回波 */
const PEAK_OVER_IDLE = 1.2;

export interface TargetMeasurement {
  /** PIN 主波第一段峰值位置（ns，绝对采样位置） */
  startNs: number | null;
  /** 高增益通道第二段峰值位置（ns，绝对采样位置） */
  endNs: number | null;
  /** 目标距离（m）；起点或终点无效时为 null */
  distanceM: number | null;
}

/**
 * 在一段波形中找峰值点。
 *
 * 峰值需大于该段空闲信号平均值的 20%，否则认为没有有效回波。
 * 返回绝对采样位置（段起点 + 段内索引）。
 */
function segmentPeak(
  data: number[],
  start: number
): { pos: number; value: number } | null {
  if (data.length === 0) return null;

  let sum = 0;
  let max = -Infinity;
  let idx = -1;
  for (let i = 0; i < data.length; i++) {
    const v = data[i];
    sum += v;
    if (v > max) {
      max = v;
      idx = i;
    }
  }

  const idleAvg = sum / data.length;
  if (idx < 0 || max <= 0) return null;
  if (max < idleAvg * PEAK_OVER_IDLE) return null;

  return { pos: start + idx, value: max };
}

/**
 * 计算目标距离：
 * - 起点：PIN 主波通道第一段的峰值位置
 * - 终点：高增益通道第二段的峰值位置
 * - 距离：(终点 - 起点) × 0.15 m
 *
 * 任一峰值未超过空闲平均值 20% 时对应位置为 null。
 */
export function measureTargetDistance(frame: PreviewFrame): TargetMeasurement {
  const pin = frame.channels.find((ch) => ch.channel_id === CH_PIN);
  const highGain = frame.channels.find((ch) => ch.channel_id === CH_HIGH_GAIN);

  const start = pin
    ? segmentPeak(pin.seg0_data, pin.seg0_start)
    : null;
  const end = highGain
    ? segmentPeak(highGain.seg1_data, highGain.seg1_start)
    : null;

  const startNs = start ? start.pos : null;
  const endNs = end ? end.pos : null;
  const distanceM =
    startNs !== null && endNs !== null && endNs > startNs
      ? (endNs - startNs) * METERS_PER_NS
      : null;

  return { startNs, endNs, distanceM };
}
