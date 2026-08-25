# Tunnel Helper

Tunnel Helper is a portable Windows desktop app for managing SSH tunnels, SSH keys, connection profiles, and OpenSSH config snippets from a visual interface.

它的目标是把零散的 SSH 命令、密钥文件和连接配置收进一个清晰的可视化流程里，降低重复操作和配置出错的概率。

这个项目最初来自一个很具体的校园网络场景：学校内网访问需要通过腾讯 iOA/VPN 客户端完成。它可以让机器接入校园网络并访问学校服务器，但这类客户端对个人主力系统来说侵入性较强，可能修改系统设置、网络策略或安全配置，因此并不一定适合直接安装在本机。

一个更可控的做法是：准备一台 Windows 虚拟机，把 iOA/VPN 安装在虚拟机中，让虚拟机负责接入校园网络；然后在虚拟机里运行 Tunnel Helper，建立到学校服务器的 SSH 隧道，并放行虚拟机防火墙中的本地监听端口。这样，与该虚拟机处在同一局域网的主机或设备，就可以通过这个隧道访问学校服务器，同时把 iOA/VPN 与个人主力系统做物理和系统层面的隔离。

虽然项目背景来自 iOA/VPN 场景，但 Tunnel Helper 本身不是 iOA 专用工具。它更准确的定位是一个通用 SSH tunnel helper，适合需要用 SSH 本地端口转发访问远端服务、跳板机、虚拟机或内网机器的用户。

## 适用范围

Tunnel Helper 适合这些场景：

- VPN、iOA、零信任客户端运行在 Windows 虚拟机、跳板机或另一台 Windows 机器中。
- 希望把侵入性较强的网络客户端隔离在虚拟机里，不直接安装在主力系统。
- 需要通过 SSH 本地端口转发访问远端 Linux 服务器或内网服务。
- 需要让同一局域网内的其他机器通过虚拟机上的隧道端口访问目标服务器。
- 需要在多个连接档案之间切换，例如不同服务器、不同用户、不同本地端口。
- 希望可视化管理 SSH 私钥、公钥、`~/.ssh/config` 和连接状态。
- 希望把程序目录、密钥目录、配置文件一起复制到另一台机器继续使用。
- 需要生成新的 SSH 密钥，并把公钥安装到 Linux 服务器的 `authorized_keys`。

Tunnel Helper 不适合这些场景：

- 替代 iOA、VPN 或零信任客户端本身。
- 绕过学校、单位或服务器管理员明确禁止的访问策略。
- 管理复杂企业网络准入、路由策略或 VPN 拨号流程。
- 作为多人共享的密码保险库或密钥托管平台。

使用本工具前，请确认你的网络访问方式符合所在组织的安全、合规和授权要求。

## 主要功能

- 自动扫描程序相对目录和用户 `~/.ssh` 目录下的 SSH 密钥。
- 区分用户密钥和程序相对目录密钥。
- 批量导出、导入、删除密钥。
- 导出公钥 bundle，便于集中复制或安装公钥。
- 编辑和保存连接档案 `profiles.json`。
- 支持密钥认证和密码认证。
- 支持带私钥密码的密钥连接。
- 启动、停止和查看多个后台 SSH 隧道。
- 检测 SSH 服务连通性、本地端口占用和防火墙放行。
- 写入或复制 OpenSSH `~/.ssh/config` 配置。
- 生成 SSH 密钥，并辅助安装公钥到 Linux 服务器。
- 支持中文和英文界面。

## 便携目录

推荐把程序、密钥和配置放在同一个目录中，方便复制到虚拟机或其他 Windows 机器。

```text
Tunnel Helper/
  tunnel-helper.exe
  ssh_keyfiles_export/
    id_ed25519_example
    id_ed25519_example.pub
  profiles.json
  app_settings.json
```

其中：

- `tunnel-helper.exe` 是主程序。
- `ssh_keyfiles_export/` 用于保存程序相对目录密钥。
- `profiles.json` 保存连接档案。
- `app_settings.json` 保存界面语言、鼠标样式、默认填空行为等本地设置。

公开分享源码时，不要提交真实的 `ssh_keyfiles_export/`、`profiles.json` 或 `app_settings.json`。

## 基本使用流程

1. 在“密钥管理”中扫描或导入 SSH 密钥。
2. 在“密钥生成与安装”中生成新密钥，并把公钥安装到 Linux 服务器。
3. 在“配置管理”中创建或编辑连接档案。
4. 在“连接配置”中选择档案，检测 SSH 服务和本地端口。
5. 启动隧道后，在“Connections”中查看、停止或清理连接记录。
6. 在“Logs”中查看操作日志和命令反馈。

## SSH 配置

Tunnel Helper 可以生成 OpenSSH 配置片段，并写入用户目录下的：

```text
~/.ssh/config
```

这个配置可被 OpenSSH、VS Code Remote SSH、JetBrains Gateway 等工具读取。写入前请确认别名、端口、用户名和密钥路径正确。

## 安全提醒

不要提交这些本地文件到 GitHub：

- `ssh_keyfiles_export/`：可能包含私钥。
- `profiles.json`：可能包含服务器地址、用户名、密钥路径、密码字段。
- `app_settings.json`：可能包含本地默认填空信息。
- `backups/`、`downloads/`、`node_modules/`、`src-tauri/target/`：构建产物或缓存目录。

仓库里只应该提供：

- `profiles.example.json`
- `app_settings.example.json`
- 源码、构建脚本、图标和必要资源

如果你要把项目公开到 GitHub，建议在提交前运行：

```powershell
git status --short
```

确认没有私钥、真实服务器配置或真实密码被加入提交。

## 开发

本项目使用 Tauri、Rust 和前端 Web 技术构建。Windows 原生编译需要 Microsoft C++ Build Tools。

安装依赖：

```powershell
npm install
```

启动开发模式：

```powershell
npm run dev
```

构建前端和 Tauri 程序：

```powershell
npm run build
```

生成便携版 exe：

```powershell
.\build-release.ps1
```

成功后，项目根目录会生成：

```text
tunnel-helper.exe
```

## 离线开发说明

项目曾按“尽量可离线迁移”的思路整理依赖：

- Rust、Node、构建缓存和安装包可以放在项目目录或上级 `downloads/` 目录。
- `node_modules/`、`src-tauri/target/` 不建议提交到 GitHub。
- 如果要在新机器上离线开发，需要提前准备对应平台的 Node、Rust、Cargo 缓存和 Windows 构建工具。

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
