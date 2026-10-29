<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { CloudData } from "../utils/cloudCodec";
import type { CloudDisplay } from "../composables/usePointCloud";

const props = withDefaults(
    defineProps<{
        data?: CloudData | null;
        display?: CloudDisplay;
        /** 垂直放大倍数（相对水面） */
        zScale?: number;
    }>(),
    { data: null, display: "both", zScale: 1 }
);

const hostRef = ref<HTMLDivElement | null>(null);

let renderer: THREE.WebGLRenderer | null = null;
let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;
let controls: OrbitControls | null = null;
let resizeObserver: ResizeObserver | null = null;
/** 当前数据生成的临时对象，切换数据时统一释放 */
let objects: THREE.Object3D[] = [];

/** 高程色带：深蓝 → 青 → 绿 → 黄 → 红 */
function depthColor(t: number): [number, number, number] {
    const stops: Array<[number, number, number]> = [
        [0.05, 0.2, 0.55],
        [0.1, 0.6, 0.85],
        [0.2, 0.8, 0.4],
        [0.95, 0.85, 0.2],
        [0.9, 0.25, 0.2],
    ];
    const x = Math.min(Math.max(t, 0), 1) * (stops.length - 1);
    const i = Math.min(Math.floor(x), stops.length - 2);
    const f = x - i;
    return [
        stops[i][0] + (stops[i + 1][0] - stops[i][0]) * f,
        stops[i][1] + (stops[i + 1][1] - stops[i][1]) * f,
        stops[i][2] + (stops[i + 1][2] - stops[i][2]) * f,
    ];
}

function disposeObjects() {
    for (const obj of objects) {
        scene?.remove(obj);
        const mesh = obj as THREE.Mesh;
        mesh.geometry?.dispose();
        const mat = mesh.material as THREE.Material | THREE.Material[] | undefined;
        if (Array.isArray(mat)) mat.forEach((m) => m.dispose());
        else mat?.dispose();
    }
    objects = [];
}

/** 高程缩放：以水面（zMax）为基准做垂直放大 */
function scaleZ(z: number, zTop: number, k: number): number {
    return zTop + (z - zTop) * k;
}

function addPoints(d: CloudData, zTop: number, k: number) {
    if (!scene || !d.pointCount) return;
    const positions = new Float32Array(d.pointCount * 3);
    const colors = new Float32Array(d.pointCount * 3);
    const range = Math.max(1e-3, d.zMax - d.zMin);
    for (let i = 0; i < d.pointCount; i++) {
        const x = d.points[i * 4];
        const y = d.points[i * 4 + 1];
        const z = d.points[i * 4 + 2];
        positions[i * 3] = x;
        positions[i * 3 + 1] = y;
        positions[i * 3 + 2] = scaleZ(z, zTop, k);
        const [r, g, b] = depthColor((z - d.zMin) / range);
        colors[i * 3] = r;
        colors[i * 3 + 1] = g;
        colors[i * 3 + 2] = b;
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));

    // 点大小随场景尺度自适应
    const extent = Math.max(1, Math.max(d.demWidth * d.demCell, d.demHeight * d.demCell));
    const material = new THREE.PointsMaterial({
        size: Math.max(0.15, extent / 500),
        vertexColors: true,
        sizeAttenuation: true,
    });
    const points = new THREE.Points(geometry, material);
    scene.add(points);
    objects.push(points);
}

function addDem(d: CloudData, zTop: number, k: number) {
    if (!scene || d.demWidth < 2 || d.demHeight < 2) return;
    const w = d.demWidth;
    const h = d.demHeight;
    const positions = new Float32Array(w * h * 3);
    const colors = new Float32Array(w * h * 3);
    const filled = new Float32Array(w * h);
    const range = Math.max(1e-3, d.zMax - d.zMin);

    // 先把空网格用整体中位数填充，避免出现空洞
    const valid: number[] = [];
    for (let i = 0; i < d.dem.length; i++) {
        if (Number.isFinite(d.dem[i])) valid.push(d.dem[i]);
    }
    valid.sort((a, b) => a - b);
    const fallback = valid.length ? valid[Math.floor(valid.length / 2)] : d.zMin;

    for (let j = 0; j < h; j++) {
        for (let i = 0; i < w; i++) {
            const idx = j * w + i;
            const v = d.dem[idx];
            const z = Number.isFinite(v) ? v : fallback;
            filled[idx] = z;
            positions[idx * 3] = d.demX0 + (i + 0.5) * d.demCell;
            positions[idx * 3 + 1] = d.demY0 + (j + 0.5) * d.demCell;
            positions[idx * 3 + 2] = scaleZ(z, zTop, k);
            const [r, g, b] = depthColor((z - d.zMin) / range);
            colors[idx * 3] = r;
            colors[idx * 3 + 1] = g;
            colors[idx * 3 + 2] = b;
        }
    }

    const indices: number[] = [];
    for (let j = 0; j < h - 1; j++) {
        for (let i = 0; i < w - 1; i++) {
            const a = j * w + i;
            const b = a + 1;
            const c = a + w;
            const dd = c + 1;
            indices.push(a, b, c, b, dd, c);
        }
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
    geometry.setIndex(indices);
    geometry.computeVertexNormals();

    const material = new THREE.MeshLambertMaterial({
        vertexColors: true,
        side: THREE.DoubleSide,
        flatShading: false,
    });
    const mesh = new THREE.Mesh(geometry, material);
    scene.add(mesh);
    objects.push(mesh);
}

/** 让相机框住整个点云 */
function frameCamera(d: CloudData) {
    if (!camera || !controls) return;
    const cx = d.demX0 + (d.demWidth * d.demCell) / 2;
    const cy = d.demY0 + (d.demHeight * d.demCell) / 2;
    const cz = (d.zMin + d.zMax) / 2;
    const radius = Math.max(
        5,
        Math.max(d.demWidth * d.demCell, d.demHeight * d.demCell, d.zMax - d.zMin)
    );
    controls.target.set(cx, cy, cz);
    camera.position.set(cx, cy - radius * 1.1, cz + radius * 0.9);
    camera.near = Math.max(0.05, radius / 1000);
    camera.far = radius * 50;
    camera.updateProjectionMatrix();
    controls.update();
}

function rebuild() {
    if (!scene) return;
    disposeObjects();
    const d = props.data;
    if (!d) {
        clearSceneObjects();
        return;
    }
    const zTop = d.zMax;
    const k = Math.min(20, Math.max(0.2, props.zScale));
    if (props.display !== "dem") addPoints(d, zTop, k);
    if (props.display !== "points") addDem(d, zTop, k);
    frameCamera(d);
}

function clearSceneObjects() {
    if (!scene) return;
    disposeObjects();
    if (camera && controls) {
        controls.target.set(0, 0, 0);
        camera.position.set(20, -30, 25);
        controls.update();
    }
}

function resize() {
    const host = hostRef.value;
    if (!host || !renderer || !camera) return;
    const width = host.clientWidth;
    const height = host.clientHeight;
    if (width === 0 || height === 0) return;
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
}

onMounted(() => {
    const host = hostRef.value;
    if (!host) return;

    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0b1220);

    camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100000);
    camera.up.set(0, 0, 1);

    renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    host.appendChild(renderer.domElement);
    renderer.domElement.style.display = "block";
    renderer.domElement.style.width = "100%";
    renderer.domElement.style.height = "100%";

    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;

    scene.add(new THREE.AmbientLight(0xffffff, 1.5));
    const dir = new THREE.DirectionalLight(0xffffff, 1.8);
    dir.position.set(0.4, -0.8, 1);
    scene.add(dir);

    resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(host);
    resize();

    renderer.setAnimationLoop(() => {
        controls?.update();
        if (renderer && scene && camera) renderer.render(scene, camera);
    });

    rebuild();
});

watch(
    () => [props.data, props.display, props.zScale],
    () => rebuild()
);

onUnmounted(() => {
    resizeObserver?.disconnect();
    renderer?.setAnimationLoop(null);
    disposeObjects();
    controls?.dispose();
    renderer?.dispose();
    if (renderer?.domElement.parentElement) {
        renderer.domElement.parentElement.removeChild(renderer.domElement);
    }
    renderer = null;
    scene = null;
    camera = null;
    controls = null;
});
</script>

<template>
    <div ref="hostRef" class="cloud-host">
        <div v-if="!data" class="empty-tip">选择文件后点击「生成点云」</div>
    </div>
</template>

<style scoped>
.cloud-host {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 0;
    background: #0b1220;
    overflow: hidden;
}

.empty-tip {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #7c8aa5;
    font-size: 13px;
    pointer-events: none;
}
</style>
