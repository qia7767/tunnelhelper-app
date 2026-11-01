use crate::{
    config::ServerConfig,
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
}

#[derive(Clone)]
pub struct CommandService {
    paths: Arc<AppPaths>,
    tunnels: Arc<Mutex<HashMap<u32, ManagedTunnel>>>,
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
        write_json(&self.paths.settings, settings)
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
        self.spawn_tunnel(args, askpass, request.reconnect, request.reconnect_delay)
            .await
    }

    async fn start_password_tunnel(
        &self,
        request: PasswordTunnelRequest,
    ) -> Result<TunnelStartResult, String> {
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
            ],
        )?;
        self.spawn_tunnel(args, askpass, request.reconnect, request.reconnect_delay)
            .await
    }

    async fn spawn_tunnel(
        &self,
        mut args: Vec<String>,
        askpass_path: Option<PathBuf>,
        reconnect: bool,
        reconnect_delay: Option<u64>,
    ) -> Result<TunnelStartResult, String> {
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
        self.tunnels.lock().await.insert(
            pid,
            ManagedTunnel {
                child,
                askpass_path,
                control_path: control_path.clone(),
            },
        );
        Ok(TunnelStartResult {
            pid,
            command: command_text,
            connected_marker: Some(control_path.to_string_lossy().to_string()),
        })
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
                connected: tunnel.control_path.exists(),
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
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| error.to_string())?;
    secure_private_file(path)
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
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

#[cfg(not(unix))]
fn secure_private_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn secure_private_key(path: &Path) -> Result<(), String> {
    secure_private_file(path)
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
    for port in start..=u16::MAX {
        if is_local_port_available(port) {
            return Ok(port);
        }
    }
    Err(format!("No free port found from {start}"))
}

fn is_local_port_available(port: u16) -> bool {
    TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port)).is_ok()
}

fn is_local_port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        Duration::from_millis(350),
    )
    .is_ok()
}

async fn ping_host(host: String, timeout_sec: u16) -> Result<CommandResult, String> {
    let output = TokioCommand::new("ping")
        .args(["-c", "1", "-W", &timeout_sec.max(1).to_string(), &host])
        .output()
        .await
        .map_err(|error| format!("Cannot run ping: {error}"))?;
    Ok(CommandResult {
        success: output.status.success(),
        message: if output.status.success() {
            format!("ping -c 1 {host}: success")
        } else {
            format!("ping -c 1 {host}: failed or timed out")
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
        if !private_key_unlocks(identity, askpass_path).await? {
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
    args.push(
        "umask 077; mkdir -p ~/.ssh; touch ~/.ssh/authorized_keys; cat | tr -d '\\r' >> ~/.ssh/authorized_keys; awk 'NF && !seen[$0]++' ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp && mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys; chmod 700 ~/.ssh; chmod 600 ~/.ssh/authorized_keys"
            .to_string(),
    );

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

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
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

#[cfg(not(unix))]
fn configure_process_group(_command: &mut TokioCommand) {}

#[cfg(unix)]
fn terminate_process_group(pid: u32) {
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    }
}

#[cfg(not(unix))]
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
    use super::{generate_reverse_proxy_config, tunnel_args, GenerateReverseConfigArgs};

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
}
