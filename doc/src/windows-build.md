# 在 Linux 上交叉编译 Windows exe

目标：在 Linux 上生成单个 `uav-pc.exe`，复制到 Windows 后直接运行，不需要安装程序。

> Tauri 官方不支持从 Linux 交叉打包 MSI/NSIS 安装程序。本文只构建 **portable exe**；如果需要安装包，请使用 Windows 机器或 GitHub Actions 的 `windows-latest` runner。

## 环境要求

- Rust（rustup）
- MSVC 目标：

  ```bash
  rustup target add x86_64-pc-windows-msvc
  ```

- 交叉编译工具：

  ```bash
  cargo install cargo-xwin
  ```

- clang / LLVM（提供 `llvm-rc`，用于嵌入 Windows 图标和清单）：

  ```bash
  sudo apt install clang llvm
  ```

- Node.js >= 18

首次执行 `cargo xwin build` 时，`cargo-xwin` 会自动下载 MSVC CRT/SDK，需要网络，耗时取决于网速。

## 构建步骤

### 1. 生成 Windows 图标（仅首次）

仓库需要 `src-tauri/icons/icon.ico` 才能嵌入 Windows 图标。如果还没有：

```bash
cd app
npx tauri icon src-tauri/icons/icon.png
```

会同时生成 `icon.ico`、`32x32.png`、`128x128.png` 等文件。

### 2. 构建前端

```bash
npm run build
```

Tauri 会把 `dist/` 内嵌进 exe，因此必须在 cargo 编译前完成。

### 3. 交叉编译

```bash
cd app/src-tauri

# llvm-rc 在部分发行版中带版本后缀（如 llvm-rc-14），这里做一次临时软链接
mkdir -p /tmp/uav-xwin-bin
ln -sf "$(command -v llvm-rc || command -v llvm-rc-14)" /tmp/uav-xwin-bin/llvm-rc

PATH=/tmp/uav-xwin-bin:$PATH \
RUSTFLAGS="-C target-feature=+crt-static" \
cargo xwin build --release --target x86_64-pc-windows-msvc
```

也可以直接运行脚本：

```bash
cd app
./scripts/build-windows.sh
```

### 4. 产物位置

```text
app/src-tauri/target/x86_64-pc-windows-msvc/release/uav-pc.exe
```

确认产物类型：

```bash
file app/src-tauri/target/x86_64-pc-windows-msvc/release/uav-pc.exe
# PE32+ executable (GUI) x86-64, for MS Windows
```

## Windows 端使用

1. **放到可写目录**
   例如桌面、下载目录、D 盘。程序首次启动会在 exe 同目录创建 `config.toml`，放在 `Program Files` 下会因无写权限失败。

2. **不要复制 Linux 的配置文件**
   Windows 端会自动生成默认配置（`broadcast_search = true`）。
   如果复制了 Linux 的 `config.toml`（尤其是 `broadcast_search = false` + 固定 `device_ip`），程序会尝试连接该 IP，可能表现为“连接失败”。

3. **需要 WebView2 Runtime**
   Windows 10/11 通常已自带。若提示缺失，安装 Microsoft Edge WebView2 Runtime。
   单个 exe 无法内置 WebView2，这是 Tauri 的运行时依赖。

4. **防火墙**
   首次运行会弹出 Windows 防火墙授权，需要允许 UDP（默认本地 12345 / 设备 6666），否则搜索不到设备。

5. **SmartScreen**
   exe 未签名，首次运行可能提示“未知发布者”，选择“更多信息 → 仍要运行”。

## 静态 CRT 说明

默认的 MSVC 构建会依赖 `VCRUNTIME140.dll`。加上：

```bash
RUSTFLAGS="-C target-feature=+crt-static"
```

后会把 C 运行库静态链接进 exe，生成约 12 MB 的单文件，不再依赖 VC++ Redistributable。

链接时可能出现 `LNK4099`（找不到 MSVC 库的 PDB）警告，不影响产物，可以忽略。

## 限制

- 只生成 portable exe，不包含安装器、开始菜单快捷方式和卸载入口
- 无法把 WebView2 运行时打进单个 exe
- 未做代码签名
- 仅支持 x86_64；如需 arm64 Windows，需要单独的 target 和工具链

如需正式安装包，建议使用 GitHub Actions 的 Windows runner 或 Windows 虚拟机执行 `npm run tauri build`。
