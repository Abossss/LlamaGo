# LlamaGo

> 将一行命令压缩成一次点击 —— 为 [llama.cpp](https://github.com/ggml-org/llama.cpp) 打造的桌面启动器

<!-- ![LlamaGo](app-icon.png) -->

**告别终端**，用图形界面管理 `LlamaGo` 的生命周期：扫描、启动、守护、日志，一切尽在掌握。

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2021-%23dea584" alt="Rust">
  <img src="https://img.shields.io/badge/Tauri-2.x-24C8DB" alt="Tauri">
    <img src="https://img.shields.io/badge/llama.cpp-enabled-%23FF7A00" alt="llama.cpp">
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11%20%7C%20Linux%20%7C%20macOS-lightgrey" alt="支持 Windows、Linux、macOS">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT">
  <img src="https://img.shields.io/badge/release-v0.1.0-blue" alt="v0.1.0">
  <img src="https://img.shields.io/badge/vue-3.x-%2342b883" alt="vue 3">
</p>

---

## 特性一览

- **一键启停**：点击按钮启动/停止 `llama-server`，进程生命周期由应用托管，关闭窗口不影响后台服务。
- **自动扫描**：自动发现 `llama.cpp` 安装目录与 `.gguf` 模型文件（支持环境变量、目录选择和自动扫描三重机制）。
- **WebUI 直达**：服务器运行后，一键在默认浏览器打开 `http://127.0.0.1:8080`。
- **系统托盘常驻**：关闭窗口最小化至托盘，服务持续运行。托盘菜单可直接启停服务、打开 WebUI 或退出。
- **实时日志面板**：stdout/stderr 逐行滚动显示，保留最近 **200 行**环形缓冲，窗口重新打开自动回填。
- **单实例保护**：同一时间只允许一个实例，避免端口冲突。

---

## 快速开始

### 安装

从 [Releases](https://github.com/Abossss/LlamaGo/releases) 下载对应安装包：

| 包类型 | 文件                          | 适用场景           |
| ------ | ----------------------------- | ------------------ |
| NSIS   | `LlamaGo_0.1.0_x64-setup.exe` | 图形化安装（推荐） |
| MSI    | `LlamaGo_0.1.0_x64.msi`       | 企业 / 静默部署    |

> 要求：Windows 10/11（WebView2 已内置）；Linux（WebKitGTK）；macOS（WKWebView）。

### 配置路径

1. 打开「设置」→ 点击「**自动扫描**」自动查找 `llama-server`。
2. 设置环境变量 `LLAMA_CPP_PATH` 和 `LLAMA_CPP_MODELS_PATH`（点击保存后持久化），或手动选择目录。
3. 切换到「启动」视图，模型下拉列表会自动填充。

### 启动服务

1. 选择一个 `.gguf` 模型（首次使用需配置路径）。
2. 点击「**启动服务器**」。
3. 日志中出现 `server is listening` 即表示就绪，点击「🌐 WebUI」访问。

> **端口冲突**：默认 8080 被占用时会提示失败，请释放端口或调整服务参数（参数调整功能待后续版本支持）。

---

## 核心机制

### 进程管理

```
[前端] → start_server → [后端 spawn] → llama-server
                                       ├─ stdout → 日志面板
                                       └─ stderr → 日志面板（加前缀）
```

- 启动时 **预检端口**，避免不必要的启动。
- 运行中每 300ms 轮询进程状态，自动感知退出并更新界面。
- 停止策略：Unix 先 SIGTERM，5 秒后 SIGKILL；Windows 直接终止进程。

### 路径解析优先级

`llama-server` 可执行文件路径由后端从 `LLAMA_CPP_PATH` 推导（前端不可篡改）：

1. **环境变量 `LLAMA_CPP_PATH`**（最高优先级，持久化）
2. 可执行文件所在目录 + PATH 扫描（兜底）
3. 引导用户手动设置（若均未找到）

模型扫描递归查找 `.gguf` 文件，深度上限 20 层，自动去重并跳过符号链接环。

### 日志与状态

- 双管道独立线程转发，stderr 行添加 `[stderr]` 前缀。
- 环形缓冲区保留最近 200 行，窗口唤回时通过 `get_log_history` 补全。
- 应用日志（`debug.log`）记录内部诊断信息，位于应用数据目录。

### 单实例互斥

使用命名互斥体 `Local\LlamaGo.SingleInstance`，第二个实例自动退出。

---

## 用户界面

### 界面预览

LlamaGo 客户端分为「设置配置页」和「启动运行页」两大功能面板，左侧为深色侧边栏（含 Lg 图标、播放、齿轮等入口），主体为米白色卡片式布局，风格简约、分区清晰，用来可视化配置并启动 llama.cpp 模型服务，免去手动敲命令行。

**设置配置页**

配置 `llama.cpp` 运行环境，页面备注说明：配置会写入 Windows 用户环境变量，重启软件后生效。

- **llama.cpp 目录**：填写 `llama-server` 所在目录（如 `D:\Program Files\llama.cpp\llama`），路径校验显示「√有效」，右侧带「浏览」「保存」按钮，对应环境变量 `LLAMA_CPP_PATH`。
- **models 目录（模型文件夹）**：填写 `.gguf` 模型存放目录（如 `D:\Program Files\llama.cpp\llama\models`），同样校验「√有效」并带浏览、保存按钮，对应环境变量 `LLAMA_CPP_MODELS_PATH`。

<p align="center"><img src="docs/image/setting.png" width="720"></p>

**启动运行页**

选择 `.gguf` 模型、运行 `llama-server` 服务，并实时查看运行日志。

- **服务器配置**：自动识别出 `llama-server.exe` 完整路径（由设置页配置的环境变量读取）；模型下拉框选择 `models` 目录中的 `.gguf` 模型文件；下方为「启动服务器」按钮及「停止」按钮。
- **运行日志**：底部黑色日志框，启动服务后实时打印运行日志、加载进度与报错信息（当前显示「等待日志输出...」）。

<p align="center"><img src="docs/image/splash.png" width="720"></p>

**工作流程**：先在设置页配置好 `llama.cpp` 主目录与模型目录（校验有效并保存环境变量）→ 切换到启动页，下拉框自动读取 `models` 目录中的模型 → 选中模型点击「启动服务器」，即可拉起 llama.cpp 的 Web API 服务。

### 快捷键

| 按键     | 功能                         |
| -------- | ---------------------------- |
| `1`      | 切换到「设置」视图           |
| `2`      | 切换到「启动」视图           |
| 窗口关闭 | 最小化到托盘（服务继续运行） |
| 托盘左键 | 唤出主窗口                   |

> 快捷键仅在焦点不在输入框时生效。

### 托盘菜单

| 菜单项         | 行为                                       |
| -------------- | ------------------------------------------ |
| 状态行（禁用） | 显示「服务器: 运行中 (PID x)」或「已停止」 |
| 启动 / 停止    | 一键切换；启动时优先使用上次保存的模型     |
| 打开 WebUI     | 仅服务运行时可用                           |
| 显示主窗口     | 唤回主界面                                 |
| 退出           | **先停服务，再退出进程**                   |

---

## 开发与构建

### 依赖

- Rust（稳定版）
- Node.js（仅用于 Tauri CLI，前端无 npm 打包）
- 平台 WebView2 / WebKitGTK / WKWebView

### 构建命令

```sh
cd src-tauri
cargo tauri dev     # 开发模式（前端热重载自 ../frontend）
cargo tauri build   # 生产构建，安装包输出至 target/release/bundle/
```

> 前端为单页 HTML，Vue 3 通过 CDN 引入，无需额外打包步骤。

---

## 接口清单

### 后端命令

| 命令               | 作用                      | 返回                           |
| ------------------ | ------------------------- | ------------------------------ |
| `start_server`     | 启动 `llama-server`       | PID 字符串                     |
| `stop_server`      | 停止服务                  | 终止信息                       |
| `get_status`       | 查询运行状态              | `(running, pid, port)`         |
| `check_config`     | 读取上次配置              | exec / model 路径              |
| `save_config`      | 保存模型选择              | —                              |
| `get_env_settings` | 读取环境变量及有效性      | `llama_valid` / `models_valid` |
| `save_env_var`     | 校验并持久化环境变量      | —                              |
| `scan_llama_cpp`   | 自动扫描 `llama.cpp` 目录 | 路径列表                       |
| `cancel_scan`      | 取消扫描                  | —                              |
| `list_models`      | 递归列出 `.gguf`（排序）  | 路径列表                       |
| `get_log_history`  | 获取最近 200 行日志       | 日志数组                       |

### 前端事件

| 事件                         | 触发时机           |
| ---------------------------- | ------------------ |
| `log-line`                   | 每行 stdout/stderr |
| `process-exit`               | 进程退出           |
| `server-started`             | spawn 成功         |
| `server-status`              | 运行状态变化       |
| `scan-message` / `scan-done` | 扫描进度 / 完成    |
| `toast`                      | 全局通知           |

---

## 已知局限

- **macOS 未经实测**：由于作者没有 Mac，无法对 macOS 平台进行实际测试，理论上支持但可能存在未知问题，敬请见谅。欢迎有 Mac 的朋友反馈问题或提交修复。
- **参数不可调**：上下文长度、GPU 层数等 `llama-server` 参数暂未开放 UI 配置，后续版本将支持。
- **`setx` 生效延迟**：环境变量保存后，本进程即时生效，但已打开的其它终端需重启。
- **Windows 停止非优雅**：直接调用 `TerminateProcess`，不会等待 `llama-server` 落盘。
- **托盘启动自动选模型**：若未保存过模型，会选用字典序第一个 `.gguf`，可能不符合预期。
- **单实例作用域**：互斥体基于会话（`Local\`），多用户 / 远程桌面各自独立。

---

## 法律与引用

- **LlamaGo 源码**：MIT 许可证（见 [LICENSE](LICENSE)）。
- **特别感谢 [llama.cpp](https://github.com/ggml-org/llama.cpp)**：本项目离不开 llama.cpp 强大的本地推理能力，向其作者与社区致敬。
- **`llama-server` / WebUI**：属于 [llama.cpp](https://github.com/ggml-org/llama.cpp)（MIT），版权归原项目。
- **`.gguf` 模型**：版权归各模型作者，用户自行选择并遵守相应许可。
- **Vue 3 (CDN)**：MIT。

如需引用：

```bibtex
@software{llamago,
  title        = {LlamaGo: A Desktop Launcher for llama.cpp},
  author       = {LlamaGo Contributors},
  year         = {2026},
  url          = {https://github.com/Abossss/LlamaGo},
  note         = {Tauri desktop GUI managing the llama-server lifecycle}
}
```

---

## 贡献

欢迎提交 Issue 和 Pull Request。请保持代码风格一致（无强制 lint 配置）。

---

## English Summary

**LlamaGo** is a Tauri-based desktop launcher for llama.cpp's `llama-server`. It provides:

- One‑click start/stop with process lifecycle management.
- Auto‑scan for llama.cpp and `.gguf` models.
- WebUI launcher (`http://127.0.0.1:8080`).
- System tray residency with live log panel.
- Single‑instance protection.

### Quick Start

1. Set `LLAMA_CPP_PATH` and `LLAMA_CPP_MODELS_PATH` (or auto‑scan).
2. Pick a `.gguf` model.
3. Click **Start Server**, wait for `server is listening`, then click **WebUI**.

### Build from Source

```sh
cd src-tauri
cargo tauri dev
cargo tauri build
```

### License

MIT (code), model licenses apply separately.

---

_LlamaGo — one click, not one command._
