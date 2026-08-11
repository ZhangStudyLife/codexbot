# QQ Codex 远程控制台设计

日期：2026-08-11  
目标版本：v0.2.0

## 目标

把 CodexBot 从只读通知机器人扩展为 QQ 私聊中的文字版 Codex 控制台。用户通过 QQ 官方机器人按钮完成目录选择、模型配置、任务创建、进度查看、继续任务和停止任务，不需要记忆或手动输入 `/last` 等命令。

QQ 与 Codex Desktop 使用同一套本机任务记录。QQ 创建的任务必须出现在桌面端，桌面端创建的任务也必须能在 QQ 中查看和继续。

## 已确认的产品决策

- 使用 QQ C2C Markdown Keyboard 的指令按钮和状态向导。
- Keyboard 不可用时退化为编号文字菜单。
- 允许选择整台电脑上的任意工作目录。
- QQ 启动的 Codex 任务使用全盘权限，不做文件、命令或工具审批。
- 使用 `approvalPolicy: never` 和 `sandboxPolicy: dangerFullAccess`。
- 只允许当前已绑定的唯一 QQ 用户操作。
- 任务进度按需刷新，完成或失败时主动通知。
- Codex Desktop 未启动时，CodexBot 仍可通过本机 App Server 运行任务。
- 支持多个并行任务，最多同时运行 3 个由 QQ 启动的任务。
- 模型和推理强度从 App Server 动态读取。
- 记住上一次模型和强度，提供“直接使用”和“修改配置”。
- 第一版不提供独立文件管理器、远程桌面或实时终端流。

## 官方能力依据

QQ 官方 C2C 消息接口支持 `markdown` 和 `keyboard` 字段。指令按钮使用 `action.type = 2`，设置 `action.enter = true` 后可直接发送按钮中的指令数据。按钮不受支持时使用 `action.unsupport_tips` 提示，并由 CodexBot 返回编号菜单。

Codex App Server 提供本设计所需的接口：

- `thread/list`
- `thread/read`
- `thread/start`
- `thread/resume`
- `turn/start`
- `turn/steer`
- `turn/interrupt`
- `model/list`

App Server 使用本机 stdio 连接，不开放公网监听端口。

## 总体架构

```text
QQ 私聊按钮或文字输入
        |
        v
QQ 官方 Gateway
        |
        v
QQRuntime
        |
        +--> QQMenu：生成按钮和编号菜单
        |
        +--> ControlSession：解析向导步骤和一次性操作令牌
        |
        +--> CodexControlRuntime：串行请求和事件分发
                    |
                    v
          本机 Codex App Server
                    |
                    v
          Codex Desktop 共用任务存储
```

### QQMenu

负责把控制台状态渲染为纯数据结构，再由 QQ 客户端编码为 C2C Keyboard payload。菜单本身不访问数据库或 App Server。

每个菜单同时生成：

- Keyboard 按钮版本。
- 编号文字降级版本。

### ControlSession

负责唯一绑定用户的状态向导：

- 当前步骤。
- 已选目录。
- 已选模型和推理强度。
- 当前任务或线程 ID。
- 菜单操作令牌和过期时间。

完整任务提示词不写入 CodexBot SQLite。用户输入提示词后仅在内存中保存到最终“启动任务”按钮被点击；daemon 重启时丢弃未提交提示词。

### CodexControlRuntime

维护一个长期运行的 experimental App Server 会话。所有请求通过单个 Tokio channel 进入该运行时，避免多个调用者同时读取 stdout 或错配 JSON-RPC 响应。

运行时职责：

- 初始化和重连 App Server。
- 发送线程、回合和模型请求。
- 分发 `turn/*`、`thread/*` 和 `item/*` 通知。
- 保存 QQ 启动任务的运行集合。
- 驱动现有最终失败监控。
- App Server 断开后恢复只读查询能力。

不向 QQ 暴露 `thread/shellCommand`、`command/exec` 或 `process/spawn`。全盘操作只能由用户提交的 Codex 任务执行。

## 主菜单

```text
CodexBot 远程控制台
运行中：1 个任务

[新建任务] [运行中]
[任务列表] [最近结果]
[项目目录] [设置]
```

主菜单在以下情况返回：

- 用户发送 `/menu` 或 `/help`。
- 用户点击“返回主菜单”。
- 绑定成功。
- 向导取消或过期。

## 新建任务向导

### 1. 选择目录

提供四个入口：

- 最近项目：从 Codex `thread/list` 结果提取最近使用的 cwd，并去重。
- 收藏目录：从 CodexBot SQLite 读取。
- 浏览磁盘：列出 Windows 可用盘符，再逐级列出子目录。
- 输入路径：下一条普通 QQ 文本作为绝对目录路径。

目录必须存在且为目录。无法读取的目录返回错误，但向导停留在目录选择步骤。

目录浏览只列出目录，不读取或发送文件内容。

### 2. 选择模型配置

CodexBot 调用 `model/list` 获取当前可见模型、支持的 reasoning effort 和默认项。

如果存在上一次有效配置，显示：

```text
上次配置：gpt-5.6-sol / high

[直接使用] [修改配置]
```

模型下线或原强度不再受支持时，忽略旧配置并要求重新选择。

### 3. 输入任务

用户下一条普通 QQ 文本作为任务提示词。提示词只保留在内存中，不写 CodexBot 日志或 SQLite。

### 4. 启动确认

显示：

- 工作目录。
- 模型。
- 推理强度。
- 截断后的任务摘要。

按钮为“启动任务”和“取消”。这只是提交任务前的界面确认；任务运行期间不再请求操作审批。

启动时依次调用：

1. `thread/start`，传入 cwd、模型、`approvalPolicy: never` 和全盘 sandbox 设置。
2. `turn/start`，传入 thread ID、任务文本、模型、强度和同样的权限设置。

请求结果不确定时不得自动重发。机器人返回“检查任务列表”按钮，由用户确认是否已经创建。

## 任务列表与详情

任务列表每页最多显示 5 个任务，默认按最近更新时间倒序。

筛选项：

- 全部。
- 运行中。
- 已完成。
- 已失败。

任务详情显示：

- 标题或任务 ID 缩写。
- 工作目录。
- 模型和推理强度。
- 当前状态。
- 最近活动时间。
- 最近一条可安全展示的进度摘要。

### 运行中任务

按钮：

- 刷新进度。
- 查看最新输出。
- 追加引导。
- 停止任务。
- 返回任务列表。

“追加引导”使用 `turn/steer`，不创建第二个并行 Turn。

“停止任务”使用 `turn/interrupt`。停止后保留线程和历史，可以继续任务。

### 已完成任务

按钮：

- 查看结果。
- 继续任务。
- 新建同目录任务。
- 返回任务列表。

继续任务调用 `thread/resume`，用户输入下一条消息后调用新的 `turn/start`。

### 已失败任务

按钮：

- 查看错误。
- 重试任务。
- 继续任务。
- 返回任务列表。

“重试任务”在同一线程中重新发送上一条用户任务。上一条提示词从 Codex 线程历史读取，不从 CodexBot SQLite 读取。

## 并发规则

- 最多同时运行 3 个由 QQ 启动且尚未结束的 Turn。
- 桌面端任务仍全部展示，不由 CodexBot 强制停止。
- 达到上限时禁用“启动任务”，并列出当前 3 个运行任务。
- 同一线程只允许一个活动 Turn。
- 桌面端已经在某线程运行 Turn 时，QQ 只提供刷新、追加引导和停止。

## 通知行为

保持现有通知原则：

- 正常完成时发送简短完成通知，不主动发送完整回复。
- 最终失败时发送脱敏错误、错误类型和可用 HTTP 状态。
- 中间重试不发送失败通知。

完成和失败通知新增按钮：

- 查看任务。
- 查看结果或错误。
- 继续任务。
- 新建任务。

静音状态继续只影响主动通知，不影响菜单查询和任务执行。

## 身份与安全边界

全盘权限是用户明确选择的运行模式。CodexBot 不添加工具审批，但必须保护控制入口：

- 只接受 SQLite 中已绑定 openid 的控制请求。
- 沿用常量时间 openid 比较。
- 沿用 inbound message ID 幂等去重。
- 每个修改型按钮包含随机、一次性、短期有效的操作令牌。
- 令牌与 openid、动作、目标线程和过期时间绑定。
- 过期或已使用令牌返回新菜单，不执行原动作。
- AppID、AppSecret、Codex token 和完整提示词不得写入日志。
- QQ 消息中的错误继续经过脱敏和长度限制。

机器人凭据或绑定 QQ 账号失守意味着攻击者可通过 Codex 任务获得整机权限。README 和安装输出必须明确警告这一风险。

## 持久化

在现有 SQLite 中增加最小状态：

- 当前向导阶段和非敏感选择。
- 操作令牌哈希、动作、目标和过期时间。
- 收藏目录。
- 上次模型和推理强度。
- QQ 创建任务的线程 ID 和活动状态，用于并发计数与重启恢复。

不新增第二个数据库，不保存 App Server 认证信息，不复制 Codex 线程正文。

## 失败处理

### QQ Keyboard 失败

如果发送 Keyboard 返回不支持或永久错误，当前回复立即退化为编号菜单。该用户后续会话优先使用文字菜单，直到 daemon 重启或设置中重新检测。

### App Server 断开

- 控制菜单显示“控制服务重连中”。
- 只读请求使用有上限的指数退避。
- 修改请求不自动重试。
- 重连后使用 `thread/list` 和 `thread/read` 恢复真实状态。
- 无法确认的任务不得自动重新启动。

### Codex Desktop 并发操作

每次显示任务详情和执行修改操作前重新读取线程状态。状态与按钮生成时不一致时，拒绝旧操作并返回刷新后的详情。

### 向导过期

向导 15 分钟无操作后过期。路径、模型等非敏感选择可以恢复到对应步骤；内存中的未提交提示词直接丢弃。

## 代码边界

计划新增：

- `src/qq_menu.rs`
- `src/control_session.rs`
- `src/codex_control.rs`

计划修改：

- `src/qq_client.rs`：Keyboard payload 和降级发送。
- `src/commands.rs`：只保留命令入口和向新控制模块路由。
- `src/store.rs`：最小 schema 迁移和状态方法。
- `src/turn_monitor.rs`：将扫描调度接入共享控制运行时。
- `src/daemon.rs`：启动和关闭控制运行时。
- `src/delivery.rs`：为完成和失败通知附加快捷菜单。

不得顺带重构账号切换、桌面 UI 或无关安装逻辑。

## 测试与验收

### 单元测试

- C2C Keyboard payload 字段和按钮动作正确。
- 编号菜单与 Keyboard 表示相同动作。
- 向导所有合法状态迁移。
- `/cancel`、过期和 daemon 重启行为。
- 非绑定用户、过期令牌、重复令牌和重复消息被拒绝。
- Windows 盘符和目录浏览结果稳定排序。
- 模型及强度默认值和失效回退。
- `turn/start` 包含 `approvalPolicy: never` 和 `dangerFullAccess`。
- 修改请求超时不自动重试。
- 同一线程禁止第二个活动 Turn。
- 第 4 个 QQ 并行任务被拒绝。

### App Server 假实现测试

- 新建任务调用顺序为 `thread/start` 后 `turn/start`。
- 继续任务调用 `thread/resume` 后 `turn/start`。
- 运行中追加使用 `turn/steer`。
- 停止任务使用 `turn/interrupt`。
- `thread/list` 分页和状态映射。
- `model/list` 模型与 effort 映射。
- 断线重连后恢复任务状态且不重复启动。

### 回归验证

- 现有测试全部通过。
- `cargo fmt --all --check` 通过。
- `cargo clippy --all-targets --locked -- -D warnings` 通过。
- `cargo test --all-targets --locked` 通过。
- 插件校验器通过。
- `doctor --offline` 通过。

### 本机 QQ 沙箱验收

- 点击按钮能够完成整个新任务向导。
- QQ 创建的任务出现在 Codex Desktop。
- 桌面任务可在 QQ 中继续。
- 3 个任务可并行，第 4 个收到明确限制提示。
- 完成和最终失败通知附带可用按钮。
- Keyboard 人为禁用后编号菜单仍能完成同样流程。

## 版本与发布

该功能作为 v0.2.0 发布。Release 包继续包含：

- `codexbot.exe`
- `install-release.cmd`
- `codexbot.cmd`
- Codex 插件目录
- `README.md`
- `LICENSE`
- `NOTICE`

README 必须增加全盘权限风险说明、QQ 控制台使用说明和 Keyboard 能力要求。
