# PILBroadcast 测试说明

- 测试完成：是（2026-10-04）
- 测试日期：2026-10-04
- 测试内容：单元测试覆盖 `capture.rs` 的环形缓冲取帧逻辑（`poll_frame` 序列递增、base64 编码、空队列、`CaptureState` 默认值、重复启动拒绝、停止标志）；集成测试在 `src-tauri/tests/` 中真实启动 tiny_http 服务（环回端口）并通过原始 TCP 发请求，覆盖路由（`/`、`/status`、`/frame`、未知路径回退）、密码门禁（错误密码 403 / 正确密码下发 Set-Cookie / Cookie 复用）、重复启动拒绝。注入测试针对不可信输入：畸形/负数/溢出/超长 `seq` 查询参数、XSS 载荷、路径穿越（`../`、`%2e%2e`、跨盘符）、密码字段中的 `<script>` 反射、伪造 Cookie、异常 HTTP 方法，验证服务不 panic、不把磁盘文件透传、不回显用户输入。本仓库无自定义钩子/插件/事件注册表（Tauri 托盘与窗口事件需 GUI 运行时，无法在无显示环境单测），故钩子测试不适用。
- 运行命令：在 `src-tauri/` 目录执行 `cargo test`（本仓库为单 crate，非 workspace）
- 测试框架：Rust `#[cfg(test)]` + 自写原始 TCP HTTP 客户端（无额外测试依赖）
- 模型：豆包（Doubao）生成

## 测试布局

这是一个 Tauri v2 应用，真正的 Rust crate 位于 `src-tauri/`（`Cargo.toml` 所在目录），因此：

- **单元测试**：`src-tauri/src/capture.rs` 末尾的 `#[cfg(test)] mod tests`（6 个用例）。
- **集成测试**：`src-tauri/tests/server_integration.rs`（11 个用例），cargo 只在 crate 根（`src-tauri/`）下发现 `tests/`，故此处为 `src-tauri/tests/` 而非仓库根 `tests/`。
- 为让集成测试能从 crate 外部驱动真实服务，`src-tauri/src/lib.rs` 中 `mod capture; mod server;` 改为 `pub mod`（仅可见性，无运行时行为变化）。

## 如何运行

```bash
cd src-tauri
cargo test
```

依赖 `cargo 1.99 / rustc 1.99`。首次构建会编译 tauri 等依赖；之后增量很快。
集成测试会随机绑定环回端口并通过真实 HTTP 请求访问，无需显示器（屏幕捕获线程不会在测试中启动）。

## 预期结果

```
test capture::tests::...                         6 passed
test server_integration::...                    11 passed
test result: ok. 17 passed; 0 failed
```

## 原有测试覆盖

仓库原本**没有任何自动化测试**：`src/capture_test.rs` 是一个手动截屏的 `[[bin]]` 工具（在主窗口里手动运行、写一张 jpg），不是 `#[cfg(test)]`；仓库无 `tests/` 目录，也无 `.github/workflows` CI。本次新增 6 个单元 + 11 个集成，其中 6 个为注入/恶意输入用例。
