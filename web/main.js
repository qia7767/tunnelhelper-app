const invoke = window.TunnelBridge.invoke;
const isLinuxServer = !window.TunnelBridge.isTauri;

const state = {
  keys: [],
  profiles: [],
  selectedKey: null,
  selectedKeys: new Set(),
  selectedProfileName: "",
  connections: [],
  statusTimer: null,
  installGuideShown: false,
  appSettings: null,
  draggingProfileName: "",
  profileDragMoved: false,
  profileDrag: null,
  isClosingApp: false,
  currentPage: "keysPage",
  connectionsUnreadCount: 0,
  highlightedConnectionPid: null,
  highlightTimer: null,
};

const $ = (id) => document.getElementById(id);

const DEFAULT_APP_SETTINGS = {
  language: "zh",
  cursor: "NArrowBeadCursor",
  defaults: {
    keygenUser: "",
    keygenHost: "",
    keygenOffset: "",
    installKeyName: "",
    installUser: "",
    installHost: "",
    installPort: 22,
  },
};

const I18N = {
  zh: {
    help: "帮助",
    helpLoadFailed: "无法加载帮助文档。",
    refresh: "刷新检测",
    refreshing: "刷新中",
    settings: "设置",
    keyManagement: "密钥管理",
    keyInstall: "密钥生成与安装",
    profileManagement: "配置管理",
    connectConfig: "连接配置",
    connections: "Connections",
    logs: "Logs",
    userKeys: "用户密钥",
    portableKeys: "程序相对目录密钥",
    openFileLocation: "打开文件位置↗",
    selectAll: "全选",
    keyActions: "密钥操作",
    selectedCount: (count, names) => `已选择 ${count} 个：${names}`,
    notSelected: "未选择",
    allowImport: "允许导入到 ~/.ssh",
    exportPublicBundle: "导出公钥 bundle",
    deleteSelectedKeys: "删除选定密钥",
    exportToPortable: "导出到程序目录",
    importToSsh: "导入到 ~/.ssh",
    keygenTitle: "1 密钥生成",
    installTitle: "2 密钥安装",
    verifyTitle: "3 密钥验证",
    serverUser: "服务器用户",
    serverHost: "服务器地址",
    keySuffix: "密钥后缀",
    newKeyName: "新密钥名称",
    privatePassphraseOptional: "私钥密码（可选）",
    generateKey: "生成密钥",
    sshPort: "SSH 端口",
    serverPassword: "服务器密码",
    publicKeyFile: "公钥文件",
    terminalInstall: "终端安装",
    autoFill: "自动填充",
    existingServerKey: "服务器已有登录密钥",
    silentInstall: "静默安装公钥",
    verifyKey: "验证使用密钥",
    verifyLogin: "验证密钥登录",
    profileList: "配置档案",
    add: "新增",
    editProfiles: "编辑 profiles.json",
    deleteProfile: "删除此配置",
    save: "保存",
    applyToConnect: "应用到连接",
    profileName: "档案名称",
    sshAlias: "SSH 别名",
    serverPort: "服务器端口",
    localStartPort: "本地起始端口",
    reverseTunnel: "反向隧道",
    yes: "是",
    no: "否",
    remotePortOnA: "中转服务器监听端口",
    reverseTargetUser: "目标主机登录用户",
    reverseTargetPort: "目标主机 SSH 端口",
    authMode: "认证方式",
    keyAuth: "使用密钥",
    passwordAuth: "使用密码",
    optionalKey: "可选密钥",
    useProfile: "使用档案",
    localPort: "本地端口",
    vmIp: "虚拟机 IP",
    autoVmIp: "自动检测并更新 VM IP",
    on: "开启",
    off: "关闭",
    tunnelControl: "隧道控制",
    notStarted: "未启动",
    start: "启动",
    autoReconnect: "自动重连",
    reconnectDelay: "重连等待秒数",
    pingServer: "Ping 服务器",
    sshTestServer: "SSH 测试服务器",
    firewall: "放行防火墙",
    refreshPort: "重新检测端口",
    sshConfig: "SSH 配置",
    writeSshConfig: "写入 ~/.ssh/config",
    copy: "复制",
    runningCount: (running) => `${running} 个运行中`,
    connectionCount: (running, total) => `${running} 个运行中 / ${total} 个记录`,
    clearClosed: "清除已断开记录",
    clear: "清空",
    language: "语言",
    zhPreferred: "中文（首选）",
    english: "English",
    cursorStyle: "鼠标样式",
    systemDefault: "系统默认",
    confirmOperation: "确认操作",
    cancel: "取消",
    confirm: "确认",
    noConnections: "暂无连接",
    noConnectionsHint: "启动隧道后会在这里显示后台连接，可单独停止。",
    viewConfig: "查看配置",
    stop: "停止",
    localToRemote: (local, remote) => `本地 ${local} -> 远端 127.0.0.1:${remote}`,
    reverseToLocal: (remote, target) => `中转服务器 127.0.0.1:${remote} -> 目标主机 127.0.0.1:${target}`,
    noVmIp: "未检测到虚拟机 IP",
    portAvailable: "端口可用",
    usingPort: (port) => `已改用 ${port}`,
    noKey: "不指定",
    noPublicKey: "不自动选择",
    chooseInstalledKey: "选择已安装在服务器上的密钥",
    chooseVerifyKey: "选择用于验证登录的密钥",
    savedMissing: "已保存，未扫描到",
    userKeySource: "用户密钥 ~/.ssh",
    portableKeySource: "程序目录密钥",
    noFingerprint: "未找到公钥指纹",
    configuredPassword: "已配置密码",
    configuredKeyPassphrase: "已配置私钥密码",
    unspecifiedKey: "未指定密钥",
    running: "运行中",
    reverseRunning: "反向隧道运行中",
    waitingReversePort: "等待中转服务器监听端口建立",
    starting: "启动中",
    passwordTunnelRunning: "密码隧道运行中",
    passwordTunnelStarting: "后台认证中",
    waitingPassword: "等待密码输入",
    closingConnections: "正在关闭 Connections",
    closingConnectionsDetail: (count) => `检测到 ${count} 个后台连接，正在停止所有隧道，请稍候。`,
    closingConnectionProgress: (index, total, name) => `正在关闭 ${index}/${total}：${name}`,
    closedBeforeExit: "所有 Connections 已关闭",
    closeAppFailed: "关闭程序失败",
    tooltipPublicBundle: "只导出公钥，把选中密钥的 .pub 内容合并到 ssh_public_keys_bundle.pub。",
    tooltipDeleteKeys: "删除选中的私钥和对应 .pub 文件。删除前会再次确认。",
    tooltipExportPortable: "把用户 ~/.ssh 中选择的私钥/公钥复制到程序目录 ssh_keyfiles_export。",
    tooltipImportSsh: "把程序目录 ssh_keyfiles_export 中选择的密钥复制回用户 ~/.ssh，需要先勾选安全开关。",
    tooltipKeySuffix: "可选。只参与生成的密钥文件名。\n例如填 1 会生成 id_ed25519_user_1，\n不影响密钥算法或强度。",
    tooltipKeyPassphrase: "留空会生成免密码私钥，适合自动连接。\n填写后会保护私钥，但每次使用该密钥时可能需要输入这个密码。",
    tooltipServerPassword: "填写后会通过临时 askpass 脚本交给 ssh.exe，\n后台自动安装公钥，不需要手动输入。\n留空时仍会打开终端让你手动输入。",
    tooltipSilentInstall: "必要条件：服务器已能用左侧密钥登录。\n工作方式：先用已有密钥登录服务器，\n再追加上方选择的公钥文件。\n第一次安装请使用终端安装。",
    tooltipWriteSshConfig: "写入当前用户的 ~/.ssh/config。",
    tooltipCopySshConfig: "复制当前 SSH 配置。",
    refreshRunning: "刷新检测中",
    refreshRunningDetail: "正在扫描密钥、读取配置、检测虚拟机 IP 和本地端口。",
    refreshComplete: "刷新完成",
    refreshCompleteDetail: "密钥、配置、虚拟机 IP 和端口状态已更新。",
    refreshFailed: "刷新失败",
    testingKeyLogin: "正在测试密钥登录",
    startStopped: "启动已停止",
    keyLoginReady: "密钥登录可用",
    startTunnelNow: "开始启动隧道",
    missingServerPassword: "缺少服务器密码",
    passwordRequired: "密码认证模式需要填写服务器密码。",
    testingPasswordAuth: "正在测试密码认证",
    passwordAuthFailed: "密码认证失败",
    passwordAuthReady: "密码认证通过",
    startingPasswordTunnel: "正在启动密码隧道",
    preparingPasswordTunnel: "准备启动密码隧道",
    askpassTunnelDetail: "将通过临时 askpass 脚本后台自动认证。",
    missingPasswordTunnelDetail: "未填写服务器密码，可能无法自动认证。",
    startingTunnel: "正在启动隧道",
    tunnelStarted: "隧道进程已启动",
    confirmingTunnel: (pid) => `PID ${pid}，正在确认连接状态`,
    passwordTunnelConnected: "密码隧道已连接",
    tunnelConnected: "隧道已连接",
    reverseTunnelConnected: "反向隧道已连接",
    tunnelExited: "隧道已退出",
    portReleased: "端口已释放",
    portStillBusy: "端口仍被占用",
    tunnelStoppedReleased: "隧道已停止，端口已释放",
    tunnelStoppedBusy: "隧道已停止，但端口仍被占用",
    localPortDetail: (port) => `本地端口 ${port}`,
    remotePortDetail: (port) => `A 远端端口 ${port}`,
    configSaved: "配置已保存",
    profileAdded: "已新增配置",
    profileApplied: "已保存并应用到连接",
    profileSaveFailed: "保存配置失败",
    profileDeleteTitle: "删除配置档案",
    profileDeleteMessage: (name) => `确认删除配置“${name}”？这个操作会立即写入 profiles.json。`,
    delete: "删除",
    profileDeleted: "配置已删除",
    noUserKeysToExport: "没有可导出的用户密钥",
    selectUserKeysToExport: "请选择左侧用户密钥后再导出到程序目录。",
    exportingKeys: "正在导出密钥",
    exportKeysDetail: (count) => `${count} 个用户密钥 -> ssh_keyfiles_export`,
    exportDone: "导出完成",
    exportDoneDetail: (count) => `${count} 个密钥已复制到程序目录。`,
    noPortableKeysToImport: "没有可导入的程序目录密钥",
    selectPortableKeysToImport: "请选择右侧程序目录密钥。",
    importBlocked: "导入被安全开关拦截",
    importBlockedDetail: "请先勾选“允许导入到 ~/.ssh”，再执行导入。",
    importingKeys: "正在导入密钥",
    importKeysDetail: (count) => `${count} 个程序目录密钥 -> ~/.ssh`,
    importDone: "导入完成",
    importDoneDetail: (count) => `${count} 个密钥已复制到用户 ~/.ssh。`,
    noPublicKeys: "没有可导出的公钥",
    selectPublicKeys: "请选择带 .pub 的密钥。bundle 只包含公钥内容。",
    exportingPublicBundle: "正在导出公钥 bundle",
    publicBundleDetail: (count) => `${count} 个公钥将合并为 ssh_public_keys_bundle.pub`,
    publicBundleDone: "公钥 bundle 已生成",
    noKeysToDelete: "没有可删除的密钥",
    selectKeysToDelete: "请先勾选要删除的密钥。",
    deleteKeysMessage: (count, names) => `确认删除 ${count} 个密钥？会同时删除对应的 .pub 文件。\n${names}`,
    deletingKeys: "正在删除密钥",
    deletingKeysDetail: (count) => `${count} 个选中密钥`,
    keysDeleted: "密钥删除完成",
    keysDeletedDetail: (count) => `${count} 个密钥已删除。`,
    settingsUpdated: "设置已更新",
    languageSaved: "语言设置已保存",
    cursorApplied: "鼠标样式已应用",
    userKeyDirOpening: "正在打开用户密钥目录",
    userKeyDirOpened: "用户密钥目录已打开",
    portableKeyDirOpening: "正在打开程序相对目录",
    portableKeyDirOpened: "程序相对目录已打开",
    installFieldsFilled: "安装信息已填充",
    installSuccess: "公钥安装成功",
    installFailed: "公钥安装失败",
    silentInstallDone: "静默安装完成",
    silentInstallFailed: "静默安装失败",
    publicInstallDone: "公钥安装完成",
    terminalInstallOpened: "终端安装已打开",
    keyExistsSelected: "密钥已存在，已选中",
    keyGenerated: "密钥已生成",
    verifyFailed: "验证失败",
    keyVerifyFailed: "密钥登录验证失败",
    writingSshConfig: "正在写入 ~/.ssh/config",
    writeSshConfigDetail: "会写入当前用户的 OpenSSH 配置文件。",
    sshConfigWritten: "~/.ssh/config 已写入",
    pingOk: "Ping 成功",
    pingFailed: "Ping 失败或超时",
    testingSsh: "正在测试 SSH",
    sshReachable: "SSH 服务可达",
    sshCheckFailed: "SSH 检测失败",
    requestingAdmin: "正在请求管理员权限",
    uacDetail: "确认 UAC 后会隐藏执行窗口",
    firewallRequested: "防火墙放行已请求",
    firewallFailed: "防火墙放行失败",
    firewallAllowed: "防火墙已放行",
    sshConfigCopied: "SSH 配置已复制",
    pasteReady: "可以直接粘贴使用。",
    copyFailed: "复制失败",
    logCleared: "日志已清空",
    copied: "已复制",
    charsCopied: (count) => `${count} 个字符`,
    closedRecordsCleared: "已清除断开记录",
    recordsCount: (count) => `${count} 条`,
    busyExporting: "导出中",
    busyImporting: "导入中",
    busyDeleting: "删除中",
    busyOpening: "打开中",
    busyFilling: "填充中",
    busyInstalling: "安装中",
    busyGenerating: "生成中",
    busyVerifying: "验证中",
    busyWriting: "写入中",
    busyCopying: "复制中",
    busyStopping: "停止中",
    busyAllowing: "放行中",
    busyAdding: "新增中",
    busyApplying: "应用中",
    busySaving: "保存中",
    busyStarting: "启动中",
    busyPinging: "Ping 中",
    busyTesting: "测试中",
    busyChecking: "检测中",
    profileOrderSaved: "配置顺序已保存",
    firstProfile: (name) => `优先档案：${name || "无"}`,
    exportFailed: "导出失败",
    importFailed: "导入失败",
    publicBundleFailed: "导出公钥失败",
    deleteKeysFailed: "删除密钥失败",
    openFailed: "打开失败",
    keyGenerateFailed: "生成密钥失败",
    writeSshConfigFailed: "写入 ~/.ssh/config 失败",
  },
  en: {
    help: "Help",
    helpLoadFailed: "Unable to load the help document.",
    refresh: "Refresh",
    refreshing: "Refreshing",
    settings: "Settings",
    keyManagement: "Key Management",
    keyInstall: "Key Setup",
    profileManagement: "Profiles",
    connectConfig: "Connection",
    connections: "Connections",
    logs: "Logs",
    userKeys: "User Keys",
    portableKeys: "Portable Keys",
    openFileLocation: "Open Folder ↗",
    selectAll: "Select All",
    keyActions: "Key Actions",
    selectedCount: (count, names) => `${count} selected: ${names}`,
    notSelected: "None selected",
    allowImport: "Allow import to ~/.ssh",
    exportPublicBundle: "Export Public Bundle",
    deleteSelectedKeys: "Delete Selected Keys",
    exportToPortable: "Export to App Folder",
    importToSsh: "Import to ~/.ssh",
    keygenTitle: "1 Key Generation",
    installTitle: "2 Key Installation",
    verifyTitle: "3 Key Verification",
    serverUser: "Server User",
    serverHost: "Server Host",
    keySuffix: "Key Suffix",
    newKeyName: "New Key Name",
    privatePassphraseOptional: "Private Key Passphrase (optional)",
    generateKey: "Generate Key",
    sshPort: "SSH Port",
    serverPassword: "Server Password",
    publicKeyFile: "Public Key File",
    terminalInstall: "Terminal Install",
    autoFill: "Auto Fill",
    existingServerKey: "Existing Login Key",
    silentInstall: "Silent Install",
    verifyKey: "Key for Verification",
    verifyLogin: "Verify Key Login",
    profileList: "Profiles",
    add: "Add",
    editProfiles: "Edit profiles.json",
    deleteProfile: "Delete Profile",
    save: "Save",
    applyToConnect: "Apply to Connection",
    profileName: "Profile Name",
    sshAlias: "SSH Alias",
    serverPort: "Server Port",
    localStartPort: "Local Start Port",
    reverseTunnel: "Reverse Tunnel",
    yes: "Yes",
    no: "No",
    remotePortOnA: "Relay Server Listening Port",
    reverseTargetUser: "Target Host Login User",
    reverseTargetPort: "Target Host SSH Port",
    authMode: "Authentication",
    keyAuth: "Use Key",
    passwordAuth: "Use Password",
    optionalKey: "Optional Key",
    useProfile: "Profile",
    localPort: "Local Port",
    vmIp: "VM IP",
    autoVmIp: "Auto-detect VM IP",
    on: "On",
    off: "Off",
    tunnelControl: "Tunnel Control",
    notStarted: "Not Started",
    start: "Start",
    autoReconnect: "Auto reconnect",
    reconnectDelay: "Reconnect delay",
    pingServer: "Ping Server",
    sshTestServer: "SSH Test",
    firewall: "Allow Firewall",
    refreshPort: "Check Port",
    sshConfig: "SSH Config",
    writeSshConfig: "Write ~/.ssh/config",
    copy: "Copy",
    runningCount: (running) => `${running} running`,
    connectionCount: (running, total) => `${running} running / ${total} records`,
    clearClosed: "Clear Closed Records",
    clear: "Clear",
    language: "Language",
    zhPreferred: "Chinese (preferred)",
    english: "English",
    cursorStyle: "Cursor Style",
    systemDefault: "System Default",
    confirmOperation: "Confirm Action",
    cancel: "Cancel",
    confirm: "Confirm",
    noConnections: "No connections",
    noConnectionsHint: "Started tunnels will appear here and can be stopped individually.",
    viewConfig: "View Config",
    stop: "Stop",
    localToRemote: (local, remote) => `Local ${local} -> Remote 127.0.0.1:${remote}`,
    reverseToLocal: (remote, target) => `Relay 127.0.0.1:${remote} -> Target 127.0.0.1:${target}`,
    noVmIp: "No VM IP detected",
    portAvailable: "Port available",
    usingPort: (port) => `Using ${port}`,
    noKey: "None",
    noPublicKey: "Do not auto-select",
    chooseInstalledKey: "Choose a key already installed on the server",
    chooseVerifyKey: "Choose a key to verify login",
    savedMissing: "saved, not scanned",
    userKeySource: "User key ~/.ssh",
    portableKeySource: "App folder key",
    noFingerprint: "No public key fingerprint found",
    configuredPassword: "Password saved",
    configuredKeyPassphrase: "Key passphrase saved",
    unspecifiedKey: "No key selected",
    running: "Running",
    reverseRunning: "Reverse tunnel running",
    waitingReversePort: "Waiting for the relay server listening port",
    starting: "Starting",
    passwordTunnelRunning: "Password tunnel running",
    passwordTunnelStarting: "Authenticating",
    waitingPassword: "Waiting for password",
    closingConnections: "Closing Connections",
    closingConnectionsDetail: (count) => `${count} background connection(s) found. Stopping all tunnels before exit.`,
    closingConnectionProgress: (index, total, name) => `Closing ${index}/${total}: ${name}`,
    closedBeforeExit: "All Connections closed",
    closeAppFailed: "Failed to close app",
    tooltipPublicBundle: "Export public keys only. The selected .pub files are merged into ssh_public_keys_bundle.pub.",
    tooltipDeleteKeys: "Delete selected private keys and matching .pub files. A confirmation is required.",
    tooltipExportPortable: "Copy selected private/public keys from user ~/.ssh to ssh_keyfiles_export in the app folder.",
    tooltipImportSsh: "Copy selected keys from ssh_keyfiles_export back to user ~/.ssh. The safety switch must be enabled first.",
    tooltipKeySuffix: "Optional. Used only as the generated key filename suffix.\nFor example, 1 creates id_ed25519_user_1.\nIt does not affect key algorithm or strength.",
    tooltipKeyPassphrase: "Leave empty to create a key without passphrase, suitable for automatic connection.\nIf filled, the private key is protected and may require this passphrase when used.",
    tooltipServerPassword: "If filled, a temporary askpass script passes it to ssh.exe,\nso public key installation can run in the background.\nIf empty, a terminal opens for manual password input.",
    tooltipSilentInstall: "Requirement: the server can already be accessed with the key on the left.\nHow it works: log in with the existing key,\nthen append the selected public key.\nFor first-time setup, use Terminal Install.",
    tooltipWriteSshConfig: "Write to the current user's ~/.ssh/config.",
    tooltipCopySshConfig: "Copy the current SSH config.",
    refreshRunning: "Refreshing",
    refreshRunningDetail: "Scanning keys, loading profiles, detecting VM IP and local port.",
    refreshComplete: "Refresh complete",
    refreshCompleteDetail: "Keys, profiles, VM IP, and port status are up to date.",
    refreshFailed: "Refresh failed",
    testingKeyLogin: "Testing key login",
    startStopped: "Start stopped",
    keyLoginReady: "Key login ready",
    startTunnelNow: "Starting tunnel",
    missingServerPassword: "Missing server password",
    passwordRequired: "Password authentication requires a server password.",
    testingPasswordAuth: "Testing password authentication",
    passwordAuthFailed: "Password authentication failed",
    passwordAuthReady: "Password authentication ready",
    startingPasswordTunnel: "Starting password tunnel",
    preparingPasswordTunnel: "Preparing password tunnel",
    askpassTunnelDetail: "A temporary askpass script will authenticate in the background.",
    missingPasswordTunnelDetail: "Server password is empty, so automatic authentication may fail.",
    startingTunnel: "Starting tunnel",
    tunnelStarted: "Tunnel process started",
    confirmingTunnel: (pid) => `PID ${pid}, checking connection status`,
    passwordTunnelConnected: "Password tunnel connected",
    tunnelConnected: "Tunnel connected",
    reverseTunnelConnected: "Reverse tunnel connected",
    tunnelExited: "Tunnel exited",
    portReleased: "Port released",
    portStillBusy: "Port still busy",
    tunnelStoppedReleased: "Tunnel stopped, port released",
    tunnelStoppedBusy: "Tunnel stopped, but port is still busy",
    localPortDetail: (port) => `Local port ${port}`,
    remotePortDetail: (port) => `Remote port on A ${port}`,
    configSaved: "Profile saved",
    profileAdded: "Profile added",
    profileApplied: "Saved and applied to Connection",
    profileSaveFailed: "Failed to save profile",
    profileDeleteTitle: "Delete Profile",
    profileDeleteMessage: (name) => `Delete profile "${name}"? This will be written to profiles.json immediately.`,
    delete: "Delete",
    profileDeleted: "Profile deleted",
    noUserKeysToExport: "No user keys to export",
    selectUserKeysToExport: "Select user keys on the left before exporting to the app folder.",
    exportingKeys: "Exporting keys",
    exportKeysDetail: (count) => `${count} user key(s) -> ssh_keyfiles_export`,
    exportDone: "Export complete",
    exportDoneDetail: (count) => `${count} key(s) copied to the app folder.`,
    noPortableKeysToImport: "No portable keys to import",
    selectPortableKeysToImport: "Select app-folder keys on the right.",
    importBlocked: "Import blocked by safety switch",
    importBlockedDetail: "Enable \"Allow import to ~/.ssh\" before importing.",
    importingKeys: "Importing keys",
    importKeysDetail: (count) => `${count} app-folder key(s) -> ~/.ssh`,
    importDone: "Import complete",
    importDoneDetail: (count) => `${count} key(s) copied to user ~/.ssh.`,
    noPublicKeys: "No public keys to export",
    selectPublicKeys: "Select keys with .pub files. The bundle contains public keys only.",
    exportingPublicBundle: "Exporting public key bundle",
    publicBundleDetail: (count) => `${count} public key(s) will be merged into ssh_public_keys_bundle.pub`,
    publicBundleDone: "Public key bundle generated",
    noKeysToDelete: "No keys to delete",
    selectKeysToDelete: "Select keys to delete first.",
    deleteKeysMessage: (count, names) => `Delete ${count} key(s)? Matching .pub files will also be deleted.\n${names}`,
    deletingKeys: "Deleting keys",
    deletingKeysDetail: (count) => `${count} selected key(s)`,
    keysDeleted: "Keys deleted",
    keysDeletedDetail: (count) => `${count} key(s) deleted.`,
    settingsUpdated: "Settings updated",
    languageSaved: "Language saved",
    cursorApplied: "Cursor style applied",
    userKeyDirOpening: "Opening user key folder",
    userKeyDirOpened: "User key folder opened",
    portableKeyDirOpening: "Opening app key folder",
    portableKeyDirOpened: "App key folder opened",
    installFieldsFilled: "Install fields filled",
    installSuccess: "Public key installed",
    installFailed: "Public key installation failed",
    silentInstallDone: "Silent install complete",
    silentInstallFailed: "Silent install failed",
    publicInstallDone: "Public key installation complete",
    terminalInstallOpened: "Terminal install opened",
    keyExistsSelected: "Key already exists and selected",
    keyGenerated: "Key generated",
    verifyFailed: "Verification failed",
    keyVerifyFailed: "Key login verification failed",
    writingSshConfig: "Writing ~/.ssh/config",
    writeSshConfigDetail: "This writes to the current user's OpenSSH config file.",
    sshConfigWritten: "~/.ssh/config written",
    pingOk: "Ping succeeded",
    pingFailed: "Ping failed or timed out",
    testingSsh: "Testing SSH",
    sshReachable: "SSH reachable",
    sshCheckFailed: "SSH check failed",
    requestingAdmin: "Requesting administrator permission",
    uacDetail: "Confirm UAC; the execution window will be hidden.",
    firewallRequested: "Firewall rule requested",
    firewallFailed: "Firewall rule failed",
    firewallAllowed: "Firewall allowed",
    sshConfigCopied: "SSH config copied",
    pasteReady: "Ready to paste.",
    copyFailed: "Copy failed",
    logCleared: "Log cleared",
    copied: "Copied",
    charsCopied: (count) => `${count} characters`,
    closedRecordsCleared: "Closed records cleared",
    recordsCount: (count) => `${count} record(s)`,
    busyExporting: "Exporting",
    busyImporting: "Importing",
    busyDeleting: "Deleting",
    busyOpening: "Opening",
    busyFilling: "Filling",
    busyInstalling: "Installing",
    busyGenerating: "Generating",
    busyVerifying: "Verifying",
    busyWriting: "Writing",
    busyCopying: "Copying",
    busyStopping: "Stopping",
    busyAllowing: "Allowing",
    busyAdding: "Adding",
    busyApplying: "Applying",
    busySaving: "Saving",
    busyStarting: "Starting",
    busyPinging: "Pinging",
    busyTesting: "Testing",
    busyChecking: "Checking",
    profileOrderSaved: "Profile order saved",
    firstProfile: (name) => `First profile: ${name || "none"}`,
    exportFailed: "Export failed",
    importFailed: "Import failed",
    publicBundleFailed: "Public bundle export failed",
    deleteKeysFailed: "Delete keys failed",
    openFailed: "Open failed",
    keyGenerateFailed: "Generate key failed",
    writeSshConfigFailed: "Failed to write ~/.ssh/config",
  },
};

const SAVED_PROFILE_KEY_PREFIX = "__saved_profile_key__:";

const LOG_EN = {
  "读取 app_settings.json 失败": "Failed to read app_settings.json",
  "保存 app_settings.json 失败": "Failed to save app_settings.json",
  "选择配置档案": "Selected profile",
  "配置档案顺序已更新": "Profile order updated",
  "保存配置顺序失败": "Failed to save profile order",
  "读取 profiles.json": "Loaded profiles.json",
  "选择密钥": "Selected key",
  "取消选择密钥": "Deselected key",
  "扫描密钥": "Scanned keys",
  "检测虚拟机 IP": "Detected VM IP",
  "检测本地端口": "Checked local port",
  "密码模式说明": "Password mode note",
  "启动前 SSH 测试": "Pre-start SSH test",
  "启动前 SSH 测试通过": "Pre-start SSH test passed",
  "启动前 SSH 测试失败": "Pre-start SSH test failed",
  "启动前密码认证失败": "Pre-start password authentication failed",
  "启动前密码认证测试": "Pre-start password authentication test",
  "启动前密码认证通过": "Pre-start password authentication passed",
  "刷新完成": "Refresh complete",
  "刷新失败": "Refresh failed",
  "隧道进程已退出": "Tunnel process exited",
  "状态检查失败": "Status check failed",
  "启动密码模式隧道": "Starting password-mode tunnel",
  "密码模式隧道已后台启动": "Password-mode tunnel started in background",
  "启动隧道": "Starting tunnel",
  "隧道进程已启动": "Tunnel process started",
  "当前没有已记录的隧道进程": "No recorded tunnel process found",
  "停止隧道": "Stopping tunnel",
  "隧道已停止，端口已释放": "Tunnel stopped, port released",
  "隧道已停止，端口仍被占用": "Tunnel stopped, port still busy",
  "关闭程序前停止连接": "Stopping connection before app exit",
  "停止连接命令返回异常": "Stop connection command returned an error",
  "窗口关闭失败": "Failed to close window",
  "关闭程序前连接清理完成": "Connection cleanup before exit complete",
  "关闭程序失败": "Failed to close app",
  "保存 profiles.json": "Saved profiles.json",
  "没有需要导出的用户密钥": "No user keys to export",
  "导出密钥到程序目录": "Exported keys to app folder",
  "导出完成": "Export complete",
  "没有需要导入的程序目录密钥": "No app-folder keys to import",
  "导入被关闭": "Import blocked",
  "导入密钥到 ~/.ssh": "Imported keys to ~/.ssh",
  "导入完成": "Import complete",
  "没有可导出的公钥": "No public keys to export",
  "导出公钥 bundle": "Exported public key bundle",
  "公钥 bundle 已生成": "Public key bundle generated",
  "没有可删除的密钥": "No keys to delete",
  "删除选定密钥": "Deleted selected keys",
  "密钥删除完成": "Key deletion complete",
  "请选择公钥文件": "Please select a public key file",
  "静默安装缺少登录密钥": "Silent install missing login key",
  "静默写入 authorized_keys": "Silent write to authorized_keys",
  "公钥安装成功": "Public key installation succeeded",
  "公钥安装失败": "Public key installation failed",
  "密码模式安装 authorized_keys": "Password-mode authorized_keys installation",
  "打开终端安装": "Opened terminal install",
  "交互式安装脚本": "Interactive install script",
  "安装密钥已存在，已选中": "Install key already exists and was selected",
  "生成安装密钥": "Generating install key",
  "安装密钥已生成": "Install key generated",
  "无法验证密钥登录": "Cannot verify key login",
  "验证密钥登录": "Verifying key login",
  "密钥登录验证通过": "Key login verification passed",
  "密钥登录验证失败": "Key login verification failed",
  "写入 ~/.ssh/config": "Writing ~/.ssh/config",
  "~/.ssh/config 已写入": "~/.ssh/config written",
  "Ping 服务器": "Pinging server",
  "Ping 成功": "Ping succeeded",
  "Ping 失败或超时": "Ping failed or timed out",
  "后台 SSH 测试服务器": "Background SSH server test",
  "SSH 服务可达": "SSH service reachable",
  "SSH 检测失败": "SSH check failed",
  "检查管理员模式": "Checking administrator mode",
  "当前不是管理员模式": "Not running as administrator",
  "防火墙提权命令已完成": "Firewall elevation command completed",
  "防火墙提权命令失败": "Firewall elevation command failed",
  "放行防火墙": "Allowing firewall rule",
  "防火墙规则已确认": "Firewall rule confirmed",
  "防火墙规则设置失败": "Firewall rule setup failed",
  "窗口拖动失败": "Window drag failed",
  "窗口最大化失败": "Window maximize failed",
  "窗口最小化失败": "Window minimize failed",
  "保存设置失败": "Failed to save settings",
  "新增配置档案": "Added profile",
  "保存并应用配置到连接页": "Saved and applied profile to Connection page",
  "连接页应用档案": "Applied profile on Connection page",
  "保存失败": "Save failed",
  "没有可删除的配置": "No profile to delete",
  "删除配置档案": "Deleted profile",
  "启动失败": "Start failed",
  "Ping 失败": "Ping failed",
  "SSH 测试失败": "SSH test failed",
  "防火墙设置失败": "Firewall setup failed",
  "端口检测失败": "Port check failed",
  "导出失败": "Export failed",
  "导入失败": "Import failed",
  "导出公钥失败": "Public key export failed",
  "删除密钥失败": "Delete keys failed",
  "打开用户密钥目录": "Opening user key folder",
  "打开用户密钥目录失败": "Failed to open user key folder",
  "打开程序相对目录密钥位置": "Opening app key folder",
  "打开程序相对目录失败": "Failed to open app key folder",
  "自动填充安装信息": "Auto-filled install fields",
  "打开终端失败": "Failed to open terminal",
  "生成密钥失败": "Failed to generate key",
  "验证密钥登录失败": "Failed to verify key login",
  "写入失败": "Write failed",
  "已复制 SSH 配置": "SSH config copied",
  "复制 SSH 配置失败": "Failed to copy SSH config",
  "停止失败": "Stop failed",
  "初始化失败": "Initialization failed",
};

function lang() {
  return state.appSettings?.language === "en" ? "en" : "zh";
}

function tr(key, ...args) {
  const value = I18N[lang()][key] ?? I18N.zh[key] ?? key;
  return typeof value === "function" ? value(...args) : value;
}

function setText(selector, key) {
  const element = document.querySelector(selector);
  if (element) element.textContent = tr(key);
}

function setButton(id, key) {
  const element = $(id);
  if (element) element.textContent = tr(key);
}

function setLabelText(inputId, key) {
  const label = $(inputId)?.closest("label");
  if (!label) return;
  const textNode = [...label.childNodes].find((node) => node.nodeType === Node.TEXT_NODE && node.textContent.trim());
  if (textNode) {
    textNode.textContent = `\n                ${tr(key)}\n                `;
  }
}

function setSelectOptionText(selectId, value, key) {
  const option = $(`${selectId}`)?.querySelector(`option[value="${CSS.escape(value)}"]`);
  if (option) option.textContent = tr(key);
}

function setTooltip(selectorOrId, key) {
  const element = selectorOrId.startsWith("#") || selectorOrId.startsWith(".")
    ? document.querySelector(selectorOrId)
    : $(selectorOrId);
  if (element) element.dataset.tooltip = tr(key);
}

function translateLogText(text) {
  if (lang() !== "en" || text == null) return text || "";
  let translated = LOG_EN[text] || String(text);
  translated = translated
    .replace(/^全选用户密钥$/, "Selected all user keys")
    .replace(/^全选程序目录密钥$/, "Selected all app-folder keys")
    .replace(/^全选密钥$/, "Selected all keys")
    .replace(/^取消全选用户密钥$/, "Deselected all user keys")
    .replace(/^取消全选程序目录密钥$/, "Deselected all app-folder keys")
    .replace(/^取消全选密钥$/, "Deselected all keys")
    .replace(/(\d+) 个连接/g, "$1 connection(s)")
    .replace(/(\d+) 个密钥/g, "$1 key(s)")
    .replace(/(\d+) 个公钥/g, "$1 public key(s)")
    .replace(/(\d+) 个/g, "$1 item(s)")
    .replace(/(\d+) 条/g, "$1 record(s)")
    .replace(/本地端口/g, "local port")
    .replace(/密钥:/g, "key:")
    .replace(/使用已有密钥/g, "existing login key")
    .replace(/验证私钥/g, "private key")
    .replace(/参考公钥/g, "public key")
    .replace(/目标/g, "target")
    .replace(/未选择/g, "not selected")
    .replace(/未指定/g, "not specified")
    .replace(/请先打开“允许导入到 ~\/.ssh”开关。/g, "Enable the \"Allow import to ~/.ssh\" switch first.")
    .replace(/请选择一把已经安装在服务器上的私钥。/g, "Select a private key that is already installed on the server.")
    .replace(/请选择验证使用密钥。/g, "Select a key for login verification.")
    .replace(/密码模式仅作为测试\/备用功能；默认建议使用密钥认证。/g, "Password mode is intended as a test/fallback path; key authentication is recommended by default.")
    .replace(/未填写服务器密码。/g, "Server password is empty.")
    .replace(/将请求 UAC 提权来放行 SSH-Tunnel-Range。/g, "A UAC elevation request will allow SSH-Tunnel-Range.");
  return translated;
}

function log(message, detail = "") {
  const now = new Date().toLocaleTimeString();
  const translatedMessage = translateLogText(message);
  const translatedDetail = translateLogText(detail);
  const suffix = translatedDetail ? `\n    ${translatedDetail}` : "";
  $("log").textContent += `[${now}] ${translatedMessage}${suffix}\n`;
  $("log").scrollTop = $("log").scrollHeight;
}

function showToast(title, detail = "", type = "info", timeout = 3600) {
  const toast = document.createElement("div");
  toast.className = `toast ${type}`;
  toast.innerHTML = `
    <button class="toast-close" type="button" aria-label="关闭提示">×</button>
    <div class="toast-content">
      <div class="toast-title">${title}</div>
      ${detail ? `<div class="toast-detail">${detail}</div>` : ""}
    </div>
  `;
  $("toastDock").appendChild(toast);

  let removeTimer = null;
  const dismiss = () => {
    if (toast.classList.contains("fade-out")) return;
    if (removeTimer) clearTimeout(removeTimer);
    toast.classList.add("fade-out");
    removeTimer = setTimeout(() => toast.remove(), 500);
  };

  toast.querySelector(".toast-close").addEventListener("click", dismiss);
  removeTimer = setTimeout(dismiss, timeout);
}

async function runButtonAction(buttonOrId, busyText, action) {
  const button = typeof buttonOrId === "string" ? $(buttonOrId) : buttonOrId;
  if (!button || button.disabled) return;
  const oldText = button.textContent;
  const oldWidth = button.style.width;
  button.style.width = `${Math.ceil(button.getBoundingClientRect().width)}px`;
  button.disabled = true;
  button.classList.add("loading", "is-busy");
  if (busyText) button.textContent = busyText;
  try {
    return await action();
  } finally {
    button.disabled = false;
    button.classList.remove("loading", "is-busy");
    button.textContent = oldText;
    button.style.width = oldWidth;
  }
}

function confirmAction(title, message, confirmLabel = "确认") {
  $("confirmTitle").textContent = title;
  $("confirmMessage").textContent = message;
  $("confirmOk").textContent = confirmLabel;
  $("confirmModal").classList.remove("hidden");

  return new Promise((resolve) => {
    const finish = (value) => {
      $("confirmModal").classList.add("hidden");
      $("confirmOk").removeEventListener("click", ok);
      $("confirmCancel").removeEventListener("click", cancel);
      $("confirmModal").removeEventListener("click", backdrop);
      resolve(value);
    };
    const ok = () => finish(true);
    const cancel = () => finish(false);
    const backdrop = (event) => {
      if (event.target === $("confirmModal")) finish(false);
    };
    $("confirmOk").addEventListener("click", ok);
    $("confirmCancel").addEventListener("click", cancel);
    $("confirmModal").addEventListener("click", backdrop);
  });
}

function mergeAppSettings(settings = {}) {
  return {
    ...DEFAULT_APP_SETTINGS,
    ...settings,
    defaults: {
      ...DEFAULT_APP_SETTINGS.defaults,
      ...(settings.defaults || {}),
    },
  };
}

async function loadSettings() {
  try {
    state.appSettings = mergeAppSettings(await invoke("load_app_settings"));
  } catch (error) {
    state.appSettings = mergeAppSettings({
      language: localStorage.getItem("tunnelHelper.language") || "zh",
      cursor: localStorage.getItem("tunnelHelper.cursor") || "NArrowBeadCursor",
    });
    log("读取 app_settings.json 失败", String(error));
  }
  return state.appSettings;
}

function currentDefaultSettings() {
  return {
    keygenUser: $("keygenUser").value.trim(),
    keygenHost: $("keygenHost").value.trim(),
    keygenOffset: $("keygenOffset").value.trim(),
    installKeyName: $("installKeyName").value.trim(),
    installUser: $("installUser").value.trim(),
    installHost: $("installHost").value.trim(),
    installPort: Number($("installPort").value) || 22,
  };
}

function currentAppSettings() {
  return mergeAppSettings({
    language: $("languageSelect").value,
    cursor: $("cursorSelect").value,
    defaults: currentDefaultSettings(),
  });
}

async function saveSettings() {
  state.appSettings = currentAppSettings();
  await invoke("save_app_settings", { settings: state.appSettings });
}

function saveSettingsQuietly() {
  saveSettings().catch((error) => log("保存 app_settings.json 失败", String(error)));
}

function applyDefaultSettings(defaults) {
  $("keygenUser").value = defaults.keygenUser;
  $("keygenHost").value = defaults.keygenHost;
  $("keygenOffset").value = defaults.keygenOffset;
  $("installKeyName").value = defaults.installKeyName;
  $("installUser").value = defaults.installUser;
  $("installHost").value = defaults.installHost;
  $("installPort").value = defaults.installPort;
}

function applySettings() {
  const settings = state.appSettings || DEFAULT_APP_SETTINGS;
  $("languageSelect").value = settings.language;
  $("cursorSelect").value = settings.cursor;
  document.documentElement.lang = settings.language === "en" ? "en" : "zh-CN";
  document.documentElement.dataset.cursor = settings.cursor;
  applyTranslations();
}

function applyTranslations() {
  setButton("refreshAll", "refresh");
  $("settingsButton").setAttribute("aria-label", tr("settings"));
  $("helpButton").setAttribute("aria-label", tr("help"));
  $("helpButton").title = tr("help");
  $("helpTitle").textContent = tr("help");
  setText('[data-page="keysPage"]', "keyManagement");
  setText('[data-page="installPage"]', "keyInstall");
  setText('[data-page="profilesPage"]', "profileManagement");
  setText('[data-page="connectPage"]', "connectConfig");
  setText('[data-page="connectionsPage"]', "connections");
  setText('[data-page="logsPage"]', "logs");

  setText("#keysPage .panel:nth-of-type(1) h2", "userKeys");
  setText("#keysPage .panel:nth-of-type(2) h2", "portableKeys");
  setButton("openUserKeyDir", "openFileLocation");
  setButton("openPortableKeyDir", "openFileLocation");
  setText("#keysPage .panel:nth-of-type(3) h2", "keyActions");
  setButton("exportPublicBundle", "exportPublicBundle");
  setButton("deleteSelectedKeys", "deleteSelectedKeys");
  setButton("exportSelectedKey", "exportToPortable");
  setButton("importSelectedKey", "importToSsh");
  setTooltip("exportPublicBundle", "tooltipPublicBundle");
  setTooltip("deleteSelectedKeys", "tooltipDeleteKeys");
  setTooltip("exportSelectedKey", "tooltipExportPortable");
  setTooltip("importSelectedKey", "tooltipImportSsh");

  document.querySelectorAll("label.check").forEach((label) => {
    const input = label.querySelector("input");
    if (!input) return;
    const key = input.id === "allowImport" ? "allowImport" : "selectAll";
    const textNode = [...label.childNodes].find((node) => node.nodeType === Node.TEXT_NODE && node.textContent.trim());
    if (textNode) textNode.textContent = ` ${tr(key)}`;
  });

  setText("#installPage article:nth-of-type(1) h2", "keygenTitle");
  setText("#installPage article:nth-of-type(2) h2", "installTitle");
  setText("#installPage article:nth-of-type(3) h2", "verifyTitle");
  setButton("generateInstallKey", "generateKey");
  setButton("syncInstallFields", "autoFill");
  setButton("installPublicKeyTerminal", "terminalInstall");
  setButton("installPublicKey", "silentInstall");
  setButton("verifyInstalledKey", "verifyLogin");
  setLabelText("keygenUser", "serverUser");
  setLabelText("keygenHost", "serverHost");
  setLabelText("installKeyName", "newKeyName");
  const labelRows = [
    ["keygenOffset", "keySuffix"],
    ["keygenPassphrase", "privatePassphraseOptional"],
    ["installPassword", "serverPassword"],
  ];
  for (const [inputId, key] of labelRows) {
    const row = $(inputId)?.closest("label")?.querySelector(".label-row");
    const textNode = row ? [...row.childNodes].find((node) => node.nodeType === Node.TEXT_NODE && node.textContent.trim()) : null;
    if (textNode) textNode.textContent = `\n                      ${tr(key)}\n                      `;
  }
  setTooltip("#keygenOffsetInfo", "tooltipKeySuffix");
  setTooltip("#keygenPassphraseInfo", "tooltipKeyPassphrase");
  setTooltip("#installPasswordInfo", "tooltipServerPassword");
  setTooltip("#silentInstallInfo", "tooltipSilentInstall");
  setLabelText("installUser", "serverUser");
  setLabelText("installHost", "serverHost");
  setLabelText("installPort", "sshPort");
  setLabelText("publicKeySelect", "publicKeyFile");
  setLabelText("silentIdentityKey", "existingServerKey");
  setLabelText("verifyIdentityKey", "verifyKey");
  setLabelText("verifyIdentityPassphrase", "privatePassphraseOptional");

  setText("#profilesPage article:nth-of-type(1) h2", "profileList");
  setText("#profilesPage article:nth-of-type(2) h2", "editProfiles");
  setButton("newProfile", "add");
  setButton("deleteProfile", "deleteProfile");
  setButton("saveProfiles", "save");
  setButton("applyProfile", "applyToConnect");
  setLabelText("profileName", "profileName");
  setLabelText("profileAlias", "sshAlias");
  setLabelText("profileUser", "serverUser");
  setLabelText("profileHost", "serverHost");
  setLabelText("profileServerPort", "serverPort");
  setLabelText("profileLocalPort", $("profileReverseTunnel").value === "true" ? "remotePortOnA" : "localStartPort");
  setLabelText("profileReverseTunnel", "reverseTunnel");
  setLabelText("profileReverseTargetUser", "reverseTargetUser");
  setLabelText("profileReverseTargetPort", "reverseTargetPort");
  setLabelText("profileAuthMode", "authMode");
  setLabelText("profileKeyName", "optionalKey");
  setLabelText("profileKeyPassphrase", "privatePassphraseOptional");
  setLabelText("profilePassword", "serverPassword");

  setText("#connectPage article:nth-of-type(1) h2", "connectConfig");
  setText("#connectPage article:nth-of-type(2) h2", "tunnelControl");
  setLabelText("connectProfile", "useProfile");
  setLabelText("authMode", "authMode");
  setLabelText("serverUser", "serverUser");
  setLabelText("serverHost", "serverHost");
  setLabelText("serverPort", "serverPort");
  setLabelText("reverseTunnel", "reverseTunnel");
  setLabelText("localPort", $("reverseTunnel").value === "true" ? "remotePortOnA" : "localPort");
  setLabelText("reverseTargetUser", "reverseTargetUser");
  setLabelText("reverseTargetPort", "reverseTargetPort");
  setLabelText("vmIp", "vmIp");
  setLabelText("autoVmIp", "autoVmIp");
  setLabelText("connectKey", "optionalKey");
  setLabelText("connectKeyPassphrase", "privatePassphraseOptional");
  setLabelText("serverPassword", "serverPassword");
  setLabelText("hostAlias", "sshAlias");
  setButton("startTunnel", "start");
  setButton("openConnections", "connections");
  setLabelText("reconnectDelay", "reconnectDelay");
  setButton("pingHost", "pingServer");
  setButton("sshTest", "sshTestServer");
  setButton("firewallRule", "firewall");
  setButton("refreshPort", "refreshPort");
  setText("#connectPage article:nth-of-type(3) h2", "sshConfig");
  setButton("writeConfig", "writeSshConfig");
  setButton("copyConfig", "copy");
  setTooltip("writeConfig", "tooltipWriteSshConfig");
  setTooltip("copyConfig", "tooltipCopySshConfig");

  setText("#connectionsPage h2", "connections");
  setButton("clearClosedConnections", "clearClosed");
  setText("#logsPage h2", "logs");
  setButton("clearLog", "clear");
  setText("#settingsModal h2", "settings");
  setLabelText("languageSelect", "language");
  setLabelText("cursorSelect", "cursorStyle");
  setText("#confirmTitle", "confirmOperation");
  setButton("confirmCancel", "cancel");
  setButton("confirmOk", "confirm");
  setText("#shutdownTitle", "closingConnections");
  setButton("contextCopy", "copy");

  setSelectOptionText("languageSelect", "zh", "zhPreferred");
  setSelectOptionText("languageSelect", "en", "english");
  setSelectOptionText("cursorSelect", "default", "systemDefault");
  setSelectOptionText("profileAuthMode", "key", "keyAuth");
  setSelectOptionText("profileAuthMode", "password", "passwordAuth");
  setSelectOptionText("authMode", "key", "keyAuth");
  setSelectOptionText("authMode", "password", "passwordAuth");
  setSelectOptionText("profileReverseTunnel", "false", "no");
  setSelectOptionText("profileReverseTunnel", "true", "yes");
  setSelectOptionText("reverseTunnel", "false", "no");
  setSelectOptionText("reverseTunnel", "true", "yes");
  setSelectOptionText("autoVmIp", "on", "on");
  setSelectOptionText("autoVmIp", "off", "off");

  renderSelectionState();
  updateProfileTunnelMode();
  updateTunnelMode();
  updateTunnelSummary();
  updateConnectionsTabBadge();
  renderKeySelects();
  renderKeyLists();
  renderProfileLists();
  renderConnections();
}

function selectedText() {
  const active = document.activeElement;
  if (active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA")) {
    const start = active.selectionStart ?? 0;
    const end = active.selectionEnd ?? 0;
    return start === end ? "" : active.value.slice(start, end);
  }

  return window.getSelection().toString();
}

function hideContextMenu() {
  $("contextMenu").classList.remove("active");
}

function isUserKey(key) {
  return key.source_dir.toLowerCase().endsWith("\\.ssh") || key.source_dir.toLowerCase().endsWith("/.ssh");
}

function keySourceLabel(key) {
  return isUserKey(key) ? tr("userKeySource") : tr("portableKeySource");
}

function keyOptionLabel(key, suffix = "") {
  return `${key.name}${suffix} (${keySourceLabel(key)})`;
}

function selectedIdentityFile() {
  if ($("authMode").value === "password") return null;
  const keyPath = $("connectKey").value;
  return keyPath || state.selectedKey?.private_path || null;
}

function selectedIdentityPassphrase() {
  if ($("authMode").value === "password") return null;
  return $("connectKeyPassphrase").value || null;
}

function selectedPublicKeyFile() {
  return $("publicKeySelect").value || null;
}

function selectedSilentIdentityFile() {
  return $("silentIdentityKey").value || null;
}

function selectedVerifyIdentityFile() {
  return $("verifyIdentityKey").value || null;
}

function selectedVerifyIdentityPassphrase() {
  return $("verifyIdentityPassphrase").value || null;
}

function installTarget() {
  return `${$("installUser").value.trim()}@${$("installHost").value.trim()}`;
}

function keygenTarget() {
  return `${$("keygenUser").value.trim()}@${$("keygenHost").value.trim()}`;
}

function privateKeyForPublicPath(publicPath) {
  return state.keys.find((key) => key.public_path === publicPath)?.private_path || null;
}

function defaultGeneratedKeyName() {
  const base = $("keygenUser").value.trim() || "tunnel";
  const offset = $("keygenOffset").value.trim();
  return offset ? `id_ed25519_${base}_${offset}` : `id_ed25519_${base}`;
}

function generatedKeyName() {
  return $("installKeyName").value.trim() || defaultGeneratedKeyName();
}

function keyByInstallName(name) {
  return state.keys.find((key) => key.name === name || key.name === `id_ed25519_${name}`);
}

function updateGeneratedKeyName() {
  $("installKeyName").value = defaultGeneratedKeyName();
}

function syncInstallFields(selectPublicKey = false) {
  $("installUser").value = $("keygenUser").value;
  $("installHost").value = $("keygenHost").value;

  if (selectPublicKey) {
    const existing = keyByInstallName(generatedKeyName());
    $("publicKeySelect").value = existing?.public_path || "";
  }

  updateInstallActionState();
}

function updateInstallActionState() {
  const hasPublicKey = Boolean(selectedPublicKeyFile());
  const hasSilentIdentity = Boolean(selectedSilentIdentityFile());
  $("installPublicKey").disabled = !hasPublicKey || !hasSilentIdentity;
  $("installPublicKeyTerminal").disabled = !hasPublicKey;
  $("verifyInstalledKey").disabled = !selectedVerifyIdentityFile();
}

function setInstallStatus(text) {
  const status = $("installStatus");
  if (status) status.textContent = text;
}

function keyByName(name) {
  return state.keys.find((key) => key.name === name);
}

function keyByPath(path) {
  return state.keys.find((key) => key.private_path === path);
}

function selectedKeyObjects() {
  return state.keys.filter((key) => state.selectedKeys.has(key.private_path));
}

function keyForProfile(profile) {
  return profile.identityFilePath ? keyByPath(profile.identityFilePath) : profile.identityFileName ? keyByName(profile.identityFileName) : null;
}

function savedProfileKeyValue(profile) {
  return profile?.identityFilePath || profile?.identityFileName || "";
}

function savedProfileKeyLabel(profile) {
  return profile?.identityFileName || profile?.identityFilePath || tr("noKey");
}

function savedProfileKeyOptionValue(profile) {
  const value = savedProfileKeyValue(profile);
  return value ? `${SAVED_PROFILE_KEY_PREFIX}${encodeURIComponent(value)}` : "";
}

function redactedProfile(profile) {
  return {
    ...profile,
    identityPassphrase: profile.identityPassphrase ? "***" : null,
    serverPassword: profile.serverPassword ? "***" : null,
  };
}

function ensureSavedProfileKeyOption(select, profile) {
  if (!select || keyForProfile(profile)) return "";
  const value = savedProfileKeyValue(profile);
  if (!value) return "";

  const optionValue = savedProfileKeyOptionValue(profile);
  if (!select.querySelector(`option[value="${CSS.escape(optionValue)}"]`)) {
    const option = document.createElement("option");
    option.value = optionValue;
    option.textContent = `${savedProfileKeyLabel(profile)} (${tr("savedMissing")})`;
    option.dataset.identityFileName = profile.identityFileName || "";
    option.dataset.identityFilePath = profile.identityFilePath || "";
    select.appendChild(option);
  }
  return optionValue;
}

function profileByName(name) {
  return state.profiles.find((profile) => profile.name === name);
}

function uniqueProfileName(base = "new-profile") {
  let name = base;
  let index = 2;
  while (profileByName(name)) {
    name = `${base}-${index}`;
    index += 1;
  }
  return name;
}

function activeProfileFromEditor() {
  const selectedKeyValue = $("profileKeyName").value;
  const key = keyByPath(selectedKeyValue);
  const authMode = $("profileAuthMode").value;
  const existingProfile = profileByName(state.selectedProfileName) || profileByName($("profileName").value.trim());
  const savedOption = selectedKeyValue.startsWith(SAVED_PROFILE_KEY_PREFIX)
    ? $("profileKeyName").selectedOptions[0]
    : null;
  const typedPassword = $("profilePassword").value;
  const typedIdentityPassphrase = $("profileKeyPassphrase").value;
  return {
    name: $("profileName").value.trim() || "new-profile",
    user: $("profileUser").value.trim(),
    host: $("profileHost").value.trim(),
    serverPort: Number($("profileServerPort").value || 22),
    baseLocalPort: Number($("profileLocalPort").value || 2222),
    identityFileName: key?.name || savedOption?.dataset.identityFileName || null,
    identityFilePath: key?.private_path || savedOption?.dataset.identityFilePath || null,
    identityPassphrase: typedIdentityPassphrase || existingProfile?.identityPassphrase || null,
    serverPassword: typedPassword || existingProfile?.serverPassword || null,
    authMode,
    hostAlias: $("profileAlias").value.trim() || $("profileName").value.trim(),
    reverseTunnel: isLinuxServer && $("profileReverseTunnel").value === "true",
    reverseTargetUser: $("profileReverseTargetUser").value.trim(),
    reverseTargetPort: Number($("profileReverseTargetPort").value || 22),
  };
}

function fillEditor(profile) {
  $("profileName").value = profile.name || "";
  $("profileAlias").value = profile.hostAlias || "";
  $("profileUser").value = profile.user || "";
  $("profileHost").value = profile.host || "";
  $("profileServerPort").value = profile.serverPort || 22;
  $("profileLocalPort").value = profile.baseLocalPort || 2222;
  $("profileAuthMode").value = profile.authMode || "key";
  $("profileReverseTunnel").value = isLinuxServer && profile.reverseTunnel ? "true" : "false";
  $("profileReverseTargetUser").value = profile.reverseTargetUser || "";
  $("profileReverseTargetPort").value = profile.reverseTargetPort || 22;
  const key = keyForProfile(profile);
  const savedOptionValue = ensureSavedProfileKeyOption($("profileKeyName"), profile);
  $("profileKeyName").value = key?.private_path || savedOptionValue || "";
  $("profileKeyPassphrase").value = profile.identityPassphrase || "";
  $("profilePassword").value = profile.serverPassword || "";
  updateProfileAuthMode();
  updateProfileTunnelMode();
  updateProfilePreview();
}

function fillConnect(profile) {
  $("serverUser").value = profile.user || "";
  $("serverHost").value = profile.host || "";
  $("serverPort").value = profile.serverPort || 22;
  $("localPort").value = profile.baseLocalPort || 2222;
  $("hostAlias").value = profile.hostAlias || profile.name || "";
  $("authMode").value = profile.authMode || "key";
  $("reverseTunnel").value = isLinuxServer && profile.reverseTunnel ? "true" : "false";
  $("reverseTargetUser").value = profile.reverseTargetUser || "";
  $("reverseTargetPort").value = profile.reverseTargetPort || 22;
  $("serverPassword").value = profile.serverPassword || "";
  $("connectKeyPassphrase").value = profile.identityPassphrase || "";

  const key = keyForProfile(profile);
  $("connectKey").value = key?.private_path || "";
  updateAuthMode();
  updateTunnelMode();
  updateConfig();
}

function updateProfilePreview() {
  $("profileJsonPreview").textContent = JSON.stringify(activeProfileFromEditor(), null, 2);
}

function updateSelectedProfileDraft() {
  const current = activeProfileFromEditor();
  const index = state.profiles.findIndex((profile) => profile.name === state.selectedProfileName);
  if (index >= 0) {
    state.profiles[index] = current;
  } else {
    state.profiles.push(current);
  }
  state.selectedProfileName = current.name;
  renderProfileLists();
}

function updateConnectionsTabBadge() {
  const tab = document.querySelector('[data-page="connectionsPage"]');
  if (!tab) return;
  tab.textContent = state.connectionsUnreadCount > 0
    ? `${tr("connections")} · ${state.connectionsUnreadCount}`
    : tr("connections");
  tab.classList.toggle("has-badge", state.connectionsUnreadCount > 0);
}

function markConnectionArrived(connection) {
  state.highlightedConnectionPid = connection.pid;
  if (state.currentPage !== "connectionsPage") {
    state.connectionsUnreadCount += 1;
  }
  updateConnectionsTabBadge();
  renderConnections();
  clearTimeout(state.highlightTimer);
  state.highlightTimer = setTimeout(() => {
    if (state.highlightedConnectionPid === connection.pid) {
      state.highlightedConnectionPid = null;
      renderConnections();
    }
  }, 1600);
}

function setPage(pageId) {
  state.currentPage = pageId;
  document.querySelectorAll(".page").forEach((page) => page.classList.toggle("active", page.id === pageId));
  document.querySelectorAll(".tab").forEach((tab) => tab.classList.toggle("active", tab.dataset.page === pageId));
  if (pageId === "connectionsPage") {
    state.connectionsUnreadCount = 0;
    updateConnectionsTabBadge();
  }
  if (pageId === "installPage") {
    showToast(tr("keyInstall"), `${tr("keygenTitle")}\n${tr("installTitle")}\n${tr("verifyTitle")}`, "info", 5200);
    state.installGuideShown = true;
  }
}

function runningConnections() {
  return state.connections.filter((connection) => connection.status === "running" || connection.status === "starting");
}

function updateTunnelSummary() {
  const running = runningConnections();
  $("tunnelState").textContent = running.length ? tr("runningCount", running.length) : tr("notStarted");
  $("connectionCount").textContent = tr("connectionCount", running.length, state.connections.length);
}

function renderConnections() {
  const list = $("connectionList");
  list.innerHTML = "";

  if (!state.connections.length) {
    const empty = document.createElement("div");
    empty.className = "connection-card";
    empty.innerHTML = `
      <div class="item-title">${tr("noConnections")}</div>
      <div class="item-meta">${tr("noConnectionsHint")}</div>
    `;
    list.appendChild(empty);
    updateTunnelSummary();
    return;
  }

  for (const connection of state.connections) {
    const item = document.createElement("div");
    item.className = `connection-card ${connection.status}`;
    item.classList.toggle("just-connected", connection.pid === state.highlightedConnectionPid);
    item.innerHTML = `
      <div class="connection-head">
        <div class="item-title">${connection.alias || connection.user}@${connection.host}</div>
        <span class="status-pill">${connection.statusText}</span>
      </div>
      <div class="item-meta">PID ${connection.pid} / ${connection.user}@${connection.host}:${connection.serverPort}</div>
      <div class="item-meta">${connection.reverseTunnel
        ? tr("reverseToLocal", connection.localPort, connection.reverseTargetPort || 22)
        : tr("localToRemote", connection.localPort, connection.serverPort)}</div>
      <div class="item-meta">${connection.command}</div>
      <div class="connection-actions">
        <button data-page-jump="connectPage">${tr("viewConfig")}</button>
        <button data-stop-pid="${connection.pid}" ${connection.status !== "running" && connection.status !== "starting" ? "disabled" : ""}>${tr("stop")}</button>
      </div>
    `;
    list.appendChild(item);
  }

  updateTunnelSummary();
}

function clearProfileDropMarkers() {
  document.querySelectorAll(".profile-item.drop-before, .profile-item.drop-after").forEach((node) => {
    node.classList.remove("drop-before", "drop-after");
  });
}

function beginProfileDrag(event, profileName, item) {
  if (event.button !== 0) return;
  event.preventDefault();
  event.stopPropagation();
  state.profileDrag = {
    name: profileName,
    pointerId: event.pointerId,
    startY: event.clientY,
    targetName: "",
    afterTarget: false,
    active: false,
  };
  state.draggingProfileName = profileName;
  item.classList.add("drag-armed");
  event.currentTarget.setPointerCapture(event.pointerId);
}

function moveProfileDrag(event, item) {
  const drag = state.profileDrag;
  if (!drag || drag.pointerId !== event.pointerId) return;
  event.preventDefault();
  const deltaY = event.clientY - drag.startY;
  if (!drag.active && Math.abs(deltaY) < 4) return;

  drag.active = true;
  state.profileDragMoved = true;
  item.classList.add("dragging");
  item.style.transform = `translateY(${deltaY}px)`;
  item.style.zIndex = "8";
  document.body.classList.add("profile-dragging");

  const target = document.elementFromPoint(event.clientX, event.clientY)?.closest(".profile-item");
  clearProfileDropMarkers();
  if (!target || target.dataset.profileName === drag.name) {
    drag.targetName = "";
    return;
  }

  const after = event.clientY > target.getBoundingClientRect().top + target.offsetHeight / 2;
  target.classList.toggle("drop-before", !after);
  target.classList.toggle("drop-after", after);
  drag.targetName = target.dataset.profileName;
  drag.afterTarget = after;
}

function endProfileDrag(event, item) {
  const drag = state.profileDrag;
  if (!drag || drag.pointerId !== event.pointerId) return;
  event.preventDefault();
  if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
    event.currentTarget.releasePointerCapture(event.pointerId);
  }
  item.classList.remove("drag-armed", "dragging");
  item.style.transform = "";
  item.style.zIndex = "";
  document.body.classList.remove("profile-dragging");
  clearProfileDropMarkers();

  const targetName = drag.targetName;
  const afterTarget = drag.afterTarget;
  const sourceName = drag.name;
  state.profileDrag = null;
  state.draggingProfileName = "";

  if (targetName && sourceName !== targetName) {
    reorderProfiles(sourceName, targetName, afterTarget);
    return;
  }

  setTimeout(() => {
    state.profileDragMoved = false;
  }, 120);
}

function renderProfileLists() {
  const list = $("profileList");
  const connect = $("connectProfile");
  list.innerHTML = "";
  connect.innerHTML = "";

  for (const profile of state.profiles) {
    const authDetail = profile.authMode === "password"
      ? profile.serverPassword ? `password / ${tr("configuredPassword")}` : "password"
      : `key / ${keyForProfile(profile) ? keyOptionLabel(keyForProfile(profile)) : profile.identityFileName || tr("unspecifiedKey")}${profile.identityPassphrase ? ` / ${tr("configuredKeyPassphrase")}` : ""}`;
    const directionDetail = profile.reverseTunnel
      ? tr("reverseToLocal", profile.baseLocalPort || 22022, profile.reverseTargetPort || 22)
      : tr("localToRemote", profile.baseLocalPort || 2222, profile.serverPort || 22);
    const item = document.createElement("div");
    item.className = "profile-item";
    item.classList.toggle("profile-reverse", Boolean(profile.reverseTunnel));
    item.role = "button";
    item.tabIndex = 0;
    item.dataset.profileName = profile.name;
    item.classList.toggle("active", profile.name === state.selectedProfileName);
    item.innerHTML = `
      <div class="profile-item-head">
        <div class="item-title">${profile.name}</div>
        ${profile.reverseTunnel ? `<span class="profile-direction">${tr("reverseTunnel")}</span>` : ""}
        <span class="drag-handle" aria-label="拖动调整顺序">⋮⋮</span>
      </div>
      <div class="item-meta">${profile.user}@${profile.host}:${profile.serverPort}</div>
      <div class="item-meta">${directionDetail}</div>
      <div class="item-meta">${authDetail}</div>
    `;
    item.addEventListener("click", () => {
      if (state.profileDragMoved) {
        state.profileDragMoved = false;
        return;
      }
      state.selectedProfileName = profile.name;
      fillEditor(profile);
      renderProfileLists();
      log("选择配置档案", profile.name);
    });
    item.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      state.selectedProfileName = profile.name;
      fillEditor(profile);
      renderProfileLists();
      log("选择配置档案", profile.name);
    });
    const handle = item.querySelector(".drag-handle");
    handle.addEventListener("pointerdown", (event) => beginProfileDrag(event, profile.name, item));
    handle.addEventListener("pointermove", (event) => moveProfileDrag(event, item));
    handle.addEventListener("pointerup", (event) => endProfileDrag(event, item));
    handle.addEventListener("pointercancel", (event) => endProfileDrag(event, item));
    handle.addEventListener("click", (event) => event.stopPropagation());
    list.appendChild(item);

    const option = document.createElement("option");
    option.value = profile.name;
    option.textContent = `${profile.name} (${profile.user}@${profile.host})`;
    connect.appendChild(option);
  }

  if (state.selectedProfileName) connect.value = state.selectedProfileName;
}

async function persistProfileOrder() {
  await invoke("save_profiles", { profiles: state.profiles });
  renderProfileLists();
  log("配置档案顺序已更新", state.profiles.map((profile) => profile.name).join(" -> "));
  showToast(tr("profileOrderSaved"), tr("firstProfile", state.profiles[0]?.name), "success", 2600);
}

function reorderProfiles(sourceName, targetName, afterTarget) {
  if (!sourceName || !targetName || sourceName === targetName) return;
  const sourceIndex = state.profiles.findIndex((profile) => profile.name === sourceName);
  const targetIndex = state.profiles.findIndex((profile) => profile.name === targetName);
  if (sourceIndex < 0 || targetIndex < 0) return;

  const [moved] = state.profiles.splice(sourceIndex, 1);
  const adjustedTargetIndex = state.profiles.findIndex((profile) => profile.name === targetName);
  state.profiles.splice(adjustedTargetIndex + (afterTarget ? 1 : 0), 0, moved);
  state.profileDragMoved = true;
  state.draggingProfileName = "";
  clearProfileDropMarkers();
  setTimeout(() => {
    state.profileDragMoved = false;
  }, 180);
  persistProfileOrder().catch((error) => log("保存配置顺序失败", String(error)));
}

async function refreshProfiles() {
  log("读取 profiles.json");
  state.profiles = await invoke("load_profiles");
  if (!state.selectedProfileName && state.profiles[0]) state.selectedProfileName = state.profiles[0].name;
  const selected = profileByName(state.selectedProfileName) || state.profiles[0];
  if (selected) {
    state.selectedProfileName = selected.name;
    fillEditor(selected);
    fillConnect(selected);
  }
  renderProfileLists();
}

function renderKeySelects() {
  const profileKey = $("profileKeyName");
  const connectKey = $("connectKey");
  const publicSelect = $("publicKeySelect");
  const silentIdentity = $("silentIdentityKey");
  const verifyIdentity = $("verifyIdentityKey");
  profileKey.innerHTML = `<option value="">${tr("noKey")}</option>`;
  connectKey.innerHTML = `<option value="">${tr("noKey")}</option>`;
  const oldPublicPath = publicSelect.value;
  const oldSilentIdentity = silentIdentity.value;
  const oldVerifyIdentity = verifyIdentity.value;
  publicSelect.innerHTML = `<option value="">${tr("noPublicKey")}</option>`;
  silentIdentity.innerHTML = `<option value="">${tr("chooseInstalledKey")}</option>`;
  verifyIdentity.innerHTML = `<option value="">${tr("chooseVerifyKey")}</option>`;

  for (const key of state.keys) {
    const option1 = document.createElement("option");
    option1.value = key.private_path;
    option1.textContent = keyOptionLabel(key);
    profileKey.appendChild(option1);

    const option2 = document.createElement("option");
    option2.value = key.private_path;
    option2.textContent = keyOptionLabel(key);
    connectKey.appendChild(option2);

    const option4 = document.createElement("option");
    option4.value = key.private_path;
    option4.textContent = keyOptionLabel(key);
    silentIdentity.appendChild(option4);

    const option5 = document.createElement("option");
    option5.value = key.private_path;
    option5.textContent = keyOptionLabel(key);
    verifyIdentity.appendChild(option5);

    if (key.public_path) {
      const option3 = document.createElement("option");
      option3.value = key.public_path;
      option3.textContent = keyOptionLabel(key, ".pub");
      publicSelect.appendChild(option3);
    }
  }

  publicSelect.value = state.keys.some((key) => key.public_path === oldPublicPath) ? oldPublicPath : "";
  silentIdentity.value = state.keys.some((key) => key.private_path === oldSilentIdentity) ? oldSilentIdentity : "";
  verifyIdentity.value = state.keys.some((key) => key.private_path === oldVerifyIdentity) ? oldVerifyIdentity : "";
  updateInstallActionState();
}

function renderKeyList(containerId, keys) {
  const list = $(containerId);
  list.innerHTML = "";

  for (const key of keys) {
    const item = document.createElement("div");
    item.className = "key-item";
    item.dataset.path = key.private_path;
    item.classList.toggle("active", state.selectedKeys.has(key.private_path));
    item.innerHTML = `
      <div class="key-head">
        <input class="key-check" type="checkbox" ${state.selectedKeys.has(key.private_path) ? "checked" : ""} />
        <span class="item-title">${keyOptionLabel(key)}</span>
      </div>
      <div class="item-meta">${key.fingerprint || tr("noFingerprint")}</div>
      <div class="item-meta">${key.private_path}</div>
    `;
    item.addEventListener("click", (event) => {
      if (event.target.matches("input")) return;
      setKeySelection(key, !state.selectedKeys.has(key.private_path), true);
    });
    item.querySelector(".key-check").addEventListener("change", (event) => {
      setKeySelection(key, event.target.checked, true);
    });
    list.appendChild(item);
  }
}

function renderSelectionState() {
  const selected = selectedKeyObjects();
  const names = selected.map((key) => keyOptionLabel(key));
  $("selectedKeyLabel").textContent = selected.length ? tr("selectedCount", selected.length, names.join(", ")) : tr("notSelected");

  const hasUserKey = selected.some(isUserKey);
  const hasPortableKey = selected.some((key) => !isUserKey(key));
  $("exportSelectedKey").classList.toggle("hidden", !hasUserKey);
  $("importSelectedKey").classList.toggle("hidden", !hasPortableKey);

  const allSelected = state.keys.length > 0 && state.keys.every((key) => state.selectedKeys.has(key.private_path));
  $("selectAllKeys").checked = allSelected;
  $("selectAllKeys").indeterminate = false;

  updateScopedSelectAll("selectAllUserKeys", state.keys.filter(isUserKey));
  updateScopedSelectAll("selectAllPortableKeys", state.keys.filter((key) => !isUserKey(key)));
}

function updateScopedSelectAll(id, keys) {
  const checkbox = $(id);
  const selectedCount = keys.filter((key) => state.selectedKeys.has(key.private_path)).length;
  checkbox.checked = keys.length > 0 && selectedCount === keys.length;
  checkbox.indeterminate = false;
}

function renderKeyLists() {
  renderKeyList("userKeyList", state.keys.filter(isUserKey));
  renderKeyList("portableKeyList", state.keys.filter((key) => !isUserKey(key)));
  renderSelectionState();
}

function setKeySelection(key, checked, shouldLog = false) {
  if (checked) {
    state.selectedKeys.add(key.private_path);
    state.selectedKey = key;
  } else {
    state.selectedKeys.delete(key.private_path);
    if (state.selectedKey?.private_path === key.private_path) {
      state.selectedKey = selectedKeyObjects()[0] || null;
    }
  }

  if (state.selectedKey) {
    $("connectKey").value = state.selectedKey.private_path;
  }

  if (shouldLog) log(checked ? "选择密钥" : "取消选择密钥", key.private_path);
  renderKeyLists();
  updateConfig();
}

function selectKey(key) {
  state.selectedKey = key;
  state.selectedKeys.add(key.private_path);
  $("connectKey").value = key.private_path;
  renderKeyLists();
  updateConfig();
}

function setKeyGroupSelection(keys, checked, label) {
  if (checked) {
    for (const key of keys) state.selectedKeys.add(key.private_path);
    state.selectedKey = keys[0] || state.selectedKey;
  } else {
    for (const key of keys) state.selectedKeys.delete(key.private_path);
    if (state.selectedKey && keys.some((key) => key.private_path === state.selectedKey.private_path)) {
      state.selectedKey = selectedKeyObjects()[0] || null;
    }
  }

  if (state.selectedKey) {
    $("connectKey").value = state.selectedKey.private_path;
  }

  log(checked ? `全选${label}` : `取消全选${label}`, `${keys.length} 个`);
  renderKeyLists();
  updateConfig();
}

async function refreshKeys() {
  log("扫描密钥", "scan_key_bundles");
  state.keys = await invoke("scan_key_bundles");
  const userKeys = state.keys.filter(isUserKey);
  const portableKeys = state.keys.filter((key) => !isUserKey(key));
  $("userKeyCount").textContent = lang() === "en" ? `${userKeys.length}` : `${userKeys.length} 个`;
  $("portableKeyCount").textContent = lang() === "en" ? `${portableKeys.length}` : `${portableKeys.length} 个`;
  renderKeySelects();
  state.selectedKeys = new Set([...state.selectedKeys].filter((path) => state.keys.some((key) => key.private_path === path)));
  state.selectedKey = state.selectedKey && keyByPath(state.selectedKey.private_path) ? keyByPath(state.selectedKey.private_path) : selectedKeyObjects()[0] || null;
  if (state.selectedKey) {
    $("connectKey").value = state.selectedKey.private_path;
  }
  renderKeyLists();
}

async function refreshVmIps() {
  log("检测虚拟机 IP", "detect_vm_ips");
  const ips = await invoke("detect_vm_ips");
  const select = $("vmIp");
  const oldValue = select.value;
  select.innerHTML = "";

  for (const item of ips) {
    const option = document.createElement("option");
    option.value = item.ip;
    option.textContent = `${item.ip} (${item.source})`;
    select.appendChild(option);
  }

  if (!ips.length) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = tr("noVmIp");
    select.appendChild(option);
  } else if ($("autoVmIp").value === "off" && oldValue) {
    select.value = oldValue;
  }
}

async function refreshPort(showNotice = false) {
  if (isLinuxServer && $("reverseTunnel").value === "true") {
    $("portStatus").textContent = tr("waitingReversePort");
    return;
  }
  const start = Number($("localPort").value || 2222);
  log("检测本地端口", `find_free_port ${start}`);
  const port = await invoke("find_free_port", { startPort: start });
  $("localPort").value = port;
  $("portStatus").textContent = port === start ? tr("portAvailable") : tr("usingPort", port);
  if (showNotice) showToast(port === start ? tr("portAvailable") : tr("usingPort", port), `${tr("localPort")} ${port}`, "success");
}

function updateAuthMode() {
  const passwordMode = $("authMode").value === "password";
  $("connectKey").disabled = passwordMode;
  $("connectKey").closest("label").classList.toggle("hidden", passwordMode);
  $("connectKeyPassphraseWrap").classList.toggle("hidden", passwordMode);
  $("connectPasswordWrap").classList.toggle("hidden", !passwordMode);
  if (passwordMode) {
    log("密码模式说明", "密码模式仅作为测试/备用功能；默认建议使用密钥认证。");
  }
}

function updateProfileAuthMode() {
  $("profileKeyName").closest("label").classList.remove("hidden");
  $("profileKeyPassphraseWrap").classList.toggle("hidden", $("profileAuthMode").value === "password");
  $("profilePasswordWrap").classList.remove("hidden");
}

function updateProfileTunnelMode() {
  const reverse = isLinuxServer && $("profileReverseTunnel").value === "true";
  $("profileReverseTargetUserWrap").classList.toggle("hidden", !reverse);
  $("profileReverseTargetPortWrap").classList.toggle("hidden", !reverse);
  setLabelText("profileLocalPort", reverse ? "remotePortOnA" : "localStartPort");
}

function updateTunnelMode() {
  const reverse = isLinuxServer && $("reverseTunnel").value === "true";
  $("reverseTargetUserWrap").classList.toggle("hidden", !reverse);
  $("reverseTargetPortWrap").classList.toggle("hidden", !reverse);
  $("vmIpWrap").classList.toggle("hidden", reverse);
  $("autoVmIpWrap").classList.toggle("hidden", reverse);
  $("firewallRule").classList.toggle("hidden", reverse);
  $("refreshPort").disabled = reverse;
  $("writeConfig").disabled = reverse;
  setLabelText("localPort", reverse ? "remotePortOnA" : "localPort");
  if (reverse) {
    $("portStatus").textContent = tr("waitingReversePort");
  }
}

async function updateConfig() {
  if (isLinuxServer && $("reverseTunnel").value === "true") {
    const config = await invoke("generate_reverse_proxy_config", {
      alias: $("hostAlias").value,
      jumpHost: $("serverHost").value,
      jumpPort: Number($("serverPort").value),
      jumpUser: $("serverUser").value,
      remotePort: Number($("localPort").value),
      targetUser: $("reverseTargetUser").value,
    });
    $("sshConfig").textContent = config;
    return;
  }
  const config = await invoke("generate_vscode_config", {
    alias: $("hostAlias").value,
    vmIp: $("vmIp").value,
    localPort: Number($("localPort").value),
    user: $("serverUser").value,
    identityFile: selectedIdentityFile(),
  });
  $("sshConfig").textContent = config;
}

async function testKeyBeforeTunnel(request) {
  showToast(tr("testingKeyLogin"), `${request.user}@${request.host}:${request.server_port}`, "info", 4200);
  log("启动前 SSH 测试", `${request.user}@${request.host}:${request.server_port}\n密钥: ${request.identity_file || "未指定"}`);
  const result = await invoke("test_ssh_server", {
    user: request.user,
    host: request.host,
    port: request.server_port,
    identityFile: request.identity_file,
    identityPassphrase: request.identity_passphrase,
  });
  log(result.success ? "启动前 SSH 测试通过" : "启动前 SSH 测试失败", result.message);
  if (!result.success) {
    const reason = result.message.split("\n")[0];
    $("tunnelState").textContent = reason;
    showToast(tr("startStopped"), reason, "error", 7200);
    return false;
  }
  showToast(tr("keyLoginReady"), tr("startTunnelNow"), "success", 2600);
  return true;
}

async function testPasswordBeforeTunnel(request) {
  if (!request.password) {
    $("tunnelState").textContent = tr("missingServerPassword");
    showToast(tr("startStopped"), tr("passwordRequired"), "error", 5200);
    log("启动前密码认证失败", "未填写服务器密码。");
    return false;
  }
  showToast(tr("testingPasswordAuth"), `${request.user}@${request.host}:${request.server_port}`, "info", 4200);
  log("启动前密码认证测试", `${request.user}@${request.host}:${request.server_port}`);
  const result = await invoke("test_password_ssh_server", {
    user: request.user,
    host: request.host,
    port: request.server_port,
    password: request.password || "",
  });
  log(result.success ? "启动前密码认证通过" : "启动前密码认证失败", result.message);
  if (!result.success) {
    $("tunnelState").textContent = tr("passwordAuthFailed");
    showToast(tr("startStopped"), result.message.split("\n")[0], "error", 7200);
    return false;
  }
  showToast(tr("passwordAuthReady"), tr("startTunnelNow"), "success", 2600);
  return true;
}

async function refreshAll() {
  try {
    await refreshKeys();
    await refreshProfiles();
    await refreshVmIps();
    await refreshPort();
    await updateConfig();
    renderConnections();
    log("刷新完成");
  } catch (error) {
    log("刷新失败", String(error));
    throw error;
  }
}

async function refreshAllFromButton() {
  const button = $("refreshAll");
  const oldText = button.textContent;
  const oldWidth = button.style.width;
  button.style.width = `${Math.ceil(button.getBoundingClientRect().width)}px`;
  button.disabled = true;
  button.classList.add("loading", "is-busy");
  button.textContent = tr("refreshing");
  showToast(tr("refreshRunning"), tr("refreshRunningDetail"), "info", 4200);
  try {
    await refreshAll();
    showToast(tr("refreshComplete"), tr("refreshCompleteDetail"), "success", 3200);
  } catch (error) {
    showToast(tr("refreshFailed"), String(error), "error", 5600);
    throw error;
  } finally {
    button.disabled = false;
    button.classList.remove("loading", "is-busy");
    button.textContent = oldText;
    button.style.width = oldWidth;
  }
}

function startStatusTimer() {
  clearInterval(state.statusTimer);
  state.statusTimer = setInterval(async () => {
    const active = runningConnections();
    if (!active.length) {
      clearInterval(state.statusTimer);
      state.statusTimer = null;
      updateTunnelSummary();
      return;
    }

    for (const connection of active) {
      try {
        const status = await invoke("check_tunnel_status", { pid: connection.pid });
        if (status.running) {
          if (connection.status === "starting") {
            if (connection.reverseTunnel && !status.connected) {
              connection.statusText = tr("waitingReversePort");
              continue;
            }
            if (connection.authMode === "password" && !connection.reverseTunnel) {
              const portOpen = await invoke("is_local_port_open", { port: connection.localPort });
              if (!portOpen) {
                connection.statusText = tr("waitingPassword");
                continue;
              }
            }
            connection.status = "running";
            connection.statusText = connection.reverseTunnel
              ? tr("reverseRunning")
              : connection.authMode === "password" ? tr("passwordTunnelRunning") : tr("running");
            showToast(
              connection.reverseTunnel
                ? tr("reverseTunnelConnected")
                : connection.authMode === "password" ? tr("passwordTunnelConnected") : tr("tunnelConnected"),
              `${connection.alias || connection.host} / PID ${connection.pid}`,
              "success",
            );
            markConnectionArrived(connection);
          }
        } else {
          connection.status = "exited";
          const released = connection.reverseTunnel
            ? true
            : await invoke("is_local_port_available", { port: connection.localPort });
          connection.statusText = connection.reverseTunnel
            ? status.exit_code == null ? tr("tunnelExited") : `${tr("tunnelExited")} ${status.exit_code}`
            : `${status.exit_code == null ? tr("tunnelExited") : `${tr("tunnelExited")} ${status.exit_code}`}，${released ? tr("portReleased") : tr("portStillBusy")}`;
          log("隧道进程已退出", `PID ${connection.pid} / exit=${status.exit_code ?? "unknown"}`);
          showToast(tr("tunnelExited"), `${connection.alias || connection.host} / exit=${status.exit_code ?? "unknown"}`, "error", 5200);
        }
      } catch (error) {
        connection.status = "failed";
        connection.statusText = "状态检查失败";
        log("状态检查失败", `PID ${connection.pid}: ${String(error)}`);
      }
    }

    renderConnections();
  }, 3000);
}

async function startTunnel() {
  if ($("authMode").value === "password") {
    const password = $("serverPassword").value;
    const request = {
      user: $("serverUser").value,
      host: $("serverHost").value,
      server_port: Number($("serverPort").value),
      local_port: Number($("localPort").value),
      password,
      reconnect: $("reconnect").checked,
      reconnect_delay: Number($("reconnectDelay").value || 5),
      reverse_tunnel: isLinuxServer && $("reverseTunnel").value === "true",
      reverse_target_port: Number($("reverseTargetPort").value || 22),
    };
  $("tunnelState").textContent = lang() === "en" ? "Testing password" : "测试密码";
    if (!(await testPasswordBeforeTunnel(request))) return;
    showToast(password ? tr("startingPasswordTunnel") : tr("preparingPasswordTunnel"), password ? tr("askpassTunnelDetail") : tr("missingPasswordTunnelDetail"), "info", 5200);
    log("启动密码模式隧道", JSON.stringify({ ...request, password: password ? "***" : "" }, null, 2));
    const result = await invoke("start_password_tunnel", { request });
    const connection = {
      pid: result.pid,
      alias: $("hostAlias").value,
      user: request.user,
      host: request.host,
      serverPort: request.server_port,
      localPort: request.local_port,
      identityFile: null,
      authMode: "password",
      reverseTunnel: request.reverse_tunnel,
      reverseTargetPort: request.reverse_target_port,
      reverseTargetUser: $("reverseTargetUser").value.trim(),
      command: result.command,
      status: "starting",
      statusText: request.reverse_tunnel ? tr("waitingReversePort") : tr("passwordTunnelStarting"),
      startedAt: new Date().toISOString(),
    };
    state.connections.unshift(connection);
    $("lastCommand").textContent = result.command;
    log("密码模式隧道已后台启动", result.command);
    renderConnections();
    startStatusTimer();
    return;
  }

  const request = {
    user: $("serverUser").value,
    host: $("serverHost").value,
    server_port: Number($("serverPort").value),
    local_port: Number($("localPort").value),
    identity_file: selectedIdentityFile(),
    identity_passphrase: selectedIdentityPassphrase(),
    reconnect: $("reconnect").checked,
    reconnect_delay: Number($("reconnectDelay").value || 5),
    reverse_tunnel: isLinuxServer && $("reverseTunnel").value === "true",
    reverse_target_port: Number($("reverseTargetPort").value || 22),
  };
  $("tunnelState").textContent = lang() === "en" ? "Testing SSH" : "测试 SSH";
  if (!(await testKeyBeforeTunnel(request))) return;

  showToast(tr("startingTunnel"), `${request.user}@${request.host}:${request.server_port}`, "info");
  log("启动隧道", JSON.stringify({ ...request, identity_passphrase: request.identity_passphrase ? "***" : null }, null, 2));
  const result = await invoke("start_tunnel", { request });
  const connection = {
    pid: result.pid,
    alias: $("hostAlias").value,
    user: request.user,
    host: request.host,
    serverPort: request.server_port,
    localPort: request.local_port,
    identityFile: request.identity_file,
    authMode: "key",
    reverseTunnel: request.reverse_tunnel,
    reverseTargetPort: request.reverse_target_port,
    reverseTargetUser: $("reverseTargetUser").value.trim(),
    command: result.command,
    status: "starting",
    statusText: request.reverse_tunnel ? tr("waitingReversePort") : tr("starting"),
    startedAt: new Date().toISOString(),
  };
  state.connections.unshift(connection);
  $("lastCommand").textContent = result.command;
  log("隧道进程已启动", result.command);
  showToast(tr("tunnelStarted"), tr("confirmingTunnel", result.pid), "success");
  renderConnections();
  startStatusTimer();
}

async function waitForLocalPortReleased(port, attempts = 12, delayMs = 350) {
  for (let index = 0; index < attempts; index += 1) {
    const available = await invoke("is_local_port_available", { port });
    if (available) return true;
    await new Promise((resolve) => setTimeout(resolve, delayMs));
  }
  return false;
}

async function stopTunnel(pid) {
  const connection = state.connections.find((item) => item.pid === pid);
  if (!connection) return log("当前没有已记录的隧道进程", `PID ${pid}`);
  log("停止隧道", `taskkill /PID ${pid} /T /F\n本地端口: ${connection.localPort}`);
  await invoke("stop_tunnel", { pid });
  const released = connection.reverseTunnel ? true : await waitForLocalPortReleased(connection.localPort);
  connection.status = "exited";
  connection.statusText = released ? tr("tunnelStoppedReleased") : tr("tunnelStoppedBusy");
  renderConnections();
  if (!connection.reverseTunnel && Number($("localPort").value) === Number(connection.localPort)) {
    $("portStatus").textContent = released ? tr("portReleased") : tr("portStillBusy");
  }
  await updateConfig();
  showToast(
    released ? tr("tunnelStoppedReleased") : tr("tunnelStoppedBusy"),
    `${connection.alias || connection.host} / ${tr(connection.reverseTunnel ? "remotePortDetail" : "localPortDetail", connection.localPort)}`,
    released ? "success" : "error",
    released ? 3600 : 6200,
  );
  log(released ? "隧道已停止，端口已释放" : "隧道已停止，端口仍被占用", `PID ${pid} / ${connection.reverseTunnel ? "remote" : "local"}:${connection.localPort}`);
}

function connectionName(connection) {
  return `${connection.alias || connection.user || "ssh"}@${connection.host || "unknown"}`;
}

function showShutdownModal(count) {
  $("shutdownTitle").textContent = tr("closingConnections");
  $("shutdownMessage").textContent = tr("closingConnectionsDetail", count);
  $("shutdownModal").classList.remove("hidden");
}

function updateShutdownModal(index, total, connection) {
  $("shutdownMessage").textContent = tr("closingConnectionProgress", index, total, connectionName(connection));
}

async function stopConnectionBeforeExit(connection) {
  log("关闭程序前停止连接", `PID ${connection.pid} / 本地端口: ${connection.localPort}`);
  try {
    await invoke("stop_tunnel", { pid: connection.pid });
  } catch (error) {
    log("停止连接命令返回异常", `PID ${connection.pid}: ${String(error)}`);
  }

  const released = connection.reverseTunnel ? true : await waitForLocalPortReleased(connection.localPort);
  connection.status = released ? "exited" : "failed";
  connection.statusText = released ? "已停止，端口已释放" : "已停止，端口仍被占用";
  renderConnections();
  return released;
}

async function closeAppSafely() {
  if (state.isClosingApp) return;
  state.isClosingApp = true;
  const active = runningConnections();

  if (!active.length) {
    invoke("window_close").catch((error) => {
      state.isClosingApp = false;
      log("窗口关闭失败", String(error));
      showToast(tr("closeAppFailed"), String(error), "error", 5200);
    });
    return;
  }

  showShutdownModal(active.length);
  showToast(tr("closingConnections"), tr("closingConnectionsDetail", active.length), "info", 5200);

  try {
    for (const [index, connection] of active.entries()) {
      updateShutdownModal(index + 1, active.length, connection);
      await stopConnectionBeforeExit(connection);
    }
    clearInterval(state.statusTimer);
    state.statusTimer = null;
    $("shutdownMessage").textContent = tr("closedBeforeExit");
    log("关闭程序前连接清理完成", `${active.length} 个连接`);
    await new Promise((resolve) => setTimeout(resolve, 450));
    await invoke("window_close");
  } catch (error) {
    state.isClosingApp = false;
    $("shutdownModal").classList.add("hidden");
    log("关闭程序失败", String(error));
    showToast(tr("closeAppFailed"), String(error), "error", 6200);
  }
}

async function saveProfiles(showNotice = true) {
  const current = activeProfileFromEditor();
  const currentIndex = state.profiles.findIndex((profile) => profile.name === state.selectedProfileName);
  const sameNameIndex = state.profiles.findIndex((profile) => profile.name === current.name);
  const targetIndex = currentIndex >= 0 ? currentIndex : sameNameIndex;
  const next = state.profiles.filter((profile, index) => index === targetIndex || (profile.name !== state.selectedProfileName && profile.name !== current.name));
  if (targetIndex >= 0) {
    next[targetIndex] = current;
  } else {
    next.push(current);
  }
  await invoke("save_profiles", { profiles: next });
  state.profiles = next;
  state.selectedProfileName = current.name;
  renderProfileLists();
  log("保存 profiles.json", JSON.stringify(redactedProfile(current), null, 2));
  if (showNotice) showToast(tr("configSaved"), current.name, "success", 2600);
  return current;
}

async function exportSelectedKey() {
  const keys = selectedKeyObjects().filter(isUserKey);
  if (!keys.length) {
    log("没有需要导出的用户密钥");
    showToast(tr("noUserKeysToExport"), tr("selectUserKeysToExport"), "error", 4200);
    return;
  }
  showToast(tr("exportingKeys"), tr("exportKeysDetail", keys.length), "info", 3200);
  log("导出密钥到程序目录", keys.map((key) => key.private_path).join("\n"));
  const exported = await invoke("export_key_files", { privatePaths: keys.map((key) => key.private_path) });
  log("导出完成", `${exported.length} 个密钥`);
  showToast(tr("exportDone"), tr("exportDoneDetail", exported.length), "success", 3600);
  await refreshKeys();
}

async function importSelectedKey() {
  const keys = selectedKeyObjects().filter((key) => !isUserKey(key));
  if (!keys.length) {
    log("没有需要导入的程序目录密钥");
    showToast(tr("noPortableKeysToImport"), tr("selectPortableKeysToImport"), "error", 4200);
    return;
  }
  if (!$("allowImport").checked) {
    log("导入被关闭", "请先打开“允许导入到 ~/.ssh”开关。");
    showToast(tr("importBlocked"), tr("importBlockedDetail"), "error", 5200);
    return;
  }
  showToast(tr("importingKeys"), tr("importKeysDetail", keys.length), "info", 3200);
  log("导入密钥到 ~/.ssh", keys.map((key) => key.private_path).join("\n"));
  const imported = await invoke("import_key_files", { privatePaths: keys.map((key) => key.private_path) });
  log("导入完成", `${imported.length} 个密钥`);
  showToast(tr("importDone"), tr("importDoneDetail", imported.length), "success", 3600);
  await refreshKeys();
}

async function exportPublicBundle() {
  const publicPaths = selectedKeyObjects().map((key) => key.public_path).filter(Boolean);
  if (!publicPaths.length) {
    log("没有可导出的公钥");
    showToast(tr("noPublicKeys"), tr("selectPublicKeys"), "error", 4600);
    return;
  }
  showToast(tr("exportingPublicBundle"), tr("publicBundleDetail", publicPaths.length), "info", 3600);
  log("导出公钥 bundle", `${publicPaths.length} 个公钥`);
  const path = await invoke("export_public_bundle", { publicPaths });
  log("公钥 bundle 已生成", path);
  showToast(tr("publicBundleDone"), path, "success", 5200);
}

async function deleteSelectedKeys() {
  const keys = selectedKeyObjects();
  if (!keys.length) {
    log("没有可删除的密钥");
    showToast(tr("noKeysToDelete"), tr("selectKeysToDelete"), "error", 4200);
    return;
  }

  const names = keys.map((key) => keyOptionLabel(key)).join("、");
  const confirmed = await confirmAction(
    tr("deleteSelectedKeys"),
    tr("deleteKeysMessage", keys.length, names),
    tr("delete"),
  );
  if (!confirmed) return;

  showToast(tr("deletingKeys"), tr("deletingKeysDetail", keys.length), "info", 3200);
  log("删除选定密钥", keys.map((key) => key.private_path).join("\n"));
  const deleted = await invoke("delete_key_files", { privatePaths: keys.map((key) => key.private_path) });
  state.selectedKeys.clear();
  state.selectedKey = null;
  log("密钥删除完成", deleted.join("\n"));
  showToast(tr("keysDeleted"), tr("keysDeletedDetail", keys.length), "success", 3600);
  await refreshKeys();
}

async function installPublicKey() {
  const publicKeyPath = selectedPublicKeyFile();
  if (!publicKeyPath) return log("请选择公钥文件");
  const identityFile = selectedSilentIdentityFile();
  if (!identityFile) return log("静默安装缺少登录密钥", "请选择一把已经安装在服务器上的私钥。");
  setInstallStatus("执行中");
  $("installPublicKey").disabled = true;
  const target = installTarget();
  log("静默写入 authorized_keys", `${target}:${$("installPort").value} <= ${publicKeyPath}\n使用已有密钥: ${identityFile}`);
  try {
    const result = await invoke("install_public_to_linux", {
      target,
      port: Number($("installPort").value),
      publicKeyPath,
      identityFile,
    });
    setInstallStatus(result.success ? "成功" : "失败");
    log(result.success ? "公钥安装成功" : "公钥安装失败", result.message);
    showToast(result.success ? tr("silentInstallDone") : tr("silentInstallFailed"), result.message || target, result.success ? "success" : "error", 5600);
  } finally {
    updateInstallActionState();
  }
}

async function installPublicKeyInTerminal() {
  const publicKeyPath = selectedPublicKeyFile();
  if (!publicKeyPath) return log("请选择公钥文件");
  const password = $("installPassword").value;
  if (password) {
    setInstallStatus("执行中");
    log("密码模式安装 authorized_keys", `${installTarget()}:${$("installPort").value} <= ${publicKeyPath}`);
    const result = await invoke("install_public_to_linux_password", {
      target: installTarget(),
      port: Number($("installPort").value),
      publicKeyPath,
      password,
    });
    setInstallStatus(result.success ? "成功" : "失败");
    log(result.success ? "公钥安装成功" : "公钥安装失败", result.message);
    showToast(result.success ? tr("publicInstallDone") : tr("installFailed"), result.message.split("\n")[0], result.success ? "success" : "error", 5600);
    return;
  }
  log("打开终端安装", `${installTarget()}:${$("installPort").value} <= ${publicKeyPath}`);
  const scriptPath = await invoke("launch_public_install_terminal", {
    target: installTarget(),
    port: Number($("installPort").value),
    publicKeyPath,
    identityFile: null,
  });
  setInstallStatus("已打开终端");
  log("交互式安装脚本", scriptPath);
  showToast(tr("terminalInstallOpened"), installTarget(), "info", 6200);
}

async function generateInstallKey() {
  const name = generatedKeyName();
  const comment = keygenTarget();
  const existing = keyByInstallName(name);
  if (existing?.public_path) {
    syncInstallFields(false);
    $("publicKeySelect").value = existing.public_path;
    $("verifyIdentityKey").value = existing.private_path;
    updateInstallActionState();
    setInstallStatus("密钥已存在");
    log("安装密钥已存在，已选中", existing.private_path);
    showToast(tr("keyExistsSelected"), existing.name, "success");
    return;
  }

  setInstallStatus("生成中");
  log("生成安装密钥", name);
  const passphrase = $("keygenPassphrase").value;
  const key = await invoke("generate_ssh_key", { name, comment, passphrase, overwrite: false });
  await refreshKeys();
  syncInstallFields(false);
  $("publicKeySelect").value = key.public_path || "";
  $("verifyIdentityKey").value = key.private_path || "";
  updateInstallActionState();
  state.selectedKey = keyByPath(key.private_path) || state.selectedKey;
  setInstallStatus("已生成");
  log("安装密钥已生成", key.private_path);
  showToast(tr("keyGenerated"), key.name, "success");
}

async function verifyInstalledKey() {
  const publicKeyPath = selectedPublicKeyFile();
  const identityFile = selectedVerifyIdentityFile();
  if (!identityFile) return log("无法验证密钥登录", "请选择验证使用密钥。");
  setInstallStatus("验证中");
  log("验证密钥登录", `验证私钥: ${identityFile}\n参考公钥: ${publicKeyPath || "未选择"}\n目标: ${installTarget()}:${$("installPort").value}`);
  const result = await invoke("test_ssh_server", {
    user: $("installUser").value,
    host: $("installHost").value,
    port: Number($("installPort").value),
    identityFile,
    identityPassphrase: selectedVerifyIdentityPassphrase(),
  });
  setInstallStatus(result.success ? "验证通过" : "验证失败");
  log(result.success ? "密钥登录验证通过" : "密钥登录验证失败", result.message);
  const reason = result.message.split("\n")[0];
  showToast(result.success ? tr("keyLoginReady") : tr("keyVerifyFailed"), reason, result.success ? "success" : "error", 5600);
}

async function writeConfig() {
  showToast(tr("writingSshConfig"), tr("writeSshConfigDetail"), "info", 3600);
  log("写入 ~/.ssh/config", $("sshConfig").textContent);
  const path = await invoke("write_vscode_config", {
    alias: $("hostAlias").value,
    vmIp: $("vmIp").value,
    localPort: Number($("localPort").value),
    user: $("serverUser").value,
    identityFile: selectedIdentityFile(),
  });
  log("~/.ssh/config 已写入", path);
  showToast(tr("sshConfigWritten"), path, "success", 5200);
}

async function pingHost() {
  log("Ping 服务器", $("serverHost").value);
  const result = await invoke("ping_host", { host: $("serverHost").value, timeoutSec: 2 });
  log(result.success ? "Ping 成功" : "Ping 失败或超时", result.message.trim());
  showToast(result.success ? tr("pingOk") : tr("pingFailed"), result.message.trim().split("\n")[0], result.success ? "success" : "error", result.success ? 3200 : 5200);
}

async function sshTest() {
  log("后台 SSH 测试服务器", `${$("serverUser").value}@${$("serverHost").value}:${$("serverPort").value}`);
  showToast(tr("testingSsh"), `${$("serverUser").value}@${$("serverHost").value}:${$("serverPort").value}`, "info");
  const result = await invoke("test_ssh_server", {
    user: $("serverUser").value,
    host: $("serverHost").value,
    port: Number($("serverPort").value),
    identityFile: selectedIdentityFile(),
    identityPassphrase: selectedIdentityPassphrase(),
  });
  log(result.success ? "SSH 服务可达" : "SSH 检测失败", result.message);
  showToast(result.success ? tr("sshReachable") : tr("sshCheckFailed"), result.message.split("\n")[0], result.success ? "success" : "error", result.success ? 4200 : 6200);
}

async function ensureFirewall() {
  log("检查管理员模式", "is_admin");
  const admin = await invoke("is_admin");
  if (!admin) {
    log("当前不是管理员模式", "将请求 UAC 提权来放行 SSH-Tunnel-Range。");
    showToast(tr("requestingAdmin"), tr("uacDetail"), "info", 5200);
    const result = await invoke("request_firewall_admin");
    log(result.success ? "防火墙提权命令已完成" : "防火墙提权命令失败", result.message);
    showToast(result.success ? tr("firewallRequested") : tr("firewallFailed"), result.message, result.success ? "success" : "error", 5600);
    return;
  }

  log("放行防火墙", "New/Set-NetFirewallRule SSH-Tunnel-Range 2222-2299");
  const result = await invoke("ensure_firewall_rule");
  log(result.success ? "防火墙规则已确认" : "防火墙规则设置失败", result.message);
  showToast(result.success ? tr("firewallAllowed") : tr("firewallFailed"), result.message, result.success ? "success" : "error", 5600);
}

for (const tab of document.querySelectorAll(".tab")) {
  tab.addEventListener("click", () => setPage(tab.dataset.page));
}

for (const id of ["profileName", "profileAlias", "profileUser", "profileHost", "profileServerPort", "profileLocalPort", "profileReverseTunnel", "profileReverseTargetUser", "profileReverseTargetPort", "profileAuthMode", "profileKeyName", "profileKeyPassphrase", "profilePassword"]) {
  $(id).addEventListener("input", () => {
    updateProfilePreview();
    updateSelectedProfileDraft();
  });
  $(id).addEventListener("change", () => {
    updateProfileAuthMode();
    updateProfileTunnelMode();
    updateProfilePreview();
    updateSelectedProfileDraft();
  });
}

for (const id of ["serverUser", "serverHost", "serverPort", "localPort", "reverseTunnel", "reverseTargetUser", "reverseTargetPort", "vmIp", "hostAlias", "connectKey", "connectKeyPassphrase", "authMode", "serverPassword"]) {
  $(id).addEventListener("input", () => {
    updateAuthMode();
    updateTunnelMode();
    updateConfig();
  });
  $(id).addEventListener("change", () => {
    updateAuthMode();
    updateTunnelMode();
    updateConfig();
  });
}

for (const id of ["keygenUser", "keygenHost", "keygenOffset", "keygenPassphrase"]) {
  $(id).addEventListener("input", () => {
    if (id === "keygenUser" || id === "keygenOffset") updateGeneratedKeyName();
    setInstallStatus("待生成");
    if (id !== "keygenPassphrase") saveSettingsQuietly();
  });
  $(id).addEventListener("change", () => {
    if (id === "keygenUser" || id === "keygenOffset") updateGeneratedKeyName();
    setInstallStatus("待生成");
    if (id !== "keygenPassphrase") saveSettingsQuietly();
  });
}

for (const id of ["installUser", "installHost", "installPort", "installPassword", "installKeyName", "publicKeySelect", "silentIdentityKey", "verifyIdentityKey", "verifyIdentityPassphrase"]) {
  $(id).addEventListener("input", () => {
    setInstallStatus("待安装");
    updateInstallActionState();
    if (["installUser", "installHost", "installPort", "installKeyName"].includes(id)) saveSettingsQuietly();
  });
  $(id).addEventListener("change", () => {
    setInstallStatus("待安装");
    updateInstallActionState();
    if (["installUser", "installHost", "installPort", "installKeyName"].includes(id)) saveSettingsQuietly();
  });
}

$("refreshAll").addEventListener("click", () => refreshAllFromButton().catch((error) => log("刷新失败", String(error))));
$("titlebarDrag").addEventListener("mousedown", (event) => {
  if (event.button !== 0 || event.target.closest(".window-control")) return;
  invoke("window_start_drag").catch((error) => log("窗口拖动失败", String(error)));
});
$("titlebarDrag").addEventListener("dblclick", (event) => {
  if (event.target.closest(".window-control")) return;
  invoke("window_toggle_maximize").catch((error) => log("窗口最大化失败", String(error)));
});
$("windowMinimize").addEventListener("click", () => {
  invoke("window_minimize").catch((error) => log("窗口最小化失败", String(error)));
});
$("windowMaximize").addEventListener("click", () => {
  invoke("window_toggle_maximize").catch((error) => log("窗口最大化失败", String(error)));
});
$("windowClose").addEventListener("click", () => {
  closeAppSafely().catch((error) => {
    state.isClosingApp = false;
    $("shutdownModal").classList.add("hidden");
    log("窗口关闭失败", String(error));
    showToast(tr("closeAppFailed"), String(error), "error", 6200);
  });
});
$("settingsButton").addEventListener("click", () => {
  $("settingsModal").classList.remove("hidden");
});
async function showHelp() {
  const content = $("helpContent");
  $("helpModal").classList.remove("hidden");
  content.textContent = "";
  try {
    const response = await fetch(`./help/linux-service.${lang() === "en" ? "en" : "zh"}.md`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const markdown = await response.text();
    let code = null;
    for (const line of markdown.split(/\r?\n/)) {
      if (line.startsWith("```")) {
        if (code) {
          content.appendChild(code);
          code = null;
        } else {
          code = document.createElement("pre");
        }
      } else if (code) {
        code.textContent += `${line}\n`;
      } else if (line.trim()) {
        const heading = /^#{1,2}\s+(.+)$/.exec(line);
        const element = document.createElement(heading ? "h2" : "p");
        element.textContent = heading ? heading[1] : line;
        content.appendChild(element);
      }
    }
    if (code) content.appendChild(code);
  } catch (error) {
    content.textContent = `${tr("helpLoadFailed")} ${error.message}`;
  }
}
$("helpButton").addEventListener("click", showHelp);
$("closeHelp").addEventListener("click", () => $("helpModal").classList.add("hidden"));
$("helpModal").addEventListener("click", (event) => {
  if (event.target === $("helpModal")) $("helpModal").classList.add("hidden");
});
$("closeSettings").addEventListener("click", () => {
  $("settingsModal").classList.add("hidden");
});
$("settingsModal").addEventListener("click", (event) => {
  if (event.target === $("settingsModal")) $("settingsModal").classList.add("hidden");
});
for (const id of ["languageSelect", "cursorSelect"]) {
  $(id).addEventListener("change", async () => {
    await saveSettings().catch((error) => log("保存设置失败", String(error)));
    applySettings();
    showToast(
      tr("settingsUpdated"),
      id === "languageSelect" ? tr("languageSaved") : tr("cursorApplied"),
      "success",
      2200,
    );
  });
}
$("newProfile").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyAdding"), async () => {
  const profile = {
    name: uniqueProfileName(),
    user: "",
    host: "",
    serverPort: 22,
    baseLocalPort: 2222,
    authMode: "key",
    identityFileName: "",
    identityPassphrase: "",
    serverPassword: "",
    hostAlias: "tunnel-host",
    reverseTunnel: false,
    reverseTargetUser: "",
    reverseTargetPort: 22,
  };
  state.profiles.push(profile);
  state.selectedProfileName = profile.name;
  fillEditor(profile);
  renderProfileLists();
  log("新增配置档案", profile.name);
  showToast(tr("profileAdded"), profile.name, "success", 2400);
}));
$("applyProfile").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyApplying"), async () => {
  const profile = await saveProfiles(false);
  fillConnect(profile);
  setPage("connectPage");
  log("保存并应用配置到连接页", profile.name);
  showToast(tr("profileApplied"), profile.name, "success", 2600);
}));
$("connectProfile").addEventListener("change", () => {
  const profile = profileByName($("connectProfile").value);
  if (profile) {
    state.selectedProfileName = profile.name;
    fillConnect(profile);
    renderProfileLists();
    log("连接页应用档案", profile.name);
  }
});
$("autoVmIp").addEventListener("change", () => refreshVmIps().then(updateConfig));
$("saveProfiles").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busySaving"), () => saveProfiles()).catch((error) => {
  log("保存失败", String(error));
  showToast(tr("profileSaveFailed"), String(error), "error", 5200);
}));
$("deleteProfile").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyDeleting"), async () => {
  const name = state.selectedProfileName || $("profileName").value.trim();
  if (!name) return log("没有可删除的配置");
  const confirmed = await confirmAction(tr("profileDeleteTitle"), tr("profileDeleteMessage", name), tr("delete"));
  if (!confirmed) return;
  state.profiles = state.profiles.filter((profile) => profile.name !== name);
  state.selectedProfileName = state.profiles[0]?.name || "";
  await invoke("save_profiles", { profiles: state.profiles });
  if (state.selectedProfileName) {
    fillEditor(profileByName(state.selectedProfileName));
  } else {
    fillEditor({
      name: "",
      user: "",
      host: "",
      serverPort: 22,
      baseLocalPort: 2222,
      authMode: "key",
      identityFileName: "",
      identityPassphrase: "",
      serverPassword: "",
      hostAlias: "tunnel-host",
      reverseTunnel: false,
      reverseTargetUser: "",
      reverseTargetPort: 22,
    });
  }
  renderProfileLists();
  log("删除配置档案", name);
  showToast(tr("profileDeleted"), name, "success");
}));
$("startTunnel").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyStarting"), () => startTunnel()).catch((error) => {
  log("启动失败", String(error));
  showToast(tr("startStopped"), String(error), "error", 5600);
}));
$("openConnections").addEventListener("click", () => setPage("connectionsPage"));
$("pingHost").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyPinging"), () => pingHost()).catch((error) => {
  log("Ping 失败", String(error));
  showToast(tr("pingFailed"), String(error), "error", 5200);
}));
$("sshTest").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyTesting"), () => sshTest()).catch((error) => {
  log("SSH 测试失败", String(error));
  showToast(tr("sshCheckFailed"), String(error), "error", 5600);
}));
$("firewallRule").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyAllowing"), () => ensureFirewall()).catch((error) => {
  log("防火墙设置失败", String(error));
  showToast(tr("firewallFailed"), String(error), "error", 5600);
}));
$("refreshPort").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyChecking"), () => refreshPort(true).then(updateConfig)).catch((error) => {
  log("端口检测失败", String(error));
  showToast(tr("refreshFailed"), String(error), "error", 5200);
}));
$("exportSelectedKey").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyExporting"), () => exportSelectedKey()).catch((error) => {
  log("导出失败", String(error));
  showToast(tr("exportFailed"), String(error), "error", 5200);
}));
$("importSelectedKey").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyImporting"), () => importSelectedKey()).catch((error) => {
  log("导入失败", String(error));
  showToast(tr("importFailed"), String(error), "error", 5200);
}));
$("exportPublicBundle").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyExporting"), () => exportPublicBundle()).catch((error) => {
  log("导出公钥失败", String(error));
  showToast(tr("publicBundleFailed"), String(error), "error", 5200);
}));
$("deleteSelectedKeys").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyDeleting"), () => deleteSelectedKeys()).catch((error) => {
  log("删除密钥失败", String(error));
  showToast(tr("deleteKeysFailed"), String(error), "error", 5600);
}));
$("selectAllKeys").addEventListener("change", (event) => {
  setKeyGroupSelection(state.keys, event.target.checked, "密钥");
});
$("selectAllUserKeys").addEventListener("change", (event) => {
  setKeyGroupSelection(state.keys.filter(isUserKey), event.target.checked, "用户密钥");
});
$("selectAllPortableKeys").addEventListener("change", (event) => {
  setKeyGroupSelection(state.keys.filter((key) => !isUserKey(key)), event.target.checked, "程序目录密钥");
});
$("openUserKeyDir").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyOpening"), async () => {
  log("打开用户密钥目录", "~/.ssh");
  showToast(tr("userKeyDirOpening"), "~/.ssh", "info", 2600);
  await invoke("open_path", { path: "~/.ssh" });
  showToast(tr("userKeyDirOpened"), "~/.ssh", "success", 2600);
}).catch((error) => {
  log("打开用户密钥目录失败", String(error));
  showToast(tr("openFailed"), String(error), "error", 5200);
}));
$("openPortableKeyDir").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyOpening"), async () => {
  log("打开程序相对目录密钥位置", "ssh_keyfiles_export");
  showToast(tr("portableKeyDirOpening"), "ssh_keyfiles_export", "info", 2600);
  await invoke("open_path", { path: "ssh_keyfiles_export" });
  showToast(tr("portableKeyDirOpened"), "ssh_keyfiles_export", "success", 2600);
}).catch((error) => {
  log("打开程序相对目录失败", String(error));
  showToast(tr("openFailed"), String(error), "error", 5200);
}));
$("syncInstallFields").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyFilling"), async () => {
  syncInstallFields(true);
  setInstallStatus("已自动填充");
  log("自动填充安装信息", installTarget());
  showToast(tr("installFieldsFilled"), installTarget(), "success", 2400);
}));
$("installPublicKey").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyInstalling"), () => installPublicKey()).catch((error) => {
  setInstallStatus("失败");
  log("公钥安装失败", String(error));
}));
$("installPublicKeyTerminal").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyOpening"), () => installPublicKeyInTerminal()).catch((error) => {
  setInstallStatus("失败");
  log("打开终端失败", String(error));
}));
$("generateInstallKey").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyGenerating"), () => generateInstallKey()).catch((error) => {
  setInstallStatus("失败");
  log("生成密钥失败", String(error));
  showToast(tr("keyGenerateFailed"), String(error), "error", 5600);
}));
$("verifyInstalledKey").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyVerifying"), () => verifyInstalledKey()).catch((error) => {
  setInstallStatus("失败");
  log("验证密钥登录失败", String(error));
  showToast(tr("verifyFailed"), String(error), "error", 5600);
}));
$("writeConfig").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyWriting"), () => writeConfig()).catch((error) => {
  log("写入失败", String(error));
  showToast(tr("writeSshConfigFailed"), String(error), "error", 5200);
}));
$("copyConfig").addEventListener("click", (event) => runButtonAction(event.currentTarget, tr("busyCopying"), async () => {
  await navigator.clipboard.writeText($("sshConfig").textContent);
  log("已复制 SSH 配置");
  showToast(tr("sshConfigCopied"), tr("pasteReady"), "success", 3000);
}).catch((error) => {
  log("复制 SSH 配置失败", String(error));
  showToast(tr("copyFailed"), String(error), "error", 5200);
}));
$("clearLog").addEventListener("click", () => {
  $("log").textContent = "";
  showToast(tr("logCleared"), "", "success", 1800);
});
$("contextCopy").addEventListener("click", async () => {
  const text = selectedText();
  if (text) {
    await navigator.clipboard.writeText(text);
    showToast(tr("copied"), tr("charsCopied", text.length), "success", 1800);
  }
  hideContextMenu();
});
$("contextMenu").addEventListener("click", (event) => event.stopPropagation());
document.addEventListener("contextmenu", (event) => {
  event.preventDefault();
  const text = selectedText();
  if (!text.trim()) {
    hideContextMenu();
    return;
  }

  const menu = $("contextMenu");
  menu.style.left = `${Math.min(event.clientX, window.innerWidth - 132)}px`;
  menu.style.top = `${Math.min(event.clientY, window.innerHeight - 48)}px`;
  menu.classList.add("active");
});
document.addEventListener("click", hideContextMenu);
document.addEventListener("keydown", hideContextMenu);
document.addEventListener("scroll", hideContextMenu, true);
$("connectionList").addEventListener("click", (event) => {
  const stopButton = event.target.closest("[data-stop-pid]");
  if (stopButton) {
    runButtonAction(stopButton, tr("busyStopping"), () => stopTunnel(Number(stopButton.dataset.stopPid))).catch((error) => log("停止失败", String(error)));
    return;
  }

  const pageButton = event.target.closest("[data-page-jump]");
  if (pageButton) setPage(pageButton.dataset.pageJump);
});
$("clearClosedConnections").addEventListener("click", () => {
  const before = state.connections.length;
  state.connections = state.connections.filter((connection) => connection.status === "running" || connection.status === "starting");
  renderConnections();
  showToast(tr("closedRecordsCleared"), tr("recordsCount", before - state.connections.length), "success");
});

async function initializeApp() {
  await loadSettings();
  applyDefaultSettings(state.appSettings.defaults);
  applySettings();
  updateInstallActionState();
  renderConnections();
  refreshAll();
}

initializeApp().catch((error) => log("初始化失败", String(error)));
