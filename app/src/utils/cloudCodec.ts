/**
 * 点云二进制解码（与 Rust `pointcloud::CloudResult::encode` 对应，小端）。
 *
 * 布局：
 * magic "CLD1"(4) + point_count(u32) + dem_w(u32) + dem_h(u32) +
 * dem_x0(f32) + dem_y0(f32) + dem_cell(f32) + z_min(f32) + z_max(f32) +
 * surface_points(u32) + bottom_points(u32) + t0(f32) + frame_count(u32) + truncated(u32) +
 * points[n][x,y,z,intensity](f32) + dem[w*h](f32, NaN 表示无数据)
 */

export interface CloudData {
    /** 点坐标与强度，每点 4 个 float：[x, y, z, intensity] */
    points: Float32Array;
    /** 点数 */
    pointCount: number;
    /** DEM 栅格尺寸与起点 */
    demWidth: number;
    demHeight: number;
    demX0: number;
    demY0: number;
    demCell: number;
    /** DEM 高程（NaN = 无数据） */
    dem: Float32Array;
    zMin: number;
    zMax: number;
    surfacePoints: number;
    bottomPoints: number;
    /** 自动解算的窗口时间偏移（样点） */
    t0: number;
    frameCount: number;
    /** 点数超过上限被截断 */
    truncated: boolean;
}

const HEADER_LEN = 52;

export function decodeCloud(bytes: Uint8Array): CloudData | null {
    if (bytes.byteLength < HEADER_LEN) return null;
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);

    const magic = String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3]);
    if (magic !== "CLD1") return null;

    let off = 4;
    const pointCount = view.getUint32(off, true);
    off += 4;
    const demWidth = view.getUint32(off, true);
    off += 4;
    const demHeight = view.getUint32(off, true);
    off += 4;
    const demX0 = view.getFloat32(off, true);
    off += 4;
    const demY0 = view.getFloat32(off, true);
    off += 4;
    const demCell = view.getFloat32(off, true);
    off += 4;
    const zMin = view.getFloat32(off, true);
    off += 4;
    const zMax = view.getFloat32(off, true);
    off += 4;
    const surfacePoints = view.getUint32(off, true);
    off += 4;
    const bottomPoints = view.getUint32(off, true);
    off += 4;
    const t0 = view.getFloat32(off, true);
    off += 4;
    const frameCount = view.getUint32(off, true);
    off += 4;
    const truncated = view.getUint32(off, true) !== 0;
    off += 4;

    const pointFloats = pointCount * 4;
    if (off + pointFloats * 4 + demWidth * demHeight * 4 > bytes.byteLength) return null;

    // 复制一份独立的缓冲区，避免与 IPC 缓冲区共享导致被回收
    const points = new Float32Array(bytes.buffer.slice(bytes.byteOffset + off, bytes.byteOffset + off + pointFloats * 4));
    off += pointFloats * 4;

    const dem = new Float32Array(bytes.buffer.slice(bytes.byteOffset + off, bytes.byteOffset + off + demWidth * demHeight * 4));

    return {
        points,
        pointCount,
        demWidth,
        demHeight,
        demX0,
        demY0,
        demCell,
        dem,
        zMin,
        zMax,
        surfacePoints,
        bottomPoints,
        t0,
        frameCount,
        truncated,
    };
}
