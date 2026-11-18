use crate::{
    config::{read_settings, write_settings, ServerConfig},
    models::{
        AppSettings, CommandResult, KeyBundle, PasswordTunnelRequest, Profile, TunnelRequest,
        TunnelStartResult, TunnelStatus, VmIpCandidate,
    },
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    process::{Child, Command as TokioCommand},
    sync::Mutex,
};

struct ManagedTunnel {
    child: Child,
    askpass_path: Option<PathBuf>,
    control_path: PathBuf,
    connection: ConnectionInfo,
}

#[cfg(not(windows))]
fn tunnel_connected(tunnel: &ManagedTunnel) -> bool {
    tunnel.control_path.exists()
}

#[cfg(windows)]
fn windows_ssh_children(pid: u32) -> Vec<u32> {
    use windows_sys::Win32::{Foundation::{CloseHandle, INVALID_HANDLE_VALUE}, System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS}};
    let mut children = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE { return children; }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = Process32FirstW(snapshot, &mut entry);
        while found != 0 {
            let end = entry.szExeFile.iter().position(|ch| *ch == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            if entry.th32ParentProcessID == pid && name.eq_ignore_ascii_case("ssh.exe") {
                children.push(entry.th32ProcessID);
            }
            found = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
    }
    children
}

#[cfg(windows)]
fn windows_tcp_listeners() -> Result<Vec<(u16, u32)>, String> {
    use windows_sys::Win32::{NetworkManagement::IpHelper::{GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER}, Networking::WinSock::{AF_INET, AF_INET6}};
    let mut listeners = Vec::new();
    for family in [AF_INET, AF_INET6] {
        let mut size = 0u32;
        unsafe { GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, u32::from(family), TCP_TABLE_OWNER_PID_LISTENER, 0); }
        for _ in 0..3 {
            if size < 4 || size > 16 * 1024 * 1024 { return Err("Invalid TCP table size".to_string()); }
            let mut buffer = vec![0u32; (size as usize + 3) / 4];
            let mut actual = (buffer.len() * 4) as u32;
            let status = unsafe { GetExtendedTcpTable(buffer.as_mut_ptr().cast(), &mut actual, 0, u32::from(family), TCP_TABLE_OWNER_PID_LISTENER, 0) };
            if status == 122 { size = actual; continue; }
            if status != 0 { return Err(format!("Cannot read TCP listeners: {status}")); }
            let count = buffer[0] as usize;
            let row_size = if family == AF_INET { std::mem::size_of::<MIB_TCPROW_OWNER_PID>() } else { std::mem::size_of::<MIB_TCP6ROW_OWNER_PID>() };
            if count > (actual as usize).saturating_sub(4) / row_size { return Err("Invalid TCP listener table".to_string()); }
            let rows = unsafe { buffer.as_ptr().cast::<u8>().add(4) };
            for index in 0..count {
                let ptr = unsafe { rows.add(index * row_size) };
                let (port, pid) = if family == AF_INET {
                    let row = unsafe { std::ptr::read_unaligned(ptr.cast::<MIB_TCPROW_OWNER_PID>()) };
                    (row.dwLocalPort, row.dwOwningPid)
                } else {
                    let row = unsafe { std::ptr::read_unaligned(ptr.cast::<MIB_TCP6ROW_OWNER_PID>()) };
                    (row.dwLocalPort, row.dwOwningPid)
                };
                listeners.push((u16::from_be(port as u16), pid));
            }
            size = 0;
            break;
        }
        if size != 0 { return Err("TCP listener table changed repeatedly".to_string()); }
    }
    Ok(listeners)
}

#[cfg(windows)]
fn tunnel_connected(tunnel: &ManagedTunnel) -> bool {
    let mut pids = vec![tunnel.connection.pid];
    if tunnel.connection.reconnect { pids.extend(windows_ssh_children(tunnel.connection.pid)); }
    windows_tcp_listeners().is_ok_and(|listeners| listeners.iter().any(|(port, pid)| *port == tunnel.connection.local_port && pids.contains(pid)))
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionInfo {
    pid: u32,
    user: String,
    host: String,
    server_port: u16,
    local_port: u16,
    auth_mode: &'static str,
    reconnect: bool,
    reverse_tunnel: bool,
    reverse_target_port: u16,
    identity_file: Option<String>,
    command: String,
    running: bool,
    connected: bool,
}

#[derive(Clone)]
pub struct CommandService {
    paths: Arc<AppPaths>,
    tunnels: Arc<Mutex<HashMap<u32, ManagedTunnel>>>,
    settings_lock: Arc<std::sync::Mutex<()>>,
}

struct AppPaths {
    data_dir: PathBuf,
    profiles: PathBuf,
    settings: PathBuf,
    portable_keys: PathBuf,
    public_bundle: PathBuf,
}

impl CommandService {
    pub fn new(config: &ServerConfig) -> Result<Self, String> {
        let data_dir = config.data_dir.clone();
        let portable_keys = data_dir.join("ssh_keyfiles_export");
        ensure_private_dir(&data_dir)?;
        ensure_private_dir(&portable_keys)?;

        Ok(Self {
            paths: Arc::new(AppPaths {
                profiles: data_dir.join("profiles.json"),
                settings: data_dir.join("app_settings.json"),
                public_bundle: data_dir.join("ssh_public_keys_bundle.pub"),
                portable_keys,
                data_dir,
            }),
            tunnels: Arc::new(Mutex::new(HashMap::new())),
            settings_lock: Arc::new(std::sync::Mutex::new(())),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.paths.data_dir
    }

    pub async fn stop_all(&self) {
        let pids = self
            .tunnels
            .lock()
            .await
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for pid in pids {
            let _ = self.stop_tunnel(pid).await;
        }
    }

    pub async fn invoke(&self, command: &str, args: Value) -> Result<Value, String> {
        match command {
            "load_profiles" => to_value(self.load_profiles()?),
            "save_profiles" => {
                let input: SaveProfilesArgs = parse(args)?;
                self.save_profiles(&input.profiles)?;
                Ok(Value::Null)
            }
            "load_app_settings" => to_value(self.load_app_settings()?),
            "save_app_settings" => {
                let input: SaveSettingsArgs = parse(args)?;
                self.save_app_settings(&input.settings)?;
                Ok(Value::Null)
            }
            "scan_key_bundles" => to_value(self.scan_key_bundles()?),
            "generate_ssh_key" => {
                let input: GenerateKeyArgs = parse(args)?;
                to_value(self.generate_ssh_key(input)?)
            }
            "export_key_files" => {
                let input: PrivatePathsArgs = parse(args)?;
                to_value(self.copy_key_files(input.private_paths, &self.paths.portable_keys)?)
            }
            "import_key_files" => {
                let input: PrivatePathsArgs = parse(args)?;
                to_value(self.copy_key_files(input.private_paths, &user_ssh_dir()?)?)
            }
            "delete_key_files" => {
                let input: PrivatePathsArgs = parse(args)?;
                to_value(self.delete_key_files(input.private_paths)?)
            }
            "export_public_bundle" => {
                let input: PublicPathsArgs = parse(args)?;
                to_value(self.export_public_bundle(input.public_paths)?)
            }
            "install_public_to_linux" => {
                let input: InstallPublicKeyArgs = parse(args)?;
                to_value(install_public_key(input, None).await?)
            }
            "install_public_to_linux_password" => {
                let input: InstallPublicPasswordArgs = parse(args)?;
                let install = InstallPublicKeyArgs {
                    target: input.target,
                    port: input.port,
                    public_key_path: input.public_key_path,
                    identity_file: None,
                };
                to_value(install_public_key(install, Some(input.password)).await?)
            }
            "detect_vm_ips" => to_value(detect_vm_ips()),
            "find_free_port" => {
                let input: StartPortArgs = parse(args)?;
                to_value(find_free_port(input.start_port)?)
            }
            "is_local_port_open" => {
                let input: PortArgs = parse(args)?;
                to_value(is_local_port_open(input.port))
            }
            "is_local_port_available" => {
                let input: PortArgs = parse(args)?;
                to_value(is_local_port_available(input.port))
            }
            "ping_host" => {
                let input: PingArgs = parse(args)?;
                to_value(ping_host(input.host, input.timeout_sec).await?)
            }
            "test_ssh_server" => {
                let input: TestSshArgs = parse(args)?;
                to_value(test_ssh_server(input).await?)
            }
            "test_password_ssh_server" => {
                let input: TestPasswordArgs = parse(args)?;
                to_value(test_password_ssh_server(input).await?)
            }
            "start_tunnel" => {
                let input: TunnelEnvelope = parse(args)?;
                to_value(self.start_key_tunnel(input.request).await?)
            }
            "start_password_tunnel" => {
                let input: PasswordTunnelEnvelope = parse(args)?;
                to_value(self.start_password_tunnel(input.request).await?)
            }
            "check_tunnel_status" => {
                let input: PidArgs = parse(args)?;
                to_value(self.check_tunnel_status(input.pid).await?)
            }
            "list_connections" => to_value(self.list_connections().await?),
            "stop_tunnel" => {
                let input: PidArgs = parse(args)?;
                self.stop_tunnel(input.pid).await?;
                Ok(Value::Null)
            }
            "generate_vscode_config" => {
                let input: GenerateConfigArgs = parse(args)?;
                to_value(generate_ssh_config(input))
            }
            "generate_reverse_proxy_config" => {
                let input: GenerateReverseConfigArgs = parse(args)?;
                to_value(generate_reverse_proxy_config(input))
            }
            "write_vscode_config" => {
                let input: GenerateConfigArgs = parse(args)?;
                to_value(write_ssh_config(input)?)
            }
            "is_admin" => to_value(is_admin()),
            "ensure_firewall_rule" | "request_firewall_admin" => to_value(CommandResult {
                success: false,
                message: "Linux server mode does not modify the host firewall automatically.".to_string(),
            }),
            "open_path" => {
                let input: OpenPathArgs = parse(args)?;
                let resolved = resolve_open_path(&input.path, &self.paths)?;
                to_value(resolved.to_string_lossy().to_string())
            }
            "window_minimize" | "window_toggle_maximize" | "window_close"
            | "window_start_drag" | "hide_process_window" => Ok(Value::Null),
            "launch_public_install_terminal" => Err(
                "Interactive terminal installation is unavailable in headless mode. Use password or key-based background installation."
                    .to_string(),
            ),
            other => Err(format!("Unsupported Linux server command: {other}")),
        }
    }

    fn load_profiles(&self) -> Result<Vec<Profile>, String> {
        if !self.paths.profiles.exists() {
            return Ok(Vec::new());
        }
        read_json(&self.paths.profiles)
    }

    fn save_profiles(&self, profiles: &[Profile]) -> Result<(), String> {
        write_json(&self.paths.profiles, profiles)
    }

    fn load_app_settings(&self) -> Result<AppSettings, String> {
        if !self.paths.settings.exists() {
            return Ok(AppSettings::default());
        }
        read_json(&self.paths.settings).or_else(|_| Ok(AppSettings::default()))
    }

    fn save_app_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let _guard = self.settings_lock.lock().map_err(|_| "Settings lock poisoned")?;
        let mut existing = read_settings(&self.paths.settings)?;
        let incoming = serde_json::to_value(settings).map_err(|error| error.to_string())?;
        for (key, value) in incoming.as_object().ok_or("Invalid settings")? {
            existing[key] = value.clone();
        }
        write_settings(&self.paths.settings, &existing)
    }

    #[cfg(windows)]
    pub(crate) fn tray_language(&self) -> Result<String, String> {
        let _guard = self.settings_lock.lock().map_err(|_| "Settings lock poisoned")?;
        let settings = read_settings(&self.paths.settings)?;
        Ok(if settings.get("language").and_then(Value::as_str) == Some("en") { "en" } else { "zh" }.to_string())
    }

    #[cfg(windows)]
    pub(crate) fn set_tray_language(&self, language: &str) -> Result<(), String> {
        if language != "en" && language != "zh" { return Err("Unsupported language".to_string()); }
        let _guard = self.settings_lock.lock().map_err(|_| "Settings lock poisoned")?;
        let mut existing = read_settings(&self.paths.settings)?;
        existing["language"] = Value::String(language.to_string());
        write_settings(&self.paths.settings, &existing)
    }

    fn scan_key_bundles(&self) -> Result<Vec<KeyBundle>, String> {
        let mut bundles = Vec::new();
        let mut seen = HashSet::new();
        for dir in [&self.paths.portable_keys, &user_ssh_dir()?] {
            if !dir.is_dir() {
                continue;
            }
            for entry in fs::read_dir(dir)
                .map_err(|error| error.to_string())?
                .flatten()
            {
                let path = entry.path();
                if !path.is_file() || !looks_like_private_key(&path) {
                    continue;
                }
                let canonical = path.canonicalize().unwrap_or(path);
                if !seen.insert(canonical.clone()) {
                    continue;
                }
                bundles.push(key_bundle_from_private(&canonical, dir));
            }
        }
        bundles.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(bundles)
    }

    fn generate_ssh_key(&self, input: GenerateKeyArgs) -> Result<KeyBundle, String> {
        let name = sanitize_key_name(&input.name);
        if name.is_empty() {
            return Err("Invalid key filename".to_string());
        }
        let private_path = self.paths.portable_keys.join(&name);
        let public_path = self.paths.portable_keys.join(format!("{name}.pub"));
        if private_path.exists() || public_path.exists() {
            if input.overwrite.unwrap_or(false) {
                let _ = fs::remove_file(&private_path);
                let _ = fs::remove_file(&public_path);
            } else {
                return Err(format!("Key already exists: {}", private_path.display()));
            }
        }

        let output = Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N"])
            .arg(input.passphrase.unwrap_or_default())
            .arg("-f")
            .arg(&private_path)
            .arg("-C")
            .arg(input.comment.unwrap_or_else(|| "tunnel-helper".to_string()))
            .arg("-q")
            .output()
            .map_err(|error| format!("Cannot run ssh-keygen: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        secure_private_key(&private_path)?;
        Ok(key_bundle_from_private(
            &private_path,
            &self.paths.portable_keys,
        ))
    }

    fn copy_key_files(
        &self,
        private_paths: Vec<String>,
        target: &Path,
    ) -> Result<Vec<KeyBundle>, String> {
        ensure_private_dir(target)?;
        let mut copied = Vec::new();
        for raw in private_paths {
            let source = PathBuf::from(raw);
            if !looks_like_private_key(&source) {
                return Err(format!("Not a private key: {}", source.display()));
            }
            let filename = source
                .file_name()
                .ok_or_else(|| "Missing key filename".to_string())?;
            let destination = target.join(filename);
            fs::copy(&source, &destination).map_err(|error| error.to_string())?;
            secure_private_key(&destination)?;
            let public_source =
                source.with_file_name(format!("{}.pub", filename.to_string_lossy()));
            if public_source.is_file() {
                fs::copy(
                    &public_source,
                    target.join(format!("{}.pub", filename.to_string_lossy())),
                )
                .map_err(|error| error.to_string())?;
            }
            copied.push(key_bundle_from_private(&destination, target));
        }
        Ok(copied)
    }

    fn delete_key_files(&self, private_paths: Vec<String>) -> Result<Vec<String>, String> {
        let allowed = [
            self.paths.portable_keys.canonicalize().ok(),
            user_ssh_dir()?.canonicalize().ok(),
        ];
        let mut deleted = Vec::new();
        for raw in private_paths {
            let path = PathBuf::from(raw);
            let canonical = path.canonicalize().map_err(|error| error.to_string())?;
            let is_allowed = allowed
                .iter()
                .flatten()
                .any(|dir| canonical.parent().is_some_and(|parent| parent == dir));
            if !is_allowed || !looks_like_private_key(&canonical) {
                return Err(format!(
                    "Refusing to delete key outside managed directories: {}",
                    canonical.display()
                ));
            }
            let public = canonical.with_file_name(format!(
                "{}.pub",
                canonical.file_name().unwrap_or_default().to_string_lossy()
            ));
            fs::remove_file(&canonical).map_err(|error| error.to_string())?;
            deleted.push(canonical.to_string_lossy().to_string());
            if public.is_file() {
                fs::remove_file(&public).map_err(|error| error.to_string())?;
                deleted.push(public.to_string_lossy().to_string());
            }
        }
        Ok(deleted)
    }

    fn export_public_bundle(&self, public_paths: Vec<String>) -> Result<String, String> {
        let mut lines = Vec::new();
        let mut seen = HashSet::new();
        for raw in public_paths {
            let text = fs::read_to_string(raw).map_err(|error| error.to_string())?;
            for line in text
                .lines()
                .map(str::trim)
                .filter(|line| is_public_key_line(line))
            {
                if seen.insert(line.to_string()) {
                    lines.push(line.to_string());
                }
            }
        }
        if lines.is_empty() {
            return Err("No valid public keys selected".to_string());
        }
        fs::write(&self.paths.public_bundle, format!("{}\n", lines.join("\n")))
            .map_err(|error| error.to_string())?;
        Ok(self.paths.public_bundle.to_string_lossy().to_string())
    }

    async fn start_key_tunnel(&self, request: TunnelRequest) -> Result<TunnelStartResult, String> {
        let connection = ConnectionInfo { pid: 0, user: request.user.clone(), host: request.host.clone(),
            server_port: request.server_port, local_port: request.local_port, auth_mode: "key", reconnect: request.reconnect,
            reverse_tunnel: request.reverse_tunnel, reverse_target_port: request.reverse_target_port,
            identity_file: request.identity_file.clone(), command: String::new(), running: true, connected: false };
        let askpass = request
            .identity_passphrase
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(create_askpass_script)
            .transpose()?;
        let mut args = tunnel_args(
            &request.user,
            &request.host,
            request.server_port,
            request.local_port,
            request.reverse_tunnel,
            request.reverse_target_port,
        );
        if let Some(identity) = request.identity_file.as_deref().filter(|v| !v.is_empty()) {
            let mut identity_args = vec![
                "-i".to_string(),
                identity.to_string(),
                "-o".to_string(),
                "IdentitiesOnly=yes".to_string(),
            ];
            if askpass.is_none() {
                identity_args.extend(["-o".to_string(), "BatchMode=yes".to_string()]);
            }
            insert_before_forward(&mut args, identity_args)?;
        }
        self.spawn_tunnel(args, askpass, request.reconnect, request.reconnect_delay, connection)
            .await
    }

    async fn start_password_tunnel(
        &self,
        request: PasswordTunnelRequest,
    ) -> Result<TunnelStartResult, String> {
        let connection = ConnectionInfo { pid: 0, user: request.user.clone(), host: request.host.clone(),
            server_port: request.server_port, local_port: request.local_port, auth_mode: "password", reconnect: request.reconnect,
            reverse_tunnel: request.reverse_tunnel, reverse_target_port: request.reverse_target_port,
            identity_file: None, command: String::new(), running: true, connected: false };
        let password = request
            .password
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Server password is required in headless password mode.".to_string())?;
        let askpass = Some(create_askpass_script(&password)?);
        let mut args = tunnel_args(
            &request.user,
            &request.host,
            request.server_port,
            request.local_port,
            request.reverse_tunnel,
            request.reverse_target_port,
        );
        insert_before_forward(
            &mut args,
            [
                "-o".to_string(),
                "PreferredAuthentications=password,keyboard-interactive".to_string(),
                "-o".to_string(),
                "PubkeyAuthentication=no".to_string(),
                "-o".to_string(),
                "NumberOfPasswordPrompts=1".to_string(),
            ],
        )?;
        self.spawn_tunnel(args, askpass, request.reconnect, request.reconnect_delay, connection)
            .await
    }

    async fn spawn_tunnel(
        &self,
        args: Vec<String>,
        askpass_path: Option<PathBuf>,
        reconnect: bool,
        reconnect_delay: Option<u64>,
        connection: ConnectionInfo,
    ) -> Result<TunnelStartResult, String> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            if connection.reverse_tunnel {
                if let Some(path) = askpass_path.as_ref() { let _ = fs::remove_file(path); }
                return Err("Windows web reverse tunnels are not yet supported".to_string());
            }
            if !is_local_port_available(connection.local_port) {
                if let Some(path) = askpass_path.as_ref() { let _ = fs::remove_file(path); }
                return Err(format!("Local port {} is already in use", connection.local_port));
            }
            let command_text = format!("ssh {}", args.join(" "));
            let mut command = if reconnect {
                let quoted = args.iter().map(|value| format!("'{}'", value.replace('\'', "''"))).collect::<Vec<_>>().join(",");
                let delay = reconnect_delay.unwrap_or(5).clamp(1, 86400);
                let script = format!("$sshArgs=@({quoted}); while ($true) {{ & ssh.exe @sshArgs; Start-Sleep -Seconds {delay} }}");
                let mut shell = TokioCommand::new("powershell.exe");
                shell.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
                shell
            } else {
                let mut ssh = TokioCommand::new("ssh.exe");
                ssh.args(&args);
                ssh
            };
            command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
            command.as_std_mut().creation_flags(0x08000000);
            configure_askpass(&mut command, askpass_path.as_deref());
            let child = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    if let Some(path) = askpass_path.as_ref() { let _ = fs::remove_file(path); }
                    return Err(error.to_string());
                }
            };
            let pid = child.id().ok_or_else(|| "Tunnel process has no PID".to_string())?;
            let connection = ConnectionInfo { pid, command: command_text.clone(), ..connection };
            self.tunnels.lock().await.insert(pid, ManagedTunnel { child, askpass_path,
                control_path: PathBuf::new(), connection });
            return Ok(TunnelStartResult { pid, command: command_text, connected_marker: None });
        }
        #[cfg(not(windows))]
        {
        let mut args = args;
        let control_path = env::temp_dir().join(format!(
            "tunnel-helper-control-{}-{}.sock",
            std::process::id(),
            rand::random::<u64>()
        ));
        let target_index = args
            .len()
            .checked_sub(1)
            .ok_or_else(|| "Tunnel command is missing the SSH target".to_string())?;
        args.splice(
            target_index..target_index,
            [
                "-o".to_string(),
                "ControlMaster=yes".to_string(),
                "-o".to_string(),
                format!("ControlPath={}", control_path.display()),
                "-o".to_string(),
                "ControlPersist=no".to_string(),
            ],
        );
        let command_text = format!("ssh {}", args.join(" "));
        let mut command = if reconnect {
            let mut shell = TokioCommand::new("sh");
            shell
                .arg("-c")
                .arg(
                    "while true; do rm -f \"$TUNNEL_CONTROL_PATH\"; ssh \"$@\"; rm -f \"$TUNNEL_CONTROL_PATH\"; sleep \"$TUNNEL_RECONNECT_DELAY\"; done",
                )
                .arg("tunnel-helper")
                .args(&args)
                .env(
                    "TUNNEL_RECONNECT_DELAY",
                    reconnect_delay.unwrap_or(5).max(1).to_string(),
                )
                .env("TUNNEL_CONTROL_PATH", &control_path);
            shell
        } else {
            let mut ssh = TokioCommand::new("ssh");
            ssh.args(&args);
            ssh
        };
        configure_askpass(&mut command, askpass_path.as_deref());
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        configure_process_group(&mut command);
        let child = command.spawn().map_err(|error| error.to_string())?;
        let pid = child
            .id()
            .ok_or_else(|| "Tunnel process has no PID".to_string())?;
        let connection = ConnectionInfo { pid, command: command_text.clone(), ..connection };
        self.tunnels.lock().await.insert(
            pid,
            ManagedTunnel {
                child,
                askpass_path,
                control_path: control_path.clone(),
                    connection,
            },
        );
        Ok(TunnelStartResult {
            pid,
            command: command_text,
            connected_marker: Some(control_path.to_string_lossy().to_string()),
        })
        }
    }

    async fn list_connections(&self) -> Result<Vec<ConnectionInfo>, String> {
        #[cfg(windows)]
        let listeners = windows_tcp_listeners()?;
        let mut tunnels = self.tunnels.lock().await;
        let mut connections = Vec::with_capacity(tunnels.len());
        for tunnel in tunnels.values_mut() {
            let mut connection = tunnel.connection.clone();
            connection.running = tunnel.child.try_wait().map_err(|error| error.to_string())?.is_none();
            #[cfg(not(windows))]
            { connection.connected = connection.running && tunnel_connected(tunnel); }
            #[cfg(windows)]
            {
                let mut pids = vec![connection.pid];
                if connection.running && connection.reconnect { pids.extend(windows_ssh_children(connection.pid)); }
                connection.connected = connection.running && listeners.iter().any(|(port, pid)| *port == connection.local_port && pids.contains(pid));
            }
            connections.push(connection);
        }
        connections.sort_by_key(|connection| std::cmp::Reverse(connection.pid));
        Ok(connections)
    }

    async fn check_tunnel_status(&self, pid: u32) -> Result<TunnelStatus, String> {
        let mut tunnels = self.tunnels.lock().await;
        let Some(tunnel) = tunnels.get_mut(&pid) else {
            return Ok(TunnelStatus {
                pid,
                running: false,
                exit_code: None,
                connected: false,
            });
        };
        match tunnel.child.try_wait().map_err(|error| error.to_string())? {
            None => Ok(TunnelStatus {
                pid,
                running: true,
                exit_code: None,
                connected: tunnel_connected(tunnel),
            }),
            Some(status) => {
                let exit_code = status.code();
                if let Some(path) = tunnel.askpass_path.take() {
                    let _ = fs::remove_file(path);
                }
                let _ = fs::remove_file(&tunnel.control_path);
                tunnels.remove(&pid);
                Ok(TunnelStatus {
                    pid,
                    running: false,
                    exit_code,
                    connected: false,
                })
            }
        }
    }

    async fn stop_tunnel(&self, pid: u32) -> Result<(), String> {
        let Some(mut tunnel) = self.tunnels.lock().await.remove(&pid) else {
            return Ok(());
        };
        terminate_process_group(pid);
        let _ = tunnel.child.kill().await;
        let _ = tunnel.child.wait().await;
        if let Some(path) = tunnel.askpass_path.take() {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_file(&tunnel.control_path);
        Ok(())
    }
}

fn parse<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("Invalid command arguments: {error}"))
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn write_json<T: serde::Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    #[cfg(windows)]
    {
        let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
        return write_settings(path, &value);
    }
    #[cfg(not(windows))]
    {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| error.to_string())?;
    secure_private_file(path)
    }
}

fn home_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let home = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"));
    #[cfg(not(windows))]
    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"));
    home
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

fn user_ssh_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".ssh"))
}

fn looks_like_private_key(path: &Path) -> bool {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pub"))
    {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if matches!(name, "config" | "known_hosts" | "authorized_keys") {
        return false;
    }
    fs::read_to_string(path).ok().is_some_and(|text| {
        text.lines()
            .take(5)
            .any(|line| line.contains("BEGIN") && line.contains("PRIVATE KEY"))
    })
}

fn key_bundle_from_private(path: &Path, source_dir: &Path) -> KeyBundle {
    let public = path.with_file_name(format!(
        "{}.pub",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let public = public.is_file().then_some(public);
    KeyBundle {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        private_path: path.to_string_lossy().to_string(),
        public_path: public
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        fingerprint: public.as_deref().and_then(fingerprint_for),
        source_dir: source_dir.to_string_lossy().to_string(),
    }
}

fn fingerprint_for(public_key: &Path) -> Option<String> {
    let output = Command::new("ssh-keygen")
        .arg("-lf")
        .arg(public_key)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn sanitize_key_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('.')
        .to_string()
}

fn is_public_key_line(line: &str) -> bool {
    line.starts_with("ssh-rsa ")
        || line.starts_with("ssh-ed25519 ")
        || line.starts_with("ecdsa-sha2-")
        || line.starts_with("sk-ssh-")
        || line.starts_with("sk-ecdsa-")
}

#[cfg(unix)]
fn ensure_private_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path).map_err(|error| error.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("Cannot set directory mode 0700: {error}"))
}

#[cfg(not(unix))]
fn ensure_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn secure_private_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("Cannot set private file mode 0600: {error}"))
}

#[cfg(not(any(unix, windows)))]
fn secure_private_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn secure_private_file(path: &Path) -> Result<(), String> {
    secure_private_key(path)
}

#[cfg(not(windows))]
fn secure_private_key(path: &Path) -> Result<(), String> {
    secure_private_file(path)
}

#[cfg(windows)]
pub(crate) fn secure_private_key(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let script = r#"$ErrorActionPreference = 'Stop'
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
$acl = New-Object System.Security.AccessControl.FileSecurity
$acl.SetAccessRuleProtection($true, $false)
$rule = New-Object System.Security.AccessControl.FileSystemAccessRule($sid, 'FullControl', 'Allow')
$acl.AddAccessRule($rule)
[System.IO.File]::SetAccessControl($env:TUNNEL_HELPER_ACL_PATH, $acl)"#;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("TUNNEL_HELPER_ACL_PATH", path)
        .creation_flags(0x08000000)
        .output()
        .map_err(|error| format!("Cannot set private-key ACL: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("Cannot set private-key ACL: {}", String::from_utf8_lossy(&output.stderr).trim()))
    }
}

fn detect_vm_ips() -> Vec<VmIpCandidate> {
    let mut candidates = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| match interface.ip() {
            IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => Some(VmIpCandidate {
                ip: ip.to_string(),
                source: interface.name,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.ip.cmp(&right.ip));
    candidates
}

fn find_free_port(start: u16) -> Result<u16, String> {
    #[cfg(windows)]
    let occupied: HashSet<u16> = windows_tcp_listeners()?.into_iter().map(|(port, _)| port).collect();
    for port in start..=u16::MAX {
        #[cfg(windows)]
        if occupied.contains(&port) { continue; }
        #[cfg(windows)]
        if TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port)).is_ok() {
            return Ok(port);
        }
        #[cfg(not(windows))]
        if is_local_port_available(port) {
            return Ok(port);
        }
    }
    Err(format!("No free port found from {start}"))
}

fn is_local_port_available(port: u16) -> bool {
    #[cfg(windows)]
    {
        let listeners = match windows_tcp_listeners() {
            Ok(listeners) => listeners,
            _ => return false,
        };
        if listeners.iter().any(|(listening_port, _)| *listening_port == port) { return false; }
    }
    TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port)).is_ok()
}

fn is_local_port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        Duration::from_millis(350),
    )
    .is_ok()
}

fn ping_args(host: &str, timeout_sec: u16) -> Vec<String> {
    #[cfg(windows)]
    let args = ["-n".to_string(), "1".to_string(), "-w".to_string(),
        (u32::from(timeout_sec.max(1)) * 1000).to_string()];
    #[cfg(not(windows))]
    let args = ["-c".to_string(), "1".to_string(), "-W".to_string(),
        timeout_sec.max(1).to_string()];
    args.into_iter().chain(std::iter::once(host.to_string())).collect()
}

async fn ping_host(host: String, timeout_sec: u16) -> Result<CommandResult, String> {
    let output = TokioCommand::new("ping")
        .args(ping_args(&host, timeout_sec))
        .output()
        .await
        .map_err(|error| format!("Cannot run ping: {error}"))?;
    Ok(CommandResult {
        success: output.status.success(),
        message: if output.status.success() {
            format!("ping {host}: success")
        } else {
            format!("ping {host}: failed or timed out")
        },
    })
}

async fn test_ssh_server(input: TestSshArgs) -> Result<CommandResult, String> {
    let askpass = input
        .identity_passphrase
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(create_askpass_script)
        .transpose()?;
    if let (Some(identity), Some(askpass_path)) = (
        input
            .identity_file
            .as_deref()
            .filter(|value| !value.is_empty()),
        askpass.as_deref(),
    ) {
        let unlocked = match private_key_unlocks(identity, askpass_path).await {
            Ok(unlocked) => unlocked,
            Err(error) => {
                let _ = fs::remove_file(askpass_path);
                return Err(error);
            }
        };
        if !unlocked {
            let _ = fs::remove_file(askpass_path);
            return Ok(CommandResult {
                success: false,
                message:
                    "The private key passphrase is incorrect or the key could not be unlocked."
                        .to_string(),
            });
        }
    }
    let mut args = vec![
        "-T".to_string(),
        "-p".to_string(),
        input.port.to_string(),
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
    ];
    if askpass.is_none() {
        args.extend(["-o".to_string(), "BatchMode=yes".to_string()]);
    }
    if let Some(identity) = input
        .identity_file
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        args.extend([
            "-i".to_string(),
            identity.to_string(),
            "-o".to_string(),
            "IdentitiesOnly=yes".to_string(),
        ]);
    }
    args.push(format!("{}@{}", input.user, input.host));
    args.push("exit".to_string());
    run_ssh_test(args, askpass, input.identity_passphrase.is_some()).await
}

async fn private_key_unlocks(identity: &str, askpass_path: &Path) -> Result<bool, String> {
    let mut command = TokioCommand::new("ssh-keygen");
    command
        .args(["-y", "-f", identity])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_askpass(&mut command, Some(askpass_path));
    command
        .status()
        .await
        .map(|status| status.success())
        .map_err(|error| format!("Cannot validate private key passphrase: {error}"))
}

async fn test_password_ssh_server(input: TestPasswordArgs) -> Result<CommandResult, String> {
    let askpass = Some(create_askpass_script(&input.password)?);
    let args = vec![
        "-T".to_string(),
        "-p".to_string(),
        input.port.to_string(),
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "PreferredAuthentications=password,keyboard-interactive".to_string(),
        "-o".to_string(),
        "PubkeyAuthentication=no".to_string(),
        "-o".to_string(),
        "NumberOfPasswordPrompts=1".to_string(),
        format!("{}@{}", input.user, input.host),
        "exit".to_string(),
    ];
    run_ssh_test(args, askpass, false).await
}

async fn install_public_key(
    input: InstallPublicKeyArgs,
    password: Option<String>,
) -> Result<CommandResult, String> {
    let public_key = fs::read_to_string(&input.public_key_path)
        .map_err(|error| format!("Cannot read public key {}: {error}", input.public_key_path))?;
    let public_key = public_key.trim_start_matches('\u{feff}').trim();
    if !public_key.starts_with("ssh-") && !public_key.starts_with("ecdsa-") {
        return Err(
            "The selected file does not contain a supported OpenSSH public key.".to_string(),
        );
    }

    let askpass = password
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(create_askpass_script)
        .transpose()?;
    let mut args = vec![
        "-T".to_string(),
        "-p".to_string(),
        input.port.to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
    ];
    if askpass.is_some() {
        args.extend([
            "-o".to_string(),
            "PreferredAuthentications=password,keyboard-interactive".to_string(),
            "-o".to_string(),
            "PubkeyAuthentication=no".to_string(),
        ]);
    } else if let Some(identity) = input
        .identity_file
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        args.extend([
            "-i".to_string(),
            identity.to_string(),
            "-o".to_string(),
            "IdentitiesOnly=yes".to_string(),
            "-o".to_string(),
            "BatchMode=yes".to_string(),
        ]);
    } else {
        return Err("A server password or an existing login key is required.".to_string());
    }
    args.push(input.target.clone());
    #[cfg(not(windows))]
    args.push(
        "umask 077; mkdir -p ~/.ssh; touch ~/.ssh/authorized_keys; cat | tr -d '\\r' >> ~/.ssh/authorized_keys; awk 'NF && !seen[$0]++' ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp && mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys; chmod 700 ~/.ssh; chmod 600 ~/.ssh/authorized_keys"
            .to_string(),
    );
    #[cfg(windows)]
    args.push("umask 077; mkdir -p ~/.ssh; touch ~/.ssh/authorized_keys; cat | tr -d '\\r' >> ~/.ssh/authorized_keys; chmod 700 ~/.ssh; chmod 600 ~/.ssh/authorized_keys".to_string());

    let mut command = TokioCommand::new("ssh");
    command
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_askpass(&mut command, askpass.as_deref());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Cannot start ssh: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(format!("{public_key}\n").as_bytes())
            .await
            .map_err(|error| format!("Cannot send public key to ssh: {error}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("Cannot close ssh input: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Cannot wait for ssh: {error}"));
    if let Some(path) = askpass {
        let _ = fs::remove_file(path);
    }
    let output = output?;
    let details = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(CommandResult {
        success: output.status.success(),
        message: if output.status.success() {
            format!("Public key installed on {}.", input.target)
        } else {
            format!(
                "Public key installation failed (exit {}). {}",
                output
                    .status
                    .code()
                    .map_or_else(|| "unknown".to_string(), |code| code.to_string()),
                details.trim()
            )
        },
    })
}

async fn run_ssh_test(
    args: Vec<String>,
    askpass: Option<PathBuf>,
    passphrase_provided: bool,
) -> Result<CommandResult, String> {
    let mut command = TokioCommand::new("ssh");
    command.args(&args).stdin(Stdio::null());
    configure_askpass(&mut command, askpass.as_deref());
    let output = command.output().await.map_err(|error| error.to_string());
    if let Some(path) = askpass {
        let _ = fs::remove_file(path);
    }
    let output = output?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = combined.to_lowercase();
    let status = if output.status.success() {
        "SSH service is reachable and authentication succeeded."
    } else if passphrase_provided
        && (lower.contains("bad passphrase")
            || lower.contains("incorrect passphrase")
            || lower.contains("load key"))
    {
        "The private key passphrase is incorrect or the key could not be unlocked."
    } else if lower.contains("permission denied") {
        "SSH service is reachable, but authentication failed."
    } else {
        "SSH connectivity test failed."
    };
    Ok(CommandResult {
        success: output.status.success(),
        message: format!(
            "{status}\nCommand: ssh {}\nExit code: {}\nOutput: {}",
            args.join(" "),
            output
                .status
                .code()
                .map_or_else(|| "unknown".to_string(), |code| code.to_string()),
            combined.trim()
        ),
    })
}

fn tunnel_args(
    user: &str,
    host: &str,
    server_port: u16,
    tunnel_port: u16,
    reverse_tunnel: bool,
    reverse_target_port: u16,
) -> Vec<String> {
    let mut args = vec![
        "-4".to_string(),
        "-N".to_string(),
        "-p".to_string(),
        server_port.to_string(),
        "-o".to_string(),
        "ExitOnForwardFailure=yes".to_string(),
        "-o".to_string(),
        "ServerAliveInterval=30".to_string(),
        "-o".to_string(),
        "ServerAliveCountMax=3".to_string(),
    ];
    if reverse_tunnel {
        args.extend([
            "-R".to_string(),
            format!("127.0.0.1:{tunnel_port}:127.0.0.1:{reverse_target_port}"),
        ]);
    } else {
        args.extend([
            "-g".to_string(),
            "-L".to_string(),
            format!("0.0.0.0:{tunnel_port}:127.0.0.1:{server_port}"),
        ]);
    }
    args.push(format!("{user}@{host}"));
    args
}

fn insert_before_forward<I>(args: &mut Vec<String>, values: I) -> Result<(), String>
where
    I: IntoIterator<Item = String>,
{
    let index = args
        .iter()
        .position(|value| value == "-L" || value == "-R")
        .ok_or_else(|| "Tunnel command is missing a forwarding direction".to_string())?;
    args.splice(index..index, values);
    Ok(())
}

#[cfg(unix)]
fn create_askpass_script(secret: &str) -> Result<PathBuf, String> {
    let path = env::temp_dir().join(format!(
        "tunnel-helper-askpass-{}-{}.sh",
        std::process::id(),
        rand::random::<u64>()
    ));
    let escaped = secret.replace('\'', "'\"'\"'");
    fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{escaped}'\n"))
        .map_err(|error| error.to_string())?;
    set_executable(&path)?;
    Ok(path)
}

#[cfg(windows)]
fn create_askpass_script(secret: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    if secret.contains(['\r', '\n', '\0']) {
        return Err("Passphrases containing a newline or NUL are not supported".to_string());
    }
    if !secret.is_ascii() {
        return Err("Non-ASCII passphrases are not yet supported by Windows askpass".to_string());
    }
    let path = env::temp_dir().join(format!(
        "tunnel-helper-askpass-{}-{}.cmd", std::process::id(), rand::random::<u64>()
    ));
    let escaped = secret.replace('^', "^^").replace('%', "%%")
        .replace('&', "^&").replace('|', "^|").replace('<', "^<").replace('>', "^>");
    let result = (|| {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path)
            .map_err(|error| error.to_string())?;
        secure_private_key(&path)?;
        write!(file, "@echo off\r\nsetlocal DisableDelayedExpansion\r\necho({escaped}\r\n").map_err(|error| error.to_string())?;
        Ok(path.clone())
    })();
    if result.is_err() { let _ = fs::remove_file(&path); }
    result
}

#[cfg(not(any(unix, windows)))]
fn create_askpass_script(_secret: &str) -> Result<PathBuf, String> {
    Err("Askpass is not supported on this platform".to_string())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())
}

fn configure_askpass(command: &mut TokioCommand, path: Option<&Path>) {
    if let Some(path) = path {
        command
            .env("SSH_ASKPASS", path)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env(
                "DISPLAY",
                env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()),
            );
    }
}

#[cfg(unix)]
fn configure_process_group(command: &mut TokioCommand) {
    use std::os::unix::process::CommandExt;
    command.as_std_mut().process_group(0);
}

#[cfg(not(any(unix, windows)))]
fn configure_process_group(_command: &mut TokioCommand) {}

#[cfg(unix)]
fn terminate_process_group(pid: u32) {
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    }
}

#[cfg(windows)]
fn terminate_process_group(pid: u32) {
    use std::os::windows::process::CommandExt;
    let _ = Command::new("taskkill.exe")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(0x08000000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_group(_pid: u32) {}

fn generate_ssh_config(input: GenerateConfigArgs) -> String {
    let mut lines = vec![
        format!("Host {}", input.alias),
        format!("    HostName {}", input.vm_ip),
        format!("    Port {}", input.local_port),
        format!("    User {}", input.user),
        "    ServerAliveInterval 30".to_string(),
        "    ServerAliveCountMax 3".to_string(),
    ];
    if let Some(identity) = input.identity_file.filter(|value| !value.is_empty()) {
        lines.extend([
            format!("    IdentityFile {identity}"),
            "    IdentitiesOnly yes".to_string(),
            "    PreferredAuthentications publickey".to_string(),
        ]);
    }
    lines.join("\n")
}

fn generate_reverse_proxy_config(input: GenerateReverseConfigArgs) -> String {
    let jump_alias = format!("{}-jump", input.alias);
    let target_user = if input.target_user.trim().is_empty() {
        "<B-user>"
    } else {
        input.target_user.trim()
    };
    let example_key_name = |user: &str| {
        let name = sanitize_key_name(user.trim());
        if name.is_empty() {
            "user".to_string()
        } else {
            name
        }
    };
    [
        format!("Host {jump_alias}"),
        format!("    HostName {}", input.jump_host),
        format!("    Port {}", input.jump_port),
        format!("    User {}", input.jump_user),
        "    # Set the relay server private key path on this computer:".to_string(),
        format!(
            "    # IdentityFile ~/.ssh/id_ed25519_{}",
            example_key_name(&input.jump_user)
        ),
        "    # IdentitiesOnly yes".to_string(),
        "    ServerAliveInterval 30".to_string(),
        "    ServerAliveCountMax 3".to_string(),
        String::new(),
        format!("Host {}", input.alias),
        "    HostName 127.0.0.1".to_string(),
        format!("    Port {}", input.remote_port),
        format!("    User {target_user}"),
        "    # Set the target host private key path on this computer:".to_string(),
        format!(
            "    # IdentityFile ~/.ssh/id_ed25519_{}",
            example_key_name(&input.target_user)
        ),
        "    # IdentitiesOnly yes".to_string(),
        format!("    ProxyJump {jump_alias}"),
        format!("    HostKeyAlias {}", input.alias),
        "    ServerAliveInterval 30".to_string(),
        "    ServerAliveCountMax 3".to_string(),
    ]
    .join("\n")
}

fn write_ssh_config(input: GenerateConfigArgs) -> Result<String, String> {
    let alias = input.alias.clone();
    let block_text = generate_ssh_config(input);
    let ssh_dir = user_ssh_dir()?;
    ensure_private_dir(&ssh_dir)?;
    let path = ssh_dir.join("config");
    let begin = format!("# BEGIN tunnel-helper {alias}");
    let end = format!("# END tunnel-helper {alias}");
    let block = format!("{begin}\n{block_text}\n{end}\n");
    let old = fs::read_to_string(&path).unwrap_or_default();
    let next = if let Some(start) = old.find(&begin) {
        if let Some(relative_end) = old[start..].find(&end) {
            let finish = start + relative_end + end.len();
            format!(
                "{}{}{}",
                &old[..start],
                block,
                old[finish..].trim_start_matches(['\r', '\n'])
            )
        } else {
            format!("{}\n\n{}", old.trim_end(), block)
        }
    } else if old.trim().is_empty() {
        block
    } else {
        format!("{}\n\n{}", old.trim_end(), block)
    };
    fs::write(&path, next).map_err(|error| error.to_string())?;
    secure_private_file(&path)?;
    Ok(path.to_string_lossy().to_string())
}

fn resolve_open_path(path: &str, paths: &AppPaths) -> Result<PathBuf, String> {
    if path == "~/.ssh" || path == "~\\.ssh" {
        user_ssh_dir()
    } else if path == "ssh_keyfiles_export" {
        Ok(paths.portable_keys.clone())
    } else {
        let path = PathBuf::from(path);
        Ok(if path.is_absolute() {
            path
        } else {
            paths.data_dir.join(path)
        })
    }
}

#[cfg(unix)]
fn is_admin() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
fn is_admin() -> bool {
    false
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveProfilesArgs {
    profiles: Vec<Profile>,
}

#[derive(Deserialize)]
struct SaveSettingsArgs {
    settings: AppSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateKeyArgs {
    name: String,
    comment: Option<String>,
    passphrase: Option<String>,
    overwrite: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivatePathsArgs {
    private_paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicPathsArgs {
    public_paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallPublicKeyArgs {
    target: String,
    port: u16,
    public_key_path: String,
    identity_file: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallPublicPasswordArgs {
    target: String,
    port: u16,
    public_key_path: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartPortArgs {
    start_port: u16,
}

#[derive(Deserialize)]
struct PortArgs {
    port: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PingArgs {
    host: String,
    timeout_sec: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestSshArgs {
    user: String,
    host: String,
    port: u16,
    identity_file: Option<String>,
    identity_passphrase: Option<String>,
}

#[derive(Deserialize)]
struct TestPasswordArgs {
    user: String,
    host: String,
    port: u16,
    password: String,
}

#[derive(Deserialize)]
struct TunnelEnvelope {
    request: TunnelRequest,
}

#[derive(Deserialize)]
struct PasswordTunnelEnvelope {
    request: PasswordTunnelRequest,
}

#[derive(Deserialize)]
struct PidArgs {
    pid: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateConfigArgs {
    alias: String,
    vm_ip: String,
    local_port: u16,
    user: String,
    identity_file: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateReverseConfigArgs {
    alias: String,
    jump_host: String,
    jump_port: u16,
    jump_user: String,
    remote_port: u16,
    target_user: String,
}

#[derive(Deserialize)]
struct OpenPathArgs {
    path: String,
}

#[cfg(test)]
mod tests {
    use super::{
        generate_reverse_proxy_config, tunnel_args, AppSettings, CommandService,
        GenerateReverseConfigArgs,
    };
    use crate::config::{AuthMode, ServerConfig};

    #[cfg(windows)]
    #[test]
    fn windows_ping_uses_milliseconds() {
        assert_eq!(super::ping_args("127.0.0.1", 2), ["-n", "1", "-w", "2000", "127.0.0.1"]);
        assert_eq!(super::ping_args("127.0.0.1", 0)[3], "1000");
    }

    #[cfg(windows)]
    #[test]
    fn windows_listener_must_match_process_and_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let rows = super::windows_tcp_listeners().unwrap();
        assert!(rows.contains(&(port, std::process::id())));
        assert!(!super::is_local_port_available(port));
        drop(listener);
        assert!(super::is_local_port_available(port));
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_snapshot_finds_ssh_child() {
        use std::os::windows::process::CommandExt;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let script = format!("& ssh.exe -o ConnectTimeout=10 -p {port} test@127.0.0.1");
        let mut parent = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(0x08000000).stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
        let mut found = false;
        for _ in 0..50 {
            if !super::windows_ssh_children(parent.id()).is_empty() { found = true; break; }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        super::terminate_process_group(parent.id());
        let _ = parent.wait();
        assert!(found, "SSH child should be detected without starting another process");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_ping_reaches_loopback() {
        assert!(super::ping_host("127.0.0.1".into(), 2).await.unwrap().success);
    }

    #[cfg(windows)]
    #[test]
    fn tray_language_changes_preserve_server_settings() {
        let dir = std::env::temp_dir().join(format!("tunnel-tray-lang-test-{}-{}", std::process::id(), rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app_settings.json");
        std::fs::write(&path, r#"{"language":"zh","server":{"accessToken":"test-only","port":6789},"other":"keep"}"#).unwrap();
        let config = ServerConfig { listen: "127.0.0.1".parse().unwrap(), port: 6789,
            data_dir: dir.clone(), auth_mode: AuthMode::Token, access_token: Some("test-only".into()) };
        let service = CommandService::new(&config).unwrap();
        assert_eq!(service.tray_language().unwrap(), "zh");
        service.set_tray_language("en").unwrap();
        assert_eq!(service.tray_language().unwrap(), "en");
        let saved = crate::config::read_settings(&path).unwrap();
        assert_eq!(saved["server"]["accessToken"], "test-only");
        assert_eq!(saved["server"]["port"], 6789);
        assert_eq!(saved["other"], "keep");
        assert!(service.set_tray_language("invalid").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_private_key_acl_restricts_access() {
        use std::os::windows::process::CommandExt;
        let path = std::env::temp_dir().join(format!("tunnel-helper-acl-test-{}-{}", std::process::id(), rand::random::<u64>()));
        std::fs::write(&path, b"test").unwrap();
        let result = (|| {
            super::secure_private_key(&path)?;
            let script = r#"$ErrorActionPreference='Stop'; $sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value; $acl=[System.IO.File]::GetAccessControl($env:TUNNEL_HELPER_ACL_PATH); if (-not $acl.AreAccessRulesProtected) { exit 1 }; $rules=@($acl.GetAccessRules($true,$true,[System.Security.Principal.SecurityIdentifier])); if ($rules.Count -ne 1 -or $rules[0].IdentityReference.Value -ne $sid -or $rules[0].AccessControlType -ne 'Allow') { exit 2 }"#;
            let status = std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("TUNNEL_HELPER_ACL_PATH", &path)
                .creation_flags(0x08000000).status().map_err(|e| e.to_string())?;
            if status.success() { Ok(()) } else { Err(format!("ACL verification failed: {status}")) }
        })();
        let _ = std::fs::remove_file(path);
        result.unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_askpass_preserves_batch_metacharacters() {
        use std::os::windows::process::CommandExt;
        for secret in ["simple password", "a&b|c<d>e^f%g!h", "parentheses()"] {
            let path = super::create_askpass_script(secret).unwrap();
            let check = r#"$acl=[System.IO.File]::GetAccessControl($env:TUNNEL_HELPER_ACL_PATH); $sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value; $r=@($acl.GetAccessRules($true,$true,[System.Security.Principal.SecurityIdentifier])); if (!$acl.AreAccessRulesProtected -or $r.Count -ne 1 -or $r[0].IdentityReference.Value -ne $sid) { exit 1 }"#;
            let acl_status = std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", check])
                .env("TUNNEL_HELPER_ACL_PATH", &path)
                .creation_flags(0x08000000).status().unwrap();
            let output = std::process::Command::new("cmd.exe")
                .args(["/d", "/v:off", "/c"]).arg(&path)
                .creation_flags(0x08000000).output().unwrap();
            let _ = std::fs::remove_file(path);
            assert!(acl_status.success());
            assert!(output.status.success());
            assert_eq!(String::from_utf8(output.stdout).unwrap().trim_end_matches(['\r', '\n']), secret);
        }
        assert!(super::create_askpass_script("bad\nsecret").is_err());
        assert!(super::create_askpass_script("密码").is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "requires an isolated test SSH server and test key"]
    async fn windows_askpass_authenticates_with_test_server() {
        let identity = std::env::var("TUNNEL_TEST_IDENTITY").unwrap();
        let host = std::env::var("TUNNEL_TEST_HOST").unwrap();
        let port = std::env::var("TUNNEL_TEST_PORT").unwrap();
        let user = std::env::var("TUNNEL_TEST_USER").unwrap();
        let known_hosts = std::env::var("TUNNEL_TEST_KNOWN_HOSTS").unwrap();
        let good = super::create_askpass_script("test-Askpass_42!").unwrap();
        let bad = super::create_askpass_script("wrong-test-passphrase").unwrap();
        let result = async {
            assert!(super::private_key_unlocks(&identity, &good).await.unwrap());
            assert!(!super::private_key_unlocks(&identity, &bad).await.unwrap());
            let mut ssh = tokio::process::Command::new("ssh");
            ssh.args(["-T", "-p", &port, "-o", "ConnectTimeout=8", "-o", "StrictHostKeyChecking=accept-new", "-o", &format!("UserKnownHostsFile={known_hosts}"), "-o", "IdentitiesOnly=yes", "-i", &identity, &format!("{user}@{host}"), "exit"])
                .stdin(std::process::Stdio::null());
            super::configure_askpass(&mut ssh, Some(&good));
            let output = ssh.output().await.unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        }.await;
        let _ = std::fs::remove_file(good);
        let _ = std::fs::remove_file(bad);
        result
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "requires an authorized SSH test server"]
    async fn windows_single_tunnel_integration() {
        let host = std::env::var("TUNNEL_TEST_HOST").unwrap();
        let identity = std::env::var("TUNNEL_TEST_IDENTITY").unwrap();
        let user = std::env::var("TUNNEL_TEST_USER").unwrap();
        let guard = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = guard.local_addr().unwrap().port();
        let dir = std::env::temp_dir().join(format!("tunnel-helper-integration-{}-{}", std::process::id(), rand::random::<u64>()));
        let config = ServerConfig { listen: "127.0.0.1".parse().unwrap(), port: 6789,
            data_dir: dir.clone(), auth_mode: AuthMode::Token, access_token: None };
        let service = CommandService::new(&config).unwrap();
        let request = || super::TunnelRequest { user: user.clone(), host: host.clone(), server_port: 22,
            local_port: port, identity_file: Some(identity.clone()), identity_passphrase: None,
            reconnect: false, reconnect_delay: None, reverse_tunnel: false, reverse_target_port: 22 };
        assert!(service.start_key_tunnel(request()).await.is_err(), "occupied port must be rejected");
        drop(guard);
        let started = service.start_key_tunnel(request()).await.unwrap();
        let mut connected = false;
        for _ in 0..30 {
            let status = service.check_tunnel_status(started.pid).await.unwrap();
            if status.connected { connected = true; break; }
            if !status.running { break; }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        let listed = service.list_connections().await.unwrap();
        let _ = service.stop_tunnel(started.pid).await;
        let released = super::is_local_port_available(port);
        let _ = std::fs::remove_dir_all(dir);
        assert!(connected, "SSH listener was not confirmed");
        assert!(listed.iter().any(|item| item.pid == started.pid && item.connected));
        assert!(released, "port should be released after stopping");
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "requires an authorized SSH test server"]
    async fn windows_reconnects_after_child_exit() {
        use std::os::windows::process::CommandExt;
        let host = std::env::var("TUNNEL_TEST_HOST").unwrap();
        let identity = std::env::var("TUNNEL_TEST_IDENTITY").unwrap();
        let user = std::env::var("TUNNEL_TEST_USER").unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let dir = std::env::temp_dir().join(format!("tunnel-helper-reconnect-test-{}-{}", std::process::id(), rand::random::<u64>()));
        let config = ServerConfig { listen: "127.0.0.1".parse().unwrap(), port: 6789,
            data_dir: dir.clone(), auth_mode: AuthMode::Token, access_token: None };
        let service = CommandService::new(&config).unwrap();
        let request = super::TunnelRequest { user, host, server_port: 22, local_port: port,
            identity_file: Some(identity), identity_passphrase: None, reconnect: true,
            reconnect_delay: Some(1), reverse_tunnel: false, reverse_target_port: 22 };
        let started = service.start_key_tunnel(request).await.unwrap();
        let mut first = None;
        for _ in 0..30 {
            let children = super::windows_ssh_children(started.pid);
            if service.check_tunnel_status(started.pid).await.unwrap().connected {
                first = children.first().copied();
                if first.is_some() { break; }
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        let mut reconnected = false;
        if let Some(first_pid) = first {
            let _ = std::process::Command::new("taskkill.exe").args(["/PID", &first_pid.to_string(), "/F"])
                .creation_flags(0x08000000).stdout(std::process::Stdio::null()).status();
            for _ in 0..30 {
                let children = super::windows_ssh_children(started.pid);
                if children.iter().any(|pid| *pid != first_pid) && service.check_tunnel_status(started.pid).await.unwrap().connected {
                    reconnected = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }
        let _ = service.stop_tunnel(started.pid).await;
        let released = super::is_local_port_available(port);
        let _ = std::fs::remove_dir_all(dir);
        assert!(first.is_some(), "initial SSH listener was not confirmed");
        assert!(reconnected, "replacement SSH listener was not confirmed");
        assert!(released, "port should be released after stopping");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_tunnel_refuses_unsupported_start_and_removes_askpass() {
        let dir = std::env::temp_dir().join(format!("tunnel-helper-unsupported-{}-{}", std::process::id(), rand::random::<u64>()));
        let config = ServerConfig { listen: "127.0.0.1".parse().unwrap(), port: 6789,
            data_dir: dir.clone(), auth_mode: AuthMode::Token, access_token: None };
        let service = CommandService::new(&config).unwrap();
        let path = super::create_askpass_script("test-secret").unwrap();
        let connection = super::ConnectionInfo { pid: 0, user: "test".into(), host: "localhost".into(),
            server_port: 22, local_port: 2222, auth_mode: "key", reconnect: true, reverse_tunnel: true,
            reverse_target_port: 22, identity_file: None, command: String::new(), running: true, connected: false };
        let result = service.spawn_tunnel(vec![], Some(path.clone()), true, Some(1), connection).await;
        assert!(result.is_err());
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn connection_list_is_shared_and_does_not_expose_secrets() {
        use std::os::windows::process::CommandExt;
        let dir = std::env::temp_dir().join(format!("tunnel-helper-registry-test-{}-{}", std::process::id(), rand::random::<u64>()));
        let config = ServerConfig { listen: "127.0.0.1".parse().unwrap(), port: 6789,
            data_dir: dir.clone(), auth_mode: AuthMode::Token, access_token: None };
        let service = CommandService::new(&config).unwrap();
        let mut command = tokio::process::Command::new("cmd.exe");
        command.args(["/c", "ping", "-t", "127.0.0.1"])
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        command.as_std_mut().creation_flags(0x08000000);
        let child = command.spawn().unwrap();
        let pid = child.id().unwrap();
        let connection = super::ConnectionInfo { pid, user: "test".into(), host: "example.invalid".into(),
            server_port: 22, local_port: 2222, auth_mode: "password", reconnect: false, reverse_tunnel: false,
            reverse_target_port: 22, identity_file: None, command: "ssh example.invalid".into(), running: true, connected: false };
        service.tunnels.lock().await.insert(pid, super::ManagedTunnel { child, askpass_path: None,
            control_path: dir.join("missing.sock"), connection });
        let other_client = service.clone();
        let listed = other_client.invoke("list_connections", serde_json::json!({})).await.unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(listed[0]["pid"], pid);
        assert!(listed[0].get("password").is_none());
        assert!(listed[0].get("identityPassphrase").is_none());
        other_client.stop_tunnel(pid).await.unwrap();
        assert!(service.list_connections().await.unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_connection_list_is_shared_and_updates_after_stop() {
        let dir = std::env::temp_dir().join(format!("tunnel-helper-registry-test-{}-{}", std::process::id(), rand::random::<u64>()));
        let config = ServerConfig { listen: "127.0.0.1".parse().unwrap(), port: 6789,
            data_dir: dir.clone(), auth_mode: AuthMode::Token, access_token: None };
        let service = CommandService::new(&config).unwrap();
        let mut command = tokio::process::Command::new("sleep");
        command.arg("60").kill_on_drop(true);
        super::configure_process_group(&mut command);
        let child = command.spawn().unwrap();
        let pid = child.id().unwrap();
        let connection = super::ConnectionInfo { pid, user: "test".into(), host: "example.invalid".into(),
            server_port: 22, local_port: 2222, auth_mode: "key", reconnect: false, reverse_tunnel: false,
            reverse_target_port: 22, identity_file: None, command: "ssh example.invalid".into(), running: true, connected: false };
        service.tunnels.lock().await.insert(pid, super::ManagedTunnel { child, askpass_path: None,
            control_path: dir.join("missing.sock"), connection });
        let other_client = service.clone();
        let listed = other_client.invoke("list_connections", serde_json::json!({})).await.unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(listed[0]["pid"], pid);
        assert!(listed[0].get("password").is_none());
        assert!(listed[0].get("identityPassphrase").is_none());
        other_client.stop_tunnel(pid).await.unwrap();
        assert!(service.list_connections().await.unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_terminates_owned_process_tree() {
        use std::os::windows::process::CommandExt;
        let mut child = std::process::Command::new("cmd.exe")
            .args(["/c", "ping", "-t", "127.0.0.1"])
            .creation_flags(0x08000000)
            .spawn().unwrap();
        super::terminate_process_group(child.id());
        assert!(child.wait().is_ok());
    }

    #[test]
    fn local_tunnel_keeps_lan_listener() {
        let args = tunnel_args("user", "server-a", 22, 2222, false, 22);
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-L", "0.0.0.0:2222:127.0.0.1:22"]));
        assert!(!args.iter().any(|value| value == "-R"));
    }

    #[test]
    fn reverse_tunnel_binds_only_on_jump_host_loopback() {
        let args = tunnel_args("user", "server-a", 22, 22022, true, 2222);
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-R", "127.0.0.1:22022:127.0.0.1:2222"]));
        assert!(!args.iter().any(|value| value == "-g"));
    }

    #[test]
    fn reverse_config_uses_jump_host_and_remote_port() {
        let config = generate_reverse_proxy_config(GenerateReverseConfigArgs {
            alias: "server-b".to_string(),
            jump_host: "server-a.example".to_string(),
            jump_port: 22,
            jump_user: "jump-user".to_string(),
            remote_port: 22022,
            target_user: "server-b-user".to_string(),
        });
        assert!(config.contains("Host server-b-jump"));
        assert!(config.contains("Port 22022"));
        assert!(config.contains("ProxyJump server-b-jump"));
        assert!(config.contains("User server-b-user"));
        assert!(config.contains("User jump-user\n    # Set the relay server private key path on this computer:\n    # IdentityFile ~/.ssh/id_ed25519_jump-user\n    # IdentitiesOnly yes"));
        assert!(config.contains("User server-b-user\n    # Set the target host private key path on this computer:\n    # IdentityFile ~/.ssh/id_ed25519_server-b-user\n    # IdentitiesOnly yes"));
    }

    #[test]
    fn ui_settings_save_preserves_server_secrets_without_exposing_them() {
        let dir = std::env::temp_dir().join(format!(
            "tunnel-helper-settings-test-{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app_settings.json");
        std::fs::write(&path, r#"{"language":"zh","server":{"listen":"127.0.0.1","tokenEnabled":true,"accessToken":"test-token"},"extra":"preserved"}"#).unwrap();
        let config = ServerConfig {
            listen: "127.0.0.1".parse().unwrap(),
            port: 6789,
            data_dir: dir.clone(),
            auth_mode: AuthMode::Token,
            access_token: Some("test-token".to_string()),
        };
        let service = CommandService::new(&config).unwrap();
        let mut settings: AppSettings = service.load_app_settings().unwrap();
        assert!(!serde_json::to_string(&settings)
            .unwrap()
            .contains("test-token"));
        settings.language = "en".to_string();
        service.save_app_settings(&settings).unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(saved["language"], "en");
        assert_eq!(saved["server"]["accessToken"], "test-token");
        assert_eq!(saved["extra"], "preserved");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
