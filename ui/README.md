# CodexBot Desktop UI

CodexBot 的桌面控制台，技术路线与 `codex_login` 保持一致：Tauri 2、React 19、TypeScript、Vite 7 和 Tailwind CSS 4。

## 本地运行

```powershell
cd ui
pnpm install
pnpm tauri:win dev
```

只查看和调试界面时，可以运行：

```powershell
pnpm dev
```

浏览器模式使用内置演示数据；Tauri 模式会调用 Rust 命令，读取 CodexBot 的本地状态、保存 QQ 凭据，并控制桥接服务。

## 构建 Windows 安装包

```powershell
cd ..
.\build-windows.cmd
```

构建完成后，交付文件位于：

```text
dist\CodexBot-Setup-0.1.0-x64.exe
dist\CodexBot-Portable-0.1.0-x64.exe
```

安装包采用当前用户安装模式，不需要管理员权限。桌面程序首次使用时，可在“连接设置”中安装或修复 Codex 插件与本地 Hook 运行时。

## 验证

```powershell
pnpm build
cargo check --manifest-path src-tauri\Cargo.toml
cargo test --manifest-path src-tauri\Cargo.toml
```
