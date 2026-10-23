/**
 * 预览帧二进制解码。
 *
 * 与 Rust 端 `preview::PreviewFrame::to_binary` 对应（小端）：
 * magic(4) + gps_week(4) + gps_second(f64) + sub_time(4) +
 * az/pitch/roll/lat/lon/alt(6×f64) +
 * encoder_bits/encoder_value/wave_channels/wave_length(4×u32) + channel_count(4) +
 * 每通道: channel_id/seg0_start/seg0_len(u16) + seg0_data(u16×len)
 *        + seg1_start/seg1_len(u16) + seg1_data(u16×len)
 */
import type { ChannelData, PreviewFrame } from "../types";

const MAGIC = 0x50525631; // 'PRV1'
const HEADER_LEN = 88;

export function decodePreviewFrame(bytes: Uint8Array): PreviewFrame | null {
  if (bytes.byteLength < HEADER_LEN) return null;

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let off = 0;

  if (view.getUint32(off, true) !== MAGIC) return null;
  off += 4;

  const gps_week = view.getUint32(off, true);
  off += 4;
  const gps_second = view.getFloat64(off, true);
  off += 8;
  const sub_time = view.getUint32(off, true);
  off += 4;
  const azimuth = view.getFloat64(off, true);
  off += 8;
  const pitch = view.getFloat64(off, true);
  off += 8;
  const roll = view.getFloat64(off, true);
  off += 8;
  const latitude = view.getFloat64(off, true);
  off += 8;
  const longitude = view.getFloat64(off, true);
  off += 8;
  const altitude = view.getFloat64(off, true);
  off += 8;
  const encoder_bits = view.getUint32(off, true);
  off += 4;
  const encoder_value = view.getUint32(off, true);
  off += 4;
  const wave_channels = view.getUint32(off, true);
  off += 4;
  const wave_length = view.getUint32(off, true);
  off += 4;
  const channelCount = view.getUint32(off, true);
  off += 4;

  const channels: ChannelData[] = [];
  const limit = bytes.byteLength;

  for (let c = 0; c < channelCount; c++) {
    if (off + 6 > limit) return null;
    const channel_id = view.getUint16(off, true);
    off += 2;
    const seg0_start = view.getUint16(off, true);
    off += 2;
    const seg0_len = view.getUint16(off, true);
    off += 2;

    if (off + seg0_len * 2 > limit) return null;
    const seg0_data = new Array<number>(seg0_len);
    for (let i = 0; i < seg0_len; i++) {
      seg0_data[i] = view.getUint16(off, true);
      off += 2;
    }

    if (off + 4 > limit) return null;
    const seg1_start = view.getUint16(off, true);
    off += 2;
    const seg1_len = view.getUint16(off, true);
    off += 2;

    if (off + seg1_len * 2 > limit) return null;
    const seg1_data = new Array<number>(seg1_len);
    for (let i = 0; i < seg1_len; i++) {
      seg1_data[i] = view.getUint16(off, true);
      off += 2;
    }

    channels.push({
      channel_id,
      seg0_start,
      seg0_len,
      seg0_data,
      seg1_start,
      seg1_len,
      seg1_data,
    });
  }

  return {
    gps_week,
    gps_second,
    sub_time,
    azimuth,
    pitch,
    roll,
    latitude,
    longitude,
    altitude,
    encoder_bits,
    encoder_value,
    wave_channels,
    wave_length,
    channels,
  };
}
