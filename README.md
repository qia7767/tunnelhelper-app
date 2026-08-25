# Tunnel Helper

便携式 SSH 隧道可视化工具原型。

便携目录示例：

```text
Tunnel Helper/
  tunnel-helper.exe
  ssh_keyfiles_export/
  profiles.json
```

主要能力：

- 自动扫描程序相对目录和 `~/.ssh` 下的密钥。
- 显示私钥文件名、公钥指纹和来源目录。
- 检测本机私有 IPv4，用于选择虚拟机/网关 IP。
- 查找可用本地端口。
- 启动/停止 SSH 隧道。
- 写入或复制 OpenSSH `~/.ssh/config` 配置。
- 生成密钥并辅助安装公钥到 Linux 服务器。
- 支持中文和英文界面。

## 安全提醒

不要提交这些本地文件到 GitHub：

- `ssh_keyfiles_export/`：包含私钥。
- `profiles.json`：可能包含服务器地址、用户名、密钥路径、密码字段。
- `app_settings.json`：可能包含本地默认填空信息。
- `backups/`、`downloads/`、`node_modules/`、`src-tauri/target/`：构建或缓存产物。

仓库里提供了 `profiles.example.json` 和 `app_settings.example.json` 作为公开模板。

## 开发

开发环境尽量放在项目目录：

```powershell
. .\dev-env.ps1
npm install
npm run dev
```

Windows 原生编译仍需要系统级 Microsoft C++ Build Tools。离线 layout 放在上级 `downloads\vs_buildtools_layout`。

管理员 PowerShell 中运行：

```powershell
.\install-build-tools-offline.ps1
```

然后重新打开 PowerShell，再运行：

```powershell
. .\dev-env.ps1
npm run build
```

生成便携 exe：

```powershell
.\build-release.ps1
```

成功后根目录会出现：

```text
tunnel-helper.exe
ssh_keyfiles_export/
profiles.json
```

## 发布到 GitHub

建议只把源码推到仓库，把生成的 `.exe` 放到 GitHub Releases。首次发布前请确认：

- `.gitignore` 生效。
- `git status --short` 里没有私钥、真实 `profiles.json`、真实 `app_settings.json`。
- 第三方鼠标指针资源具有允许再分发的授权。
