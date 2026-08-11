<p align="center">
  <img src="docs/images/codexbot-cover.png" alt="CodexBot cover" width="100%">
</p>

<h1 align="center">CodexBot</h1>

<p align="center">Codex 完成或最终失败时，通过 QQ 官方机器人通知你。</p>

CodexBot 是一个面向 Windows 的本地通知桥。它让 Codex 在电脑上继续工作，并把“本轮已结束”或“本轮最终失败”发送到你的 QQ，适合长任务、离开电脑和上游服务不稳定的场景。

> [!IMPORTANT]
> **源码来源与版权声明**
>
> 本仓库不是从零编写，也不是通过 GitHub 的 Fork 按钮建立；代码直接基于原作者 **LeaningLearner** 的开源项目 [LeaningLearner/codexbot](https://github.com/LeaningLearner/codexbot) 继续修改，并保留了原项目 Git 历史。
>
> 原项目采用 MIT License，原始版权声明 `Copyright (c) 2026 LeaningLearner` 已完整保留在 [LICENSE](LICENSE) 中。本修改版由 ZhangStudyLife 独立维护，不代表原作者官方发布、认可或提供支持。详细来源和修改范围见 [NOTICE](NOTICE)。

本修改版主要增加 Codex App Server 最终失败监控，将 QQ 端收敛为单用户通知模式，并补充中文部署及 Windows Release 安装包。除这些修改外，大量项目结构、QQ 通信、Hooks、SQLite、安装和运行代码均来源于上述原项目及其贡献者。

## 功能特点

- 回合正常结束后发送简短提醒，回复 `/last` 再查看结果，不把完整回复主动推到 QQ。
- 只在整个回合最终状态变成 `failed` 时发送失败提醒；中间的自动重试不会重复报警。
- 覆盖容量不足、429、503、连接、认证、沙箱错误及其他请求失败。
- QQ 消息包含项目名、脱敏后的错误摘要、错误类型，以及可用时的 HTTP 状态码。
- 约每 2 秒检查一次最终状态，按 `thread_id + turn_id` 幂等去重。
- 首次启动只建立基线，不补发历史失败；重启后使用 60 秒重叠窗口补漏。
- AppSecret 保存在 Windows Credential Manager，不写入仓库、SQLite 或日志。
- 仅绑定一个 QQ 用户；不支持通过 QQ 执行任意命令、批准操作或控制本机文件。

## 环境要求

- Windows 10/11 x64。
- 已安装并能正常使用 Codex Desktop 或 Codex CLI，且支持 Plugins、Hooks 和 App Server。
- QQ 开放平台官方机器人，具有 AppID 和 AppSecret。
- 机器人处于沙箱环境，你的 QQ 已加入沙箱，并已启用私聊事件和主动消息能力。
- 安装和发送通知时可以访问 QQ 开放平台。

## 部署方式一：下载 Release（推荐）

此方式不需要安装 Rust。

1. 从 [Releases](https://github.com/ZhangStudyLife/codexbot/releases/latest) 下载 `codexbot-windows-x64.zip`。压缩包中的 `NOTICE` 和 `LICENSE` 说明了代码来源与许可。
2. 完整解压压缩包，不要直接在压缩包预览窗口内运行。
3. 在解压目录中运行：

   ```bat
   .\install-release.cmd
   ```

4. 按提示输入 QQ 机器人 AppID 和 AppSecret。AppSecret 输入时不会回显，并会保存到 Windows Credential Manager。
5. 重启 Codex，在 `/hooks` 中检查并信任 `codexbot` Hooks。
6. 确认自己的 QQ 已加入机器人沙箱，然后私聊机器人发送安装器显示的配对命令：

   ```text
   /bind XXXX-XXXX
   ```

配对码有效期为 30 分钟。过期后可重新生成：

```bat
.\codexbot.cmd pair
```

如果需要替换已经保存的 QQ 凭据：

```bat
.\install-release.cmd --replace-credentials
```

## 部署方式二：从源码构建

需要 Rust 1.85 或更高版本。

```bat
git clone https://github.com/ZhangStudyLife/codexbot.git
cd codexbot
.\install.cmd
```

`install.cmd` 会执行锁定依赖的 Release 构建，将 `codexbot.exe` 安装到 `%LOCALAPPDATA%\CodexBot\bin`，保存 QQ 凭据，安装个人 Codex 插件并生成一次性配对码。

## 检查和运行

```bat
.\codexbot.cmd doctor --offline
.\codexbot.cmd doctor
```

- `doctor --offline`：检查本地二进制、凭据、插件和 daemon，不连接 QQ。
- `doctor`：额外验证 QQ 沙箱网络连接。

默认情况下，Hooks 会在 Codex 活动时拉起 daemon。若希望机器人和失败监控常驻运行：

```bat
.\codexbot.cmd start
.\codexbot.cmd stop
```

## QQ 命令

| 命令 | 作用 |
| --- | --- |
| `/bind XXXX-XXXX` | 使用一次性配对码绑定当前 QQ |
| `/status` | 查看最近的 Codex 项目和状态 |
| `/last [项目] [页码]` | 分页查看最近回复 |
| `/mute` | 暂停后续主动通知 |
| `/unmute` | 恢复后续主动通知 |
| `/help` | 显示帮助 |

`/usage` 和 `/account` 在此通知模式中未启用。

## 通知规则

### 正常结束

Codex 产生最终回复并触发 `Stop` Hook 后，QQ 收到：

```text
✅ Codex 本轮已结束
项目：demo
回复 /last 查看结果
```

完整回复只保存在本机数据库中，默认 7 天后清理；需要时由已绑定用户使用 `/last` 查看。

### 最终失败

失败监控通过本机 Codex App Server 查询回合列表。只有回合的最终状态为 `failed` 才会通知，例如：

```text
❌ Codex 本轮失败
项目：demo
错误：Selected model is at capacity. Please try a different model.
类型：responseTooManyFailedAttempts
HTTP：503
```

错误消息会移除常见密钥、压缩空白并截断到 500 个字符；不会发送提示词、工具参数、堆栈或 `additionalDetails`。

如果 App Server 暂时不可用，监控游标不会前进，恢复连接后会继续扫描。首次启用不会把旧任务全部推送到 QQ；正常重启会回看最近 60 秒，以减少关机瞬间漏报。

## 常见问题

### Codex 有回复，但 QQ 没消息

依次检查：

1. 已重启 Codex，并在 `/hooks` 中信任 `codexbot`。
2. 已向机器人发送 `/bind`，且配对码未过期。
3. QQ 账号在机器人沙箱名单中，机器人允许私聊事件和主动消息。
4. 运行 `codexbot.cmd doctor` 查看 QQ 和 daemon 状态。
5. 查看 `%LOCALAPPDATA%\CodexBot\logs\codexbot.log` 的最近错误。

### 任务失败，但 QQ 没消息

失败提醒依赖当前 Codex 的 App Server 实验接口。请先更新 Codex，再执行：

```bat
.\codexbot.cmd stop
.\codexbot.cmd start
.\codexbot.cmd doctor --offline
```

如果日志出现 `Failed to connect turn failure monitor` 或 `Turn failure monitor scan failed`，说明监控暂时无法读取回合最终状态；游标不会因此跳过失败记录。

### 更新后仍使用旧版本

重新运行安装脚本后必须重启 Codex，让新插件和 Hooks 生效。检查版本：

```bat
"%LOCALAPPDATA%\CodexBot\bin\codexbot.exe" --version
```

## 本地数据和安全

默认数据目录为 `%LOCALAPPDATA%\CodexBot`：

- `bin\codexbot.exe`：已安装运行时。
- `state.sqlite3`：绑定、状态、通知队列和最近回复。
- `logs\codexbot.log`：诊断日志。

不要提交 AppID、AppSecret、Access Token、SQLite 数据库或日志。如果凭据曾出现在公开仓库、终端录屏或截图中，请立即在 QQ 开放平台重新生成 AppSecret。

## 开发与验证

```powershell
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo run -- doctor --offline
py -3.11 "$env:USERPROFILE\.codex\skills\.system\plugin-creator\scripts\validate_plugin.py" plugin\codexbot
```

## License

[MIT](LICENSE)。本项目直接基于 [LeaningLearner/codexbot](https://github.com/LeaningLearner/codexbot) 修改；原作者版权声明、修改版归属和免责声明见 [NOTICE](NOTICE)。
