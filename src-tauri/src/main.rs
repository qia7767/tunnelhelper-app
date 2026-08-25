#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Default)]
struct AppState {
    tunnels: Mutex<HashMap<u32, ManagedTunnel>>,
}

struct ManagedTunnel {
    child: Child,
    askpass_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct KeyBundle {
    name: String,
    private_path: String,
    public_path: Option<String>,
    fingerprint: Option<String>,
    source_dir: String,
}

#[derive(Debug, Serialize)]
struct VmIpCandidate {
    ip: String,
    source: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Profile {
    name: String,
    user: String,
    host: String,
    server_port: u16,
    base_local_port: u16,
    identity_file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity_passphrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_password: Option<String>,
    auth_mode: Option<String>,
    host_alias: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", default)]
struct AppDefaults {
    keygen_user: String,
    keygen_host: String,
    keygen_offset: String,
    install_key_name: String,
    install_user: String,
    install_host: String,
    install_port: u16,
}

impl Default for AppDefaults {
    fn default() -> Self {
        Self {
            keygen_user: String::new(),
            keygen_host: String::new(),
            keygen_offset: String::new(),
            install_key_name: String::new(),
            install_user: String::new(),
            install_host: String::new(),
            install_port: 22,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", default)]
struct AppSettings {
    language: String,
    cursor: String,
    #[serde(default)]
    defaults: AppDefaults,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "zh".to_string(),
            cursor: "NArrowBeadCursor".to_string(),
            defaults: AppDefaults::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TunnelRequest {
    user: String,
    host: String,
    server_port: u16,
    local_port: u16,
    identity_file: Option<String>,
    identity_passphrase: Option<String>,
    reconnect: bool,
    reconnect_delay: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PasswordTunnelRequest {
    user: String,
    host: String,
    server_port: u16,
    local_port: u16,
    password: Option<String>,
    reconnect: bool,
    reconnect_delay: Option<u64>,
}

#[derive(Debug, Serialize)]
struct TunnelStartResult {
    pid: u32,
    command: String,
    connected_marker: Option<String>,
}

#[derive(Debug, Serialize)]
struct CommandResult {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct TunnelStatus {
    pid: u32,
    running: bool,
    exit_code: Option<i32>,
}

fn app_base_dir() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| e.to_string())?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Cannot resolve executable directory".to_string())
}

fn workspace_base_dir() -> Result<PathBuf, String> {
    env::current_dir().map_err(|e| e.to_string())
}

fn user_ssh_dir() -> Result<PathBuf, String> {
    let home = env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| "USERPROFILE is not set".to_string())?;
    Ok(home.join(".ssh"))
}

fn portable_key_dir() -> Result<PathBuf, String> {
    let base = workspace_base_dir()?;
    Ok(base.join("ssh_keyfiles_export"))
}

fn profiles_path() -> Result<PathBuf, String> {
    Ok(workspace_base_dir()?.join("profiles.json"))
}

fn app_settings_path() -> Result<PathBuf, String> {
    Ok(workspace_base_dir()?.join("app_settings.json"))
}

fn candidate_key_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(base) = app_base_dir() {
        dirs.push(base.join("ssh_keyfiles_export"));
        if let Some(parent) = base.parent() {
            dirs.push(parent.join("ssh_keyfiles_export"));
        }
    }

    if let Ok(base) = workspace_base_dir() {
        dirs.push(base.join("ssh_keyfiles_export"));
        if let Some(parent) = base.parent() {
            dirs.push(parent.join("ssh_keyfiles_export"));
        }
    }

    if let Some(home) = env::var_os("USERPROFILE").map(PathBuf::from) {
        dirs.push(home.join("Downloads").join("ssh_keyfiles_export"));
        dirs.push(home.join("Desktop").join("ssh_keyfiles_export"));
        dirs.push(home.join(".ssh"));
    }

    dirs
}

fn normalize_path(path: &Path) -> String {
    let text = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();

    if let Some(stripped) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", stripped)
    } else if let Some(stripped) = text.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        text
    }
}

fn looks_like_private_key(path: &Path) -> bool {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pub"))
    {
        return false;
    }

    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    if matches!(name, "config" | "known_hosts" | "authorized_keys") {
        return false;
    }

    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };

    text.lines()
        .take(5)
        .any(|line| line.contains("BEGIN") && line.contains("PRIVATE KEY"))
}

fn fingerprint_for(public_key: &Path) -> Option<String> {
    let mut command = Command::new("ssh-keygen");
    let output = hide_console(
        command
            .arg("-lf")
            .arg(public_key)
            .stdout(Stdio::piped())
            .stderr(Stdio::null()),
    )
    .output()
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn key_bundle_from_private(path: &Path, source_dir: &Path) -> KeyBundle {
    let public_path = path.with_file_name(format!(
        "{}.pub",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
    ));
    let public = public_path.exists().then_some(public_path);

    KeyBundle {
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
        private_path: normalize_path(path),
        public_path: public.as_ref().map(|p| normalize_path(p)),
        fingerprint: public.as_ref().and_then(|p| fingerprint_for(p)),
        source_dir: source_dir.to_string_lossy().to_string(),
    }
}

fn same_file_contents(left: &Path, right: &Path) -> bool {
    match (fs::read(left), fs::read(right)) {
        (Ok(left_bytes), Ok(right_bytes)) => left_bytes == right_bytes,
        _ => false,
    }
}

fn copy_file_if_needed(source: &Path, destination: &Path) -> Result<(), String> {
    let source_resolved = source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf());
    let destination_resolved = destination
        .canonicalize()
        .unwrap_or_else(|_| destination.to_path_buf());

    if source_resolved == destination_resolved {
        return Ok(());
    }

    if destination.exists() && same_file_contents(source, destination) {
        return Ok(());
    }

    fs::copy(source, destination).map(|_| ()).map_err(|e| {
        format!(
            "复制失败: {} -> {}: {}",
            normalize_path(source),
            normalize_path(destination),
            e
        )
    })
}

fn copy_key_pair(private_path: &Path, target_dir: &Path) -> Result<KeyBundle, String> {
    if !looks_like_private_key(private_path) {
        return Err(format!(
            "Not a private key file: {}",
            private_path.to_string_lossy()
        ));
    }

    fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
    let name = private_path
        .file_name()
        .ok_or_else(|| "Missing private key filename".to_string())?;
    let dst_private = target_dir.join(name);
    copy_file_if_needed(private_path, &dst_private)?;

    let public = private_path.with_file_name(format!("{}.pub", name.to_string_lossy()));
    if public.exists() {
        let pub_name = public
            .file_name()
            .ok_or_else(|| "Missing public key filename".to_string())?;
        let dst_public = target_dir.join(pub_name);
        copy_file_if_needed(&public, &dst_public)?;
    }

    fix_private_key_acl(&dst_private);
    Ok(key_bundle_from_private(&dst_private, target_dir))
}

fn fix_private_key_acl(path: &Path) {
    let mut command = Command::new("icacls");
    let _ = hide_console(
        command
            .arg(path)
            .args(["/inheritance:r"])
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .status();

    if let Some(user) = env::var_os("USERNAME").and_then(|v| v.into_string().ok()) {
        let grant = format!("{user}:R");
        let mut command = Command::new("icacls");
        let _ = hide_console(
            command
                .arg(path)
                .args(["/grant:r", &grant])
                .stdout(Stdio::null())
                .stderr(Stdio::null()),
        )
        .status();
    }
}

fn sanitize_key_name(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = cleaned.trim_matches('_');

    if trimmed.is_empty() {
        "id_ed25519_tunnel".to_string()
    } else if trimmed.starts_with("id_") {
        trimmed.to_string()
    } else {
        format!("id_ed25519_{trimmed}")
    }
}

#[tauri::command]
fn generate_ssh_key(
    name: String,
    comment: Option<String>,
    passphrase: Option<String>,
    overwrite: Option<bool>,
) -> Result<KeyBundle, String> {
    let target_dir = portable_key_dir()?;
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    let filename = sanitize_key_name(&name);
    let private_path = target_dir.join(filename);
    let public_path = private_path.with_file_name(format!(
        "{}.pub",
        private_path
            .file_name()
            .and_then(|item| item.to_str())
            .unwrap_or("id_ed25519_tunnel")
    ));

    if private_path.exists() || public_path.exists() {
        if overwrite.unwrap_or(false) {
            let _ = fs::remove_file(&private_path);
            let _ = fs::remove_file(&public_path);
        } else {
            return Err(format!("密钥已存在: {}", normalize_path(&private_path)));
        }
    }

    let mut command = Command::new("ssh-keygen.exe");
    let output = hide_console(
        command
            .arg("-t")
            .arg("ed25519")
            .arg("-N")
            .arg(passphrase.unwrap_or_default())
            .arg("-f")
            .arg(&private_path)
            .arg("-C")
            .arg(comment.unwrap_or_else(|| "tunnel-helper".to_string()))
            .arg("-q")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .output()
    .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "ssh-keygen failed".to_string()
        } else {
            stderr
        });
    }

    fix_private_key_acl(&private_path);
    Ok(key_bundle_from_private(&private_path, &target_dir))
}

fn is_public_key_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("ssh-rsa ")
        || trimmed.starts_with("ssh-ed25519 ")
        || trimmed.starts_with("ecdsa-sha2-")
        || trimmed.starts_with("sk-ssh-")
        || trimmed.starts_with("sk-ecdsa-")
}

fn public_key_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}').trim())
        .filter(|line| is_public_key_line(line))
        .map(ToString::to_string)
        .collect()
}

fn authorized_keys_install_command() -> &'static str {
    "umask 077; mkdir -p ~/.ssh; touch ~/.ssh/authorized_keys; cat | tr -d '\\r' >> ~/.ssh/authorized_keys; awk 'NF && !seen[$0]++' ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp && mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys; chmod 700 ~/.ssh; chmod 600 ~/.ssh/authorized_keys; echo '[OK] authorized_keys updated.'"
}

fn escape_single_quoted_powershell(value: &str) -> String {
    value.replace('\'', "''")
}

fn quote_powershell_arg(value: &str) -> String {
    format!("'{}'", escape_single_quoted_powershell(value))
}

fn escape_batch_echo(value: &str) -> String {
    value
        .replace('%', "%%")
        .replace('^', "^^")
        .replace('&', "^&")
        .replace('|', "^|")
        .replace('<', "^<")
        .replace('>', "^>")
}

fn create_askpass_script(passphrase: &str, local_port: u16) -> Result<PathBuf, String> {
    let path = env::temp_dir().join(format!(
        "tunnel_helper_askpass_{}_{}.cmd",
        local_port,
        std::process::id()
    ));
    let script = format!("@echo off\r\necho({}\r\n", escape_batch_echo(passphrase));
    fs::write(&path, script).map_err(|e| e.to_string())?;
    Ok(path)
}

fn cleanup_askpass(path: Option<PathBuf>) {
    if let Some(path) = path {
        let _ = fs::remove_file(path);
    }
}

fn hide_console(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

#[tauri::command]
fn scan_key_bundles() -> Result<Vec<KeyBundle>, String> {
    let mut bundles = Vec::new();
    let mut seen = HashMap::<String, bool>::new();

    for dir in candidate_key_dirs() {
        if !dir.exists() {
            continue;
        }

        let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !looks_like_private_key(&path) {
                continue;
            }

            let canonical = path.canonicalize().unwrap_or(path.clone());
            let key = canonical.to_string_lossy().to_string();
            if seen.insert(key, true).is_some() {
                continue;
            }

            bundles.push(key_bundle_from_private(&canonical, &dir));
        }
    }

    bundles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(bundles)
}

#[tauri::command]
fn load_profiles() -> Result<Vec<Profile>, String> {
    let path = profiles_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_profiles(profiles: Vec<Profile>) -> Result<(), String> {
    let path = profiles_path()?;
    let text = serde_json::to_string_pretty(&profiles).map_err(|e| e.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_app_settings() -> Result<AppSettings, String> {
    let path = app_settings_path()?;
    if !path.exists() {
        let settings = AppSettings::default();
        let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        fs::write(&path, format!("{text}\n")).map_err(|e| e.to_string())?;
        return Ok(settings);
    }

    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

#[tauri::command]
fn save_app_settings(settings: AppSettings) -> Result<(), String> {
    let path = app_settings_path()?;
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_key_files(private_paths: Vec<String>) -> Result<Vec<KeyBundle>, String> {
    let target = portable_key_dir()?;
    let mut exported = Vec::new();

    for raw in private_paths {
        let path = PathBuf::from(raw);
        exported.push(copy_key_pair(&path, &target)?);
    }

    Ok(exported)
}

#[tauri::command]
fn import_key_files(private_paths: Vec<String>) -> Result<Vec<KeyBundle>, String> {
    let target = user_ssh_dir()?;
    let mut imported = Vec::new();

    for raw in private_paths {
        let path = PathBuf::from(raw);
        imported.push(copy_key_pair(&path, &target)?);
    }

    Ok(imported)
}

#[tauri::command]
fn delete_key_files(private_paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut deleted = Vec::new();

    for raw in private_paths {
        let private_path = PathBuf::from(&raw);
        if !private_path.is_file() {
            return Err(format!("密钥文件不存在: {}", normalize_path(&private_path)));
        }
        if !looks_like_private_key(&private_path) {
            return Err(format!(
                "不是可删除的私钥文件: {}",
                normalize_path(&private_path)
            ));
        }

        let public_path = private_path.with_file_name(format!(
            "{}.pub",
            private_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        ));

        fs::remove_file(&private_path)
            .map_err(|e| format!("删除失败: {}: {}", normalize_path(&private_path), e))?;
        deleted.push(normalize_path(&private_path));

        if public_path.is_file() {
            fs::remove_file(&public_path)
                .map_err(|e| format!("删除失败: {}: {}", normalize_path(&public_path), e))?;
            deleted.push(normalize_path(&public_path));
        }
    }

    Ok(deleted)
}

#[tauri::command]
fn export_public_bundle(public_paths: Vec<String>) -> Result<String, String> {
    let target = workspace_base_dir()?.join("ssh_public_keys_bundle.pub");
    let mut lines = Vec::new();

    for raw in public_paths {
        let text = fs::read_to_string(&raw).map_err(|e| format!("{raw}: {e}"))?;
        for line in text
            .lines()
            .map(str::trim)
            .filter(|line| is_public_key_line(line))
        {
            if !lines.iter().any(|existing| existing == line) {
                lines.push(line.to_string());
            }
        }
    }

    if lines.is_empty() {
        return Err("No valid public key lines selected".to_string());
    }

    fs::write(&target, format!("{}\n", lines.join("\n"))).map_err(|e| e.to_string())?;
    Ok(normalize_path(&target))
}

fn install_public_to_linux_blocking(
    target: String,
    port: u16,
    public_key_path: String,
    identity_file: Option<String>,
) -> Result<CommandResult, String> {
    let key_text = fs::read_to_string(&public_key_path).map_err(|e| e.to_string())?;
    let valid = public_key_lines(&key_text).join("\n");

    if valid.is_empty() {
        return Err("No valid public key lines found".to_string());
    }

    let remote_cmd = authorized_keys_install_command();
    let mut args = vec![
        "-p".to_string(),
        port.to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "NumberOfPasswordPrompts=0".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "PreferredAuthentications=publickey".to_string(),
    ];

    if let Some(identity) = identity_file.filter(|value| !value.trim().is_empty()) {
        args.push("-i".to_string());
        args.push(identity);
        args.push("-o".to_string());
        args.push("IdentitiesOnly=yes".to_string());
    }

    args.push(target);
    args.push(remote_cmd.to_string());

    let mut command = Command::new("ssh.exe");
    let mut child = hide_console(
        command
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .spawn()
    .map_err(|e| e.to_string())?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(format!("{valid}\n").as_bytes())
            .map_err(|e| e.to_string())?;
    }
    drop(child.stdin.take());

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }

        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            let mut stdout = String::new();
            let mut stderr = String::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = out.read_to_string(&mut stdout);
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_string(&mut stderr);
            }
            return Ok(CommandResult {
                success: false,
                message: format!(
                    "静默安装超时。请确认“服务器已有登录密钥”能无密码登录，再重试。\n命令: ssh.exe {}\n输出: {}{}",
                    args.join(" "),
                    stdout.trim(),
                    stderr.trim()
                ),
            });
        }

        std::thread::sleep(Duration::from_millis(120));
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }

    Ok(CommandResult {
        success: status.success(),
        message: format!(
            "命令: ssh.exe {}\n退出码: {}\n输出: {}{}",
            args.join(" "),
            status
                .code()
                .map_or_else(|| "unknown".to_string(), |code| code.to_string()),
            stdout.trim(),
            stderr.trim()
        ),
    })
}

#[tauri::command]
async fn install_public_to_linux(
    target: String,
    port: u16,
    public_key_path: String,
    identity_file: Option<String>,
) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_public_to_linux_blocking(target, port, public_key_path, identity_file)
    })
    .await
    .map_err(|e| format!("静默安装任务失败: {e}"))?
}

fn install_public_to_linux_password_blocking(
    target: String,
    port: u16,
    public_key_path: String,
    password: String,
) -> Result<CommandResult, String> {
    let key_text = fs::read_to_string(&public_key_path).map_err(|e| e.to_string())?;
    let valid = public_key_lines(&key_text).join("\n");

    if valid.is_empty() {
        return Err("No valid public key lines found".to_string());
    }

    let askpass_path = (!password.is_empty())
        .then(|| create_askpass_script(&password, port))
        .transpose()?;
    let remote_cmd = authorized_keys_install_command();
    let args = vec![
        "-p".to_string(),
        port.to_string(),
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "PreferredAuthentications=password,keyboard-interactive".to_string(),
        "-o".to_string(),
        "PubkeyAuthentication=no".to_string(),
        target,
        remote_cmd.to_string(),
    ];

    let mut command = Command::new("ssh.exe");
    if let Some(path) = askpass_path.as_ref() {
        command.env("SSH_ASKPASS", path);
        command.env("SSH_ASKPASS_REQUIRE", "force");
        command.env("DISPLAY", "tunnel-helper");
    }
    let mut child = hide_console(
        command
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .spawn()
    .map_err(|e| e.to_string())?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(format!("{valid}\n").as_bytes())
            .map_err(|e| e.to_string())?;
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    cleanup_askpass(askpass_path);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(CommandResult {
        success: output.status.success(),
        message: format!(
            "命令: ssh.exe {}\n退出码: {}\n输出: {}{}",
            args.join(" "),
            output
                .status
                .code()
                .map_or_else(|| "unknown".to_string(), |code| code.to_string()),
            stdout.trim(),
            stderr.trim()
        ),
    })
}

#[tauri::command]
async fn install_public_to_linux_password(
    target: String,
    port: u16,
    public_key_path: String,
    password: String,
) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_public_to_linux_password_blocking(target, port, public_key_path, password)
    })
    .await
    .map_err(|e| format!("密码安装任务失败: {e}"))?
}

#[tauri::command]
fn launch_public_install_terminal(
    target: String,
    port: u16,
    public_key_path: String,
    identity_file: Option<String>,
) -> Result<String, String> {
    if !Path::new(&public_key_path).exists() {
        return Err(format!("Public key file not found: {public_key_path}"));
    }

    let remote_cmd = authorized_keys_install_command();
    let script_path = env::temp_dir().join("tunnel_helper_install_public_key.ps1");
    let mut script = String::new();
    script.push_str("$ErrorActionPreference = 'Stop'\n");
    script.push_str("chcp 65001 > $null\n");
    script.push_str("[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()\n");
    script.push_str(&format!(
        "$publicKeyPath = '{}'\n",
        escape_single_quoted_powershell(&public_key_path)
    ));
    script.push_str("$rawKeyText = Get-Content -Raw -LiteralPath $publicKeyPath\n");
    script.push_str("$keyLines = @()\n");
    script.push_str("foreach ($line in ($rawKeyText -split \"`r?`n\")) {\n");
    script.push_str("  $clean = $line.Trim().TrimStart([char]0xFEFF).Trim()\n");
    script.push_str("  if ($clean -match '^(ssh-rsa|ssh-ed25519|ecdsa-sha2-|sk-ssh-|sk-ecdsa-)\\s+') { $keyLines += $clean }\n");
    script.push_str("}\n");
    script.push_str("if ($keyLines.Count -eq 0) { throw 'No valid public key lines found.' }\n");
    script.push_str("$keyText = ($keyLines -join \"`n\") + \"`n\"\n");
    script.push_str(&format!("$sshArgs = @('-p', '{}')\n", port));

    if let Some(identity) = identity_file.filter(|value| !value.trim().is_empty()) {
        script.push_str(&format!(
            "$sshArgs += @('-i', '{}', '-o', 'IdentitiesOnly=yes')\n",
            escape_single_quoted_powershell(&identity)
        ));
    }

    script.push_str(&format!(
        "$sshArgs += @('{}', '{}')\n",
        escape_single_quoted_powershell(&target),
        escape_single_quoted_powershell(remote_cmd)
    ));
    script.push_str(
        "Write-Host 'Installing public key to remote authorized_keys. Server password may be required.' -ForegroundColor Yellow\n",
    );
    script.push_str("$keyText | & ssh.exe @sshArgs\n");
    script.push_str("Write-Host ''\n");
    script.push_str("Read-Host 'Done. Press Enter to close this window'\n");

    fs::write(&script_path, script).map_err(|e| e.to_string())?;
    let script_path_text = script_path.to_string_lossy().to_string();
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path_text,
        ])
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(normalize_path(&script_path))
}

#[tauri::command]
fn find_free_port(start_port: u16) -> Result<u16, String> {
    for port in start_port..=65535 {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
        if TcpListener::bind(addr).is_ok() {
            return Ok(port);
        }
    }

    Err(format!("No free port found from {start_port}"))
}

#[tauri::command]
fn is_local_port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        Duration::from_millis(350),
    )
    .is_ok()
}

#[tauri::command]
fn is_local_port_available(port: u16) -> bool {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
    TcpListener::bind(addr).is_ok()
}

#[tauri::command]
fn ping_host(host: String, timeout_sec: u16) -> Result<CommandResult, String> {
    let timeout_ms = timeout_sec.max(1) * 1000;
    let mut command = Command::new("ping.exe");
    let output = hide_console(
        command
            .args(["-n", "1", "-w", &timeout_ms.to_string(), &host])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .output()
    .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(CommandResult {
        success: output.status.success(),
        message: if output.status.success() {
            format!("ping.exe -n 1 -w {timeout_ms} {host}: success")
        } else {
            format!(
                "ping.exe -n 1 -w {timeout_ms} {host}: failed or timed out. Some servers block ICMP; use SSH test instead. stdout={} bytes stderr={} bytes",
                stdout.len(),
                stderr.len()
            )
        },
    })
}

#[tauri::command]
fn launch_ssh_test_terminal(
    user: String,
    host: String,
    port: u16,
    identity_file: Option<String>,
) -> Result<String, String> {
    let script_path = env::temp_dir().join("tunnel_helper_ssh_test.ps1");
    let mut script = String::new();
    script.push_str("$ErrorActionPreference = 'Continue'\n");
    script.push_str(&format!("$sshArgs = @('-p', '{}')\n", port));
    script.push_str("$sshArgs += @('-o', 'ConnectTimeout=8')\n");
    script.push_str("$sshArgs += @('-o', 'ServerAliveInterval=30')\n");

    if let Some(identity) = identity_file.filter(|value| !value.trim().is_empty()) {
        script.push_str(&format!(
            "$sshArgs += @('-i', '{}', '-o', 'IdentitiesOnly=yes')\n",
            escape_single_quoted_powershell(&identity)
        ));
    }

    script.push_str(&format!(
        "$sshArgs += '{}@{}'\n",
        escape_single_quoted_powershell(&user),
        escape_single_quoted_powershell(&host)
    ));
    script.push_str("Write-Host 'Running SSH connectivity test:' -ForegroundColor Cyan\n");
    script.push_str("Write-Host ('ssh.exe ' + ($sshArgs -join ' ')) -ForegroundColor White\n");
    script.push_str("Write-Host 'If a password prompt appears, the SSH server is reachable.' -ForegroundColor Yellow\n");
    script.push_str("& ssh.exe @sshArgs\n");
    script.push_str("Write-Host ''\n");
    script.push_str("Read-Host 'Test finished. Press Enter to close this window'\n");

    fs::write(&script_path, script).map_err(|e| e.to_string())?;
    let script_path_text = script_path.to_string_lossy().to_string();
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path_text,
        ])
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(normalize_path(&script_path))
}

fn test_ssh_server_blocking(
    user: String,
    host: String,
    port: u16,
    identity_file: Option<String>,
    identity_passphrase: Option<String>,
) -> Result<CommandResult, String> {
    let askpass_path = identity_passphrase
        .as_ref()
        .filter(|value| !value.is_empty())
        .map(|value| create_askpass_script(value, port))
        .transpose()?;

    let address = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("无法解析 SSH 地址 {host}:{port}: {e}"))?
        .next()
        .ok_or_else(|| format!("无法解析 SSH 地址 {host}:{port}"))?;

    TcpStream::connect_timeout(&address, Duration::from_secs(5)).map_err(|e| {
        format!("TCP 连接失败: {host}:{port}: {e}. 请检查网络/VPN、服务器地址、端口或防火墙。")
    })?;

    let mut args = vec![
        "-T".to_string(),
        "-p".to_string(),
        port.to_string(),
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
    ];

    if askpass_path.is_none() {
        args.push("-o".to_string());
        args.push("BatchMode=yes".to_string());
        args.push("-o".to_string());
        args.push("NumberOfPasswordPrompts=0".to_string());
    }

    if let Some(identity) = identity_file.filter(|value| !value.trim().is_empty()) {
        args.push("-i".to_string());
        args.push(identity);
        args.push("-o".to_string());
        args.push("IdentitiesOnly=yes".to_string());
    }

    args.push(format!("{user}@{host}"));
    args.push("exit".to_string());

    let mut command = Command::new("ssh.exe");
    if let Some(path) = askpass_path.as_ref() {
        command.env("SSH_ASKPASS", path);
        command.env("SSH_ASKPASS_REQUIRE", "force");
        command.env("DISPLAY", "tunnel-helper");
    }
    let output_result = hide_console(
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .output();
    cleanup_askpass(askpass_path);
    let output = output_result.map_err(|e| format!("无法运行 ssh.exe: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    let lower = combined.to_lowercase();
    let passphrase_provided = identity_passphrase
        .as_ref()
        .is_some_and(|value| !value.is_empty());
    let passphrase_failed = lower.contains("bad passphrase")
        || lower.contains("incorrect passphrase")
        || lower.contains("passphrase is incorrect")
        || (passphrase_provided
            && (lower.contains("load key")
                || lower.contains("could not load")
                || lower.contains("error loading key")
                || lower.contains("permission denied")));
    let auth_rejected = lower.contains("permission denied")
        || lower.contains("publickey")
        || lower.contains("password")
        || lower.contains("keyboard-interactive");
    let network_failed = lower.contains("connection timed out")
        || lower.contains("connection refused")
        || lower.contains("no route to host")
        || lower.contains("could not resolve")
        || lower.contains("connection reset");

    let success = output.status.success();
    let status = if success {
        "SSH 服务可达，密钥认证通过。"
    } else if passphrase_failed {
        "私钥密码错误或无法解锁私钥；请检查“私钥密码（可选）”。"
    } else if network_failed {
        "SSH 服务未连通；请检查网络/VPN、服务器地址、端口或防火墙。"
    } else if auth_rejected && !network_failed {
        "SSH 服务可达，但密钥认证失败；请确认公钥已经写入对应用户的 authorized_keys。"
    } else {
        "SSH 服务未通过检测。"
    };

    Ok(CommandResult {
        success,
        message: format!(
            "{status}\n命令: ssh.exe {}\n退出码: {}\n输出: {}",
            args.join(" "),
            output
                .status
                .code()
                .map_or_else(|| "unknown".to_string(), |code| code.to_string()),
            combined.trim()
        ),
    })
}

#[tauri::command]
async fn test_ssh_server(
    user: String,
    host: String,
    port: u16,
    identity_file: Option<String>,
    identity_passphrase: Option<String>,
) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        test_ssh_server_blocking(user, host, port, identity_file, identity_passphrase)
    })
    .await
    .map_err(|e| format!("SSH 检测任务失败: {e}"))?
}

fn test_password_ssh_server_blocking(
    user: String,
    host: String,
    port: u16,
    password: String,
) -> Result<CommandResult, String> {
    let askpass_path = (!password.is_empty())
        .then(|| create_askpass_script(&password, port))
        .transpose()?;
    let args = vec![
        "-T".to_string(),
        "-p".to_string(),
        port.to_string(),
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "PreferredAuthentications=password,keyboard-interactive".to_string(),
        "-o".to_string(),
        "PubkeyAuthentication=no".to_string(),
        format!("{user}@{host}"),
        "exit".to_string(),
    ];

    let mut command = Command::new("ssh.exe");
    if let Some(path) = askpass_path.as_ref() {
        command.env("SSH_ASKPASS", path);
        command.env("SSH_ASKPASS_REQUIRE", "force");
        command.env("DISPLAY", "tunnel-helper");
    }
    let output_result = hide_console(
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .output();
    cleanup_askpass(askpass_path);
    let output = output_result.map_err(|e| format!("无法运行 ssh.exe: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    Ok(CommandResult {
        success: output.status.success(),
        message: format!(
            "{}\n命令: ssh.exe {}\n退出码: {}\n输出: {}",
            if output.status.success() {
                "密码认证通过。"
            } else {
                "密码认证失败；请检查服务器密码。"
            },
            args.join(" "),
            output
                .status
                .code()
                .map_or_else(|| "unknown".to_string(), |code| code.to_string()),
            combined.trim()
        ),
    })
}

#[tauri::command]
async fn test_password_ssh_server(
    user: String,
    host: String,
    port: u16,
    password: String,
) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        test_password_ssh_server_blocking(user, host, port, password)
    })
    .await
    .map_err(|e| format!("密码 SSH 检测任务失败: {e}"))?
}

#[tauri::command]
fn ensure_firewall_rule() -> Result<CommandResult, String> {
    let script = "$ruleName='SSH-Tunnel-Range'; $ruleDisplay='SSH Tunnel Range 2222-2299'; $exists=Get-NetFirewallRule -Name $ruleName -ErrorAction SilentlyContinue; if ($exists) { Set-NetFirewallRule -Name $ruleName -Enabled True -Profile Any -Action Allow | Out-Null; 'Firewall rule enabled: '+$ruleName } else { New-NetFirewallRule -Name $ruleName -DisplayName $ruleDisplay -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort (2222..2299) -Profile Any | Out-Null; 'Firewall rule created: '+$ruleName }";
    let mut command = Command::new("powershell");
    let output = hide_console(
        command
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .output()
    .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(CommandResult {
        success: output.status.success(),
        message: format!("{}{}", stdout, stderr).trim().to_string(),
    })
}

#[tauri::command]
fn is_admin() -> Result<bool, String> {
    let script = "([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)";
    let mut command = Command::new("powershell");
    let output = hide_console(
        command
            .args(["-NoProfile", "-Command", script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .output()
    .map_err(|e| e.to_string())?;

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.trim().eq_ignore_ascii_case("true"))
}

#[tauri::command]
fn request_firewall_admin() -> Result<CommandResult, String> {
    let script = "$ruleName='SSH-Tunnel-Range'; $ruleDisplay='SSH Tunnel Range 2222-2299'; $exists=Get-NetFirewallRule -Name $ruleName -ErrorAction SilentlyContinue; if ($exists) { Set-NetFirewallRule -Name $ruleName -Enabled True -Profile Any -Action Allow | Out-Null } else { New-NetFirewallRule -Name $ruleName -DisplayName $ruleDisplay -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort (2222..2299) -Profile Any | Out-Null }";
    let args = format!(
        "-NoProfile -ExecutionPolicy Bypass -Command \"{}\"",
        script.replace('"', "`\"")
    );
    let mut command = Command::new("powershell");
    let status = hide_console(command.arg("-NoProfile").arg("-Command").arg(format!(
        "$p = Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList '{}'; exit $p.ExitCode",
        escape_single_quoted_powershell(&args)
    )))
    .status()
    .map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: status.success(),
        message: "UAC request launched for firewall rule SSH-Tunnel-Range.".to_string(),
    })
}

#[tauri::command]
fn detect_vm_ips() -> Result<Vec<VmIpCandidate>, String> {
    let script = "Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -match '^(10\\.|172\\.(1[6-9]|2[0-9]|3[0-1])\\.|192\\.168\\.)' -and $_.IPAddress -notmatch '^169\\.254\\.' } | Select-Object -ExpandProperty IPAddress";
    let mut command = Command::new("powershell");
    let output = hide_console(
        command
            .args(["-NoProfile", "-Command", script])
            .stdout(Stdio::piped())
            .stderr(Stdio::null()),
    )
    .output()
    .map_err(|e| e.to_string())?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut ips = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|ip| VmIpCandidate {
            ip: ip.to_string(),
            source: if ip.starts_with("192.168.232.") {
                "VMware NAT preferred".to_string()
            } else {
                "Private IPv4".to_string()
            },
        })
        .collect::<Vec<_>>();

    ips.sort_by_key(|item| {
        if item.ip.starts_with("192.168.232.") {
            0
        } else {
            1
        }
    });

    Ok(ips)
}

#[tauri::command]
fn start_tunnel(
    state: tauri::State<AppState>,
    request: TunnelRequest,
) -> Result<TunnelStartResult, String> {
    let askpass_path = request
        .identity_passphrase
        .as_ref()
        .filter(|value| !value.is_empty())
        .map(|value| create_askpass_script(value, request.local_port))
        .transpose()?;

    let mut args = vec![
        "-4".to_string(),
        "-N".to_string(),
        "-g".to_string(),
        "-p".to_string(),
        request.server_port.to_string(),
        "-o".to_string(),
        "ExitOnForwardFailure=yes".to_string(),
        "-o".to_string(),
        "ServerAliveInterval=30".to_string(),
        "-o".to_string(),
        "ServerAliveCountMax=3".to_string(),
        "-o".to_string(),
        "PreferredAuthentications=publickey".to_string(),
        "-L".to_string(),
        format!(
            "0.0.0.0:{}:127.0.0.1:{}",
            request.local_port, request.server_port
        ),
    ];

    if let Some(identity) = request
        .identity_file
        .as_ref()
        .filter(|v| !v.trim().is_empty())
    {
        args.push("-i".to_string());
        args.push(identity.to_string());
        args.push("-o".to_string());
        args.push("IdentitiesOnly=yes".to_string());
    }

    args.push(format!("{}@{}", request.user, request.host));

    if request.reconnect {
        let delay = request.reconnect_delay.unwrap_or(5).max(1);
        let ps_args = args
            .iter()
            .map(|arg| quote_powershell_arg(arg))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            "$sshArgs=@({ps_args}); while ($true) {{ & ssh.exe @sshArgs; Start-Sleep -Seconds {delay}; }}"
        );
        let mut command = Command::new("powershell.exe");
        if let Some(path) = askpass_path.as_ref() {
            command.env("SSH_ASKPASS", path);
            command.env("SSH_ASKPASS_REQUIRE", "force");
            command.env("DISPLAY", "tunnel-helper");
        }
        let child = hide_console(
            command
                .args([
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &script,
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null()),
        )
        .spawn()
        .map_err(|e| e.to_string())?;
        let pid = child.id();
        let command = format!("ssh {}", args.join(" "));
        state
            .tunnels
            .lock()
            .map_err(|_| "Tunnel state is locked".to_string())?
            .insert(
                pid,
                ManagedTunnel {
                    child,
                    askpass_path,
                },
            );
        return Ok(TunnelStartResult {
            pid,
            command,
            connected_marker: None,
        });
    }

    let mut command = Command::new("ssh.exe");
    if let Some(path) = askpass_path.as_ref() {
        command.env("SSH_ASKPASS", path);
        command.env("SSH_ASKPASS_REQUIRE", "force");
        command.env("DISPLAY", "tunnel-helper");
    }
    let child = hide_console(
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .spawn()
    .map_err(|e| e.to_string())?;

    let pid = child.id();
    let command = format!("ssh {}", args.join(" "));

    state
        .tunnels
        .lock()
        .map_err(|_| "Tunnel state is locked".to_string())?
        .insert(
            pid,
            ManagedTunnel {
                child,
                askpass_path,
            },
        );

    Ok(TunnelStartResult {
        pid,
        command,
        connected_marker: None,
    })
}

#[tauri::command]
fn start_password_tunnel(
    state: tauri::State<AppState>,
    request: PasswordTunnelRequest,
) -> Result<TunnelStartResult, String> {
    let delay = request.reconnect_delay.unwrap_or(5).max(1);
    let password = request.password.unwrap_or_default();
    let askpass_path = (!password.is_empty())
        .then(|| create_askpass_script(&password, request.local_port))
        .transpose()?;
    let args = vec![
        "-4".to_string(),
        "-N".to_string(),
        "-g".to_string(),
        "-p".to_string(),
        request.server_port.to_string(),
        "-o".to_string(),
        "ExitOnForwardFailure=yes".to_string(),
        "-o".to_string(),
        "ServerAliveInterval=30".to_string(),
        "-o".to_string(),
        "ServerAliveCountMax=3".to_string(),
        "-o".to_string(),
        "PreferredAuthentications=password,keyboard-interactive".to_string(),
        "-o".to_string(),
        "PubkeyAuthentication=no".to_string(),
        "-L".to_string(),
        format!(
            "0.0.0.0:{}:127.0.0.1:{}",
            request.local_port, request.server_port
        ),
        format!("{}@{}", request.user, request.host),
    ];

    let mut command = if request.reconnect {
        let ps_args = args
            .iter()
            .map(|arg| quote_powershell_arg(arg))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            "$sshArgs=@({ps_args}); while ($true) {{ & ssh.exe @sshArgs; Start-Sleep -Seconds {delay}; }}"
        );
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ]);
        command
    } else {
        let mut command = Command::new("ssh.exe");
        command.args(&args);
        command
    };

    if let Some(path) = askpass_path.as_ref() {
        command.env("SSH_ASKPASS", path);
        command.env("SSH_ASKPASS_REQUIRE", "force");
        command.env("DISPLAY", "tunnel-helper");
    }
    let child = hide_console(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .spawn()
    .map_err(|e| e.to_string())?;
    let pid = child.id();
    let command_text = format!("ssh {}", args.join(" "));

    state
        .tunnels
        .lock()
        .map_err(|_| "Tunnel state is locked".to_string())?
        .insert(
            pid,
            ManagedTunnel {
                child,
                askpass_path,
            },
        );

    Ok(TunnelStartResult {
        pid,
        command: command_text,
        connected_marker: None,
    })
}

#[tauri::command]
fn check_tunnel_status(state: tauri::State<AppState>, pid: u32) -> Result<TunnelStatus, String> {
    let mut tunnels = state
        .tunnels
        .lock()
        .map_err(|_| "Tunnel state is locked".to_string())?;

    let Some(tunnel) = tunnels.get_mut(&pid) else {
        return Ok(TunnelStatus {
            pid,
            running: false,
            exit_code: None,
        });
    };

    match tunnel.child.try_wait().map_err(|e| e.to_string())? {
        Some(status) => {
            let code = status.code();
            let tunnel = tunnels.remove(&pid);
            cleanup_askpass(tunnel.and_then(|item| item.askpass_path));
            Ok(TunnelStatus {
                pid,
                running: false,
                exit_code: code,
            })
        }
        None => Ok(TunnelStatus {
            pid,
            running: true,
            exit_code: None,
        }),
    }
}

#[tauri::command]
fn stop_tunnel(state: tauri::State<AppState>, pid: u32) -> Result<(), String> {
    let mut tunnels = state
        .tunnels
        .lock()
        .map_err(|_| "Tunnel state is locked".to_string())?;

    let Some(mut tunnel) = tunnels.remove(&pid) else {
        return Err(format!("Tunnel process not found: {pid}"));
    };

    let mut command = Command::new("taskkill.exe");
    let _ = hide_console(
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .status();
    let _ = tunnel.child.kill();
    let _ = tunnel.child.wait();
    cleanup_askpass(tunnel.askpass_path.take());
    Ok(())
}

#[tauri::command]
fn generate_vscode_config(
    alias: String,
    vm_ip: String,
    local_port: u16,
    user: String,
    identity_file: Option<String>,
) -> String {
    let mut lines = vec![
        format!("Host {alias}"),
        format!("    HostName {vm_ip}"),
        format!("    Port {local_port}"),
        format!("    User {user}"),
        "    ServerAliveInterval 30".to_string(),
        "    ServerAliveCountMax 3".to_string(),
    ];

    if let Some(identity) = identity_file.filter(|v| !v.trim().is_empty()) {
        lines.push(format!("    IdentityFile {identity}"));
        lines.push("    IdentitiesOnly yes".to_string());
        lines.push("    PreferredAuthentications publickey".to_string());
    }

    lines.join("\n")
}

#[tauri::command]
fn write_vscode_config(
    alias: String,
    vm_ip: String,
    local_port: u16,
    user: String,
    identity_file: Option<String>,
) -> Result<String, String> {
    let config_text = generate_vscode_config(alias.clone(), vm_ip, local_port, user, identity_file);
    let ssh_dir = user_ssh_dir()?;
    fs::create_dir_all(&ssh_dir).map_err(|e| e.to_string())?;
    let path = ssh_dir.join("config");

    let begin = format!("# BEGIN tunnel-helper {alias}");
    let end = format!("# END tunnel-helper {alias}");
    let block = format!("{begin}\n{config_text}\n{end}\n");
    let old = fs::read_to_string(&path).unwrap_or_default();

    let next = if old.contains(&begin) {
        let replace_begin = begin.as_str();
        let replace_end = end.as_str();
        let start = old.find(replace_begin).unwrap_or(0);
        if let Some(relative_end) = old[start..].find(replace_end) {
            let finish = start + relative_end + replace_end.len();
            format!(
                "{}{}{}",
                &old[..start],
                block,
                old[finish..].trim_start_matches(|c| c == '\r' || c == '\n')
            )
        } else {
            format!("{}\n{}", old.trim_end(), block)
        }
    } else if old.trim().is_empty() {
        block
    } else {
        format!("{}\n\n{}", old.trim_end(), block)
    };

    fs::write(&path, next).map_err(|e| e.to_string())?;
    Ok(normalize_path(&path))
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let resolved = if path == "~" {
        env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| "USERPROFILE is not set".to_string())?
    } else if let Some(rest) = path.strip_prefix("~/") {
        env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| "USERPROFILE is not set".to_string())?
            .join(rest)
    } else if let Some(rest) = path.strip_prefix("~\\") {
        env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| "USERPROFILE is not set".to_string())?
            .join(rest)
    } else {
        PathBuf::from(path)
    };
    let mut command = Command::new("explorer.exe");
    hide_console(command.arg(resolved))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn hide_process_window(pid: u32) -> Result<bool, String> {
    let script = format!(
        "$p = Get-Process -Id {pid} -ErrorAction SilentlyContinue; if (-not $p) {{ 'false'; exit 0 }}; Add-Type -Namespace TunnelHelper -Name Win32Window -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);'; $handle = $p.MainWindowHandle; if ($handle -eq 0) {{ Start-Sleep -Milliseconds 250; $p.Refresh(); $handle = $p.MainWindowHandle }}; if ($handle -ne 0) {{ [TunnelHelper.Win32Window]::ShowWindow($handle, 0) | Out-Null; 'true' }} else {{ 'false' }}"
    );
    let mut command = Command::new("powershell.exe");
    let output = hide_console(
        command
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .output()
    .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .eq_ignore_ascii_case("true"))
}

#[tauri::command]
fn window_minimize(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_toggle_maximize(window: tauri::Window) -> Result<(), String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn window_close(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_start_drag(window: tauri::Window) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            load_profiles,
            save_profiles,
            load_app_settings,
            save_app_settings,
            scan_key_bundles,
            export_key_files,
            import_key_files,
            delete_key_files,
            export_public_bundle,
            generate_ssh_key,
            install_public_to_linux,
            install_public_to_linux_password,
            launch_public_install_terminal,
            find_free_port,
            is_local_port_open,
            is_local_port_available,
            ping_host,
            launch_ssh_test_terminal,
            test_ssh_server,
            test_password_ssh_server,
            ensure_firewall_rule,
            is_admin,
            request_firewall_admin,
            detect_vm_ips,
            start_tunnel,
            start_password_tunnel,
            check_tunnel_status,
            stop_tunnel,
            generate_vscode_config,
            write_vscode_config,
            hide_process_window,
            window_minimize,
            window_toggle_maximize,
            window_close,
            window_start_drag,
            open_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
