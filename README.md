# WorkBuddy Code (`workbd`)

基于纯 Rust 原生开发的高性能自主编程 Agent (Autonomous Coding Agent)，专为 WorkBuddy / CodeBuddy AI 深度定制。原生支持 Linux，彻底取代 Wine 模拟，无死锁、无高 CPU 占用、秒级启动。

---

## 核心特性

- **自主编程 Agent (Autonomous ReAct Loop)**：
  - 具备本地真实开发工具调用权限，自动探索、编码、修复和验证。
  - 内置工具：
    - `bash`：在当前工作区执行终端命令（编译代码、跑测试、安装依赖、Git 操作等）。
    - `read_file`：读取指定文件内容，带精确行号标注。
    - `write_file`：自动创建目录并写入或完全重写文件。
    - `replace_in_file`：精准字符串替换修改代码。
    - `list_dir`：浏览指定目录结构。
    - `search_code`：使用 ripgrep/grep 全局代码搜索。
- **DeepSeek 深度思维链 (Reasoning Tokens)**：
  - 原生支持 `deepseek-v4.1-flash` 的思考过程推导，思考过程以专用淡色框独立渲染，随后无缝衔接工具调用与回答。
- **动态模型与域路由 (`/model`)**：
  - 支持 `/model` 实时查看全部模型；
  - 自动识别国内版（`cn:`）与国际版（`global:`）前缀，输入 `deepseek-v4.1-flash` 无感自动匹配。
- **Codex & 系统技能库深度融合 (Codex Skills Integration)**：
  - 零配置原生直接使用 Codex 技能（位于 `/home/root/.codex/skills` 与 `/home/codes/.agents/skills`）以及全量 720+ 离线技能库（`/home/codes/offlineSkills`）。
  - 内置 `load_skill` 与 `search_skills` Agent 工具，大模型可在复杂开发任务中自主按需检索并加载技能规范与脚本。
  - 支持交互命令 `/skills`、`/skill search <关键词>`、`/skill use <技能名>` 以及命令行参数 `--skill <名称>`。
- **极速轻量**：仅约 6MB 单一原生二进制，冷启动 <10ms，内存占用仅约 15MB。
- **双工作模式**：
  - 交互式 REPL：支持命令历史（上下键）、多轮上下文记忆、动态参数调整。
  - 命令行管道调用：支持直接通过参数执行单次复杂编程任务。

---

## 常用命令与交互快捷指令

在交互终端中支持以下命令：

| 指令 | 别名 | 说明 |
| :--- | :--- | :--- |
| `/agent [on\|off]` | - | 动态开启或关闭本地自主工具调用权限（默认开启） |
| `/permission [ask\|allow-all]` | `/p` | 设置工具执行权限：`ask` 每次调用前询问确认，`allow-all` 允许所有 |
| `/model [name]` | `/m` | 列出网关所有可用模型及价格，或指定切换至目标模型 |
| `/skills` | `/skill list` | 查看所有已就绪的 Codex 与系统活跃技能 |
| `/skill search <词>` | `/skill find` | 在 720+ 离线与系统技能库中模糊检索匹配的技能 |
| `/skill use <name>` | `/skill load` | 加载指定技能文档及可调用的脚本至当前对话上下文 |
| `/skill show <name>` | - | 直接在终端中查看指定技能的规范文档正文 |
| `/login [realm]` | - | 发起 OAuth 设备流登录（`cn` 国内版 / `global` 国际版） |
| `/clear` | `/c` | 清空当前对话与工具执行上下文，开始全新会话 |
| `/history` | - | 查看当前会话累计上下文消息数与工具调用次数 |
| `/system [prompt]` | - | 查看或修改当前 System Prompt |
| `/config` | - | 查看当前配置与网络接口信息 |
| `/help` | `/?` | 显示帮助菜单 |
| `/exit` | `/quit`, `/q` | 退出程序 |

---

## 快速使用示例

### 1. 启动交互式 Agent 终端
```bash
workbd
```

### 2. 命令行单次自主编程任务
可以直接让 Agent 自主创建代码并执行测试：
```bash
workbd "在当前目录创建一个 Rust 工具函数用于计算斐波那契数列，并在 tests/ 中编写测试，用 cargo test 验证通过"
```

### 3. 查看可用模型列表
```bash
workbd --list-models
```

### 4. 纯对话模式（禁用工具调用）
```bash
workbd --no-agent "什么是 Rust 的所有权机制？"
```

---

## 配置文件与路径

- 配置文件：`~/.workbd/config.json`
- 历史记录：`~/.workbd/history.txt`
- 凭证目录：`/home/bin/workbuddy2api/auths/`
