# Microsoft Store Listing — PiLPublisher

---

## 中文 (zh-CN)

### 简短描述

局域网文件共享工具 — 拖拽添加文件夹，扫码即下，设备间传文件从未如此简单。

### 描述

PiLPublisher 是一款轻量、免费的局域网文件共享工具。只需拖拽文件夹到窗口中，即可一键启动 HTTP 服务器，同一网络下的手机、平板、电脑都能通过浏览器或扫码直接访问和下载文件。

**核心功能：**

- **一键共享** — 拖拽或选择文件夹，点击开启，局域网内任意设备即可访问
- **扫码下载** — 自动生成二维码，手机扫一扫即可浏览下载，无需输入地址
- **子目录浏览** — 网页端完整支持目录结构浏览，和本地文件管理器一样直观
- **密码保护** — 可设置访问密码，防止未授权设备访问你的文件
- **权限控制** — 上传、重命名、删除等高级操作需手动开启（默认关闭），安全可控
- **持久记忆** — 添加的文件夹和密码自动保存，重启应用无需重新配置
- **系统托盘** — 最小化到托盘后台运行，不占用任务栏空间；下一版本可开机自启
- **单实例守护** — 已运行时再次打开会激活已有窗口，不会重复启动

**适用场景：**

- 手机和电脑之间快速传输照片、文档
- 办公室内同事间共享项目文件
- 家庭局域网内多设备文件互传
- 临时的文件分发，无需 U 盘或网盘上传

**技术亮点：**

- 基于 Tauri v2 + Rust 构建，内存占用极低（空闲时 < 20MB）
- 内嵌 HTTP 服务器，无需安装任何依赖
- 数据 100% 本地，不上传任何信息到互联网
- 默认端口 9726 自动获取局域网 IP

### 隐私与安全

PiLPublisher 不收集、不存储、不上传您的任何数据。文件共享仅限于您主动添加的文件夹，且仅在局域网内可访问。详见应用内隐私策略。

---

## English (en-US)

### Short Description

LAN file sharing made simple — drag in folders, scan a QR code, and download from any device on the same network.

### Description

PiLPublisher is a lightweight, free LAN file sharing tool. Drop folders into the window, start the server with one click, and any device on the same network — phone, tablet, or computer — can browse and download files via a browser or QR code scan.

**Key Features:**

- **One-Click Sharing** — Add folders by drag-and-drop or file picker, start sharing instantly
- **QR Code Access** — Auto-generated QR code lets phones scan and access without typing URLs
- **Subdirectory Browsing** — Full directory structure navigation on the web interface, just like a file manager
- **Password Protection** — Set an optional access password to keep your files private
- **Permission Control** — Upload, rename, and delete are opt-in only (off by default) for safety
- **Persistent Memory** — Shared folders and password are saved automatically across restarts
- **System Tray** — Runs quietly in the system tray; auto-start on boot coming in a future release
- **Single Instance** — Opening the app again brings the existing window to the front instead of launching a duplicate

**Use Cases:**

- Quick photo/document transfer between phone and PC
- Sharing project files with coworkers in the office
- Multi-device file transfer on a home network
- Temporary file distribution without USB drives or cloud uploads

**Tech Highlights:**

- Built with Tauri v2 + Rust — ultra-low memory footprint (< 20 MB idle)
- Embedded HTTP server — no dependencies required
- 100% local — no data is ever sent to the internet
- Auto-detects LAN IP on port 9726

### Privacy & Security

PiLPublisher does not collect, store, or upload any of your data. File sharing is limited to folders you explicitly add and is only accessible within your local network. See the in-app privacy policy for details.

---

## Feature Bullets (for Store UI)

| 中文 | English |
|------|---------|
| 拖拽添加文件夹，一键开启共享 | Drag-and-drop folders, share in one click |
| 自动生成二维码，手机扫码下载 | Auto QR code for mobile scan-and-download |
| 可选密码保护，安全可控 | Optional password protection |
| 上传/删除权限默认关闭 | Upload/delete permissions off by default |
| 文件夹和设置重启自动恢复 | Settings persist across restarts |
| 系统托盘后台运行 | System tray background operation |
| 极低内存占用 (<20MB) | Ultra-low memory footprint (<20 MB) |
| 100% 本地，无数据上传 | 100% local, zero data upload |

---

---

## 补充字段 (Additional Store Listing Fields)

### 简短说明 / Short Description (≤270 chars)

```
局域网文件共享工具 — 拖拽文件夹一键共享，自动生成二维码，手机扫码即可下载。支持密码保护，数据100%本地，不联网不上传。轻量免费，传文件从未如此简单。
```

```
LAN file sharing — drag folders to share, scan QR to download. Password protection included. 100% local, zero uploads. Free, lightweight, and effortless.
```

### 最低硬件 / Minimum Hardware

```
支持 Microsoft Edge WebView2 的硬件
Windows 10 版本 17763 或更高
512 MB 内存
50 MB 可用存储空间
局域网连接
```

```
Hardware that supports Microsoft Edge WebView2
Windows 10 version 17763 or later
512 MB RAM
50 MB available storage
LAN connection
```

### 推荐的硬件 / Recommended Hardware

```
支持 Microsoft Edge WebView2 的硬件
Windows 10 版本 17763 或更高
1 GB 内存
100 MB 可用存储空间
稳定的局域网连接 (有线或 Wi-Fi)
```

```
Hardware that supports Microsoft Edge WebView2
Windows 10 version 17763 or later
1 GB RAM
100 MB available storage
Stable LAN connection (wired or Wi-Fi)
```

---

## App Category

- **Category:** Utilities & tools / 实用工具
- **Subcategory:** File sharing / 文件共享
