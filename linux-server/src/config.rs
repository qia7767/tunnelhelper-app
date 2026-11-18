use clap::{Parser, ValueEnum};
use rand::{distr::Alphanumeric, Rng};
use serde_json::{json, Value};
use std::{
    env, fs,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AuthMode {
    Disabled,
    Token,
}

#[derive(Debug, Parser)]
#[command(
    name = "tunnel-helper",
    version,
    about = "Headless Tunnel Helper web service"
)]
pub struct Cli {
    #[arg(
        long,
        help = "Listener IP (default: 0.0.0.0, unless set in app_settings.json)"
    )]
    pub listen: Option<IpAddr>,

    #[arg(
        long,
        help = "Listener port (default: 6789, unless set in app_settings.json)"
    )]
    pub port: Option<u16>,

    #[arg(long)]
    pub data_dir: Option<PathBuf>,

    #[arg(long, value_enum)]
    pub auth: Option<AuthMode>,

    #[arg(long, env = "TUNNEL_HELPER_ACCESS_TOKEN")]
    pub access_token: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub listen: IpAddr,
    pub port: u16,
    pub data_dir: PathBuf,
    pub auth_mode: AuthMode,
    pub access_token: Option<String>,
}

impl ServerConfig {
    pub fn from_cli(cli: Cli) -> Result<Self, String> {
        let data_dir = match cli.data_dir {
            Some(path) => absolutize(path)?,
            None => executable_dir()?.join("data"),
        };

        let path = data_dir.join("app_settings.json");
        let legacy_path = data_dir.join("server_settings.json");
        let mut app_settings = read_settings(&path)?;
        if app_settings.get("server").is_none() && legacy_path.exists() {
            let legacy = read_settings(&legacy_path)?;
            validate_server_settings(&legacy)?;
            app_settings["server"] = legacy;
            write_settings(&path, &app_settings)?;
        }
        let mut settings = match app_settings.get("server") {
            None | Some(Value::Null) => json!({}),
            Some(value) if value.is_object() => value.clone(),
            Some(_) => {
                return Err("app_settings.json: server must be an object or null".to_string())
            }
        };
        validate_server_settings(&settings)?;
        #[cfg(not(windows))]
        let configured_token = settings
            .get("tokenEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        #[cfg(windows)]
        let configured_token = true;
        #[cfg(windows)]
        if cli.auth == Some(AuthMode::Disabled) {
            return Err("Windows web service requires token authentication".to_string());
        }
        let auth_mode = cli.auth.unwrap_or(if configured_token {
            AuthMode::Token
        } else {
            AuthMode::Disabled
        });
        let listen = match cli.listen {
            Some(ip) => ip,
            None => {
                if settings
                    .get("listen")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    == Some("127.0.0.1")
                {
                    IpAddr::V4(Ipv4Addr::LOCALHOST)
                } else {
                    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
                }
            }
        };
        let port = match cli.port {
            Some(port) => port,
            None => match settings.get("port") {
                None | Some(Value::Null) => 6789,
                Some(value) => u16::try_from(
                    value
                        .as_u64()
                        .ok_or("app_settings.json: invalid server port")?,
                )
                .map_err(|_| "app_settings.json: invalid server port")?,
            },
        };
        let access_token = match auth_mode {
            AuthMode::Disabled => None,
            AuthMode::Token => {
                let stored = settings
                    .get("accessToken")
                    .and_then(Value::as_str)
                    .filter(|token| !token.trim().is_empty())
                    .map(str::to_owned);
                let token = cli
                    .access_token
                    .filter(|token| !token.trim().is_empty())
                    .or(stored);
                let token = match token {
                    Some(token) => token,
                    None if configured_token => {
                        let token = generate_access_token();
                        settings["accessToken"] = json!(token);
                        #[cfg(windows)]
                        { settings["tokenEnabled"] = json!(true); }
                        app_settings["server"] = settings.clone();
                        write_settings(&path, &app_settings)?;
                        token
                    }
                    None => generate_access_token(),
                };
                Some(token)
            }
        };

        Ok(Self {
            listen,
            port,
            data_dir,
            auth_mode,
            access_token,
        })
    }

    pub fn display_host(&self) -> IpAddr {
        if self.listen == IpAddr::V4(Ipv4Addr::UNSPECIFIED) {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            self.listen
        }
    }
}

pub(crate) fn read_settings(path: &std::path::Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    let settings: Value = serde_json::from_str(&text)
        .map_err(|error| format!("Invalid {}: {error}", path.display()))?;
    if !settings.is_object() {
        return Err(format!("{} must contain a JSON object", path.display()));
    }
    Ok(settings)
}

fn validate_server_settings(settings: &Value) -> Result<(), String> {
    if let Some(value) = settings.get("tokenEnabled") {
        if !value.is_null()
            && !value.is_boolean()
            && !value.as_str().is_some_and(|text| text.trim().is_empty())
        {
            return Err(
                "app_settings.json: server.tokenEnabled must be true, false, null, or empty"
                    .to_string(),
            );
        }
    }
    Ok(())
}

pub(crate) fn write_settings(path: &std::path::Path, settings: &Value) -> Result<(), String> {
    use std::io::Write;
    fs::create_dir_all(path.parent().ok_or("Missing settings directory")?)
        .map_err(|error| error.to_string())?;
    let text = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    let temp_path = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let file = options
            .open(&temp_path)
            .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;
        #[cfg(windows)]
        let mut file = {
            drop(file);
            crate::service::secure_private_key(&temp_path)?;
            fs::OpenOptions::new().write(true).open(&temp_path).map_err(|error| error.to_string())?
        };
        #[cfg(not(windows))]
        let mut file = file;
        file.write_all(format!("{text}\n").as_bytes())
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temp_path, path)
            .map_err(|error| format!("Cannot replace {}: {error}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn executable_dir() -> Result<PathBuf, String> {
    env::current_exe()
        .map_err(|error| format!("Cannot resolve executable path: {error}"))?
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "Cannot resolve executable directory".to_string())
}

fn absolutize(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path)
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| format!("Cannot resolve data directory: {error}"))
    }
}

fn generate_access_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cli(dir: PathBuf) -> Cli {
        Cli {
            listen: None,
            port: None,
            data_dir: Some(dir),
            auth: None,
            access_token: None,
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_defaults_to_persistent_token_and_rejects_disabled_auth() {
        let dir = env::temp_dir().join(format!("tunnel-helper-win-token-test-{}", rand::random::<u64>()));
        let first = ServerConfig::from_cli(test_cli(dir.clone())).unwrap();
        let second = ServerConfig::from_cli(test_cli(dir.clone())).unwrap();
        assert_eq!(first.auth_mode, AuthMode::Token);
        assert_eq!(first.access_token, second.access_token);
        assert_eq!(first.access_token.as_ref().unwrap().len(), 40);
        let mut disabled = test_cli(dir.clone());
        disabled.auth = Some(AuthMode::Disabled);
        assert!(ServerConfig::from_cli(disabled).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn absent_configuration_defaults_to_lan_without_creating_settings() {
        let dir = env::temp_dir().join(format!(
            "tunnel-helper-config-test-{}",
            rand::random::<u64>()
        ));
        let config = ServerConfig::from_cli(test_cli(dir.clone())).unwrap();
        assert_eq!(config.listen, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert_eq!(config.port, 6789);
        assert_eq!(config.auth_mode, AuthMode::Disabled);
        assert!(!dir.exists());
    }

    #[test]
    fn enabled_token_is_saved_and_reused() {
        let dir = env::temp_dir().join(format!(
            "tunnel-helper-config-test-{}",
            rand::random::<u64>()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app_settings.json");
        fs::write(
            &path,
            r#"{"language":"en","server":{"tokenEnabled":true,"other":"preserved"}}"#,
        )
        .unwrap();
        let first = ServerConfig::from_cli(test_cli(dir.clone())).unwrap();
        let second = ServerConfig::from_cli(test_cli(dir.clone())).unwrap();
        assert_eq!(first.auth_mode, AuthMode::Token);
        assert_eq!(first.access_token, second.access_token);
        assert_eq!(first.access_token.as_ref().unwrap().len(), 40);
        let saved = read_settings(&path).unwrap();
        assert_eq!(saved["language"], "en");
        assert_eq!(saved["server"]["other"], "preserved");
        assert_eq!(
            saved["server"]["accessToken"].as_str(),
            first.access_token.as_deref()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn empty_or_disabled_token_never_enables_authentication() {
        let dir = env::temp_dir().join(format!(
            "tunnel-helper-config-test-{}",
            rand::random::<u64>()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app_settings.json");
        for value in [json!(null), json!(""), json!("  "), json!(false)] {
            fs::write(
                &path,
                json!({ "server": { "tokenEnabled": value, "accessToken": "unused" } }).to_string(),
            )
            .unwrap();
            assert_eq!(
                ServerConfig::from_cli(test_cli(dir.clone()))
                    .unwrap()
                    .auth_mode,
                AuthMode::Disabled
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn file_listener_only_selects_loopback_when_explicit() {
        let dir = env::temp_dir().join(format!(
            "tunnel-helper-config-test-{}",
            rand::random::<u64>()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app_settings.json");
        for (value, expected) in [
            (json!("127.0.0.1"), Ipv4Addr::LOCALHOST),
            (json!(""), Ipv4Addr::UNSPECIFIED),
            (json!("192.0.2.1"), Ipv4Addr::UNSPECIFIED),
            (json!(null), Ipv4Addr::UNSPECIFIED),
        ] {
            fs::write(&path, json!({ "server": { "listen": value } }).to_string()).unwrap();
            assert_eq!(
                ServerConfig::from_cli(test_cli(dir.clone()))
                    .unwrap()
                    .listen,
                IpAddr::V4(expected)
            );
        }
        let mut cli = test_cli(dir.clone());
        cli.listen = Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)));
        assert_eq!(
            ServerConfig::from_cli(cli).unwrap().listen,
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_settings_migrate_without_overwriting_app_settings() {
        let dir = env::temp_dir().join(format!(
            "tunnel-helper-config-test-{}",
            rand::random::<u64>()
        ));
        fs::create_dir_all(&dir).unwrap();
        let app = dir.join("app_settings.json");
        let legacy = dir.join("server_settings.json");
        fs::write(
            &app,
            r#"{"language":"en","defaults":{"keygenUser":"example"}}"#,
        )
        .unwrap();
        let legacy_text =
            r#"{"listen":"127.0.0.1","tokenEnabled":true,"accessToken":"test-token"}"#;
        fs::write(&legacy, legacy_text).unwrap();
        let config = ServerConfig::from_cli(test_cli(dir.clone())).unwrap();
        assert_eq!(config.listen, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(config.access_token.as_deref(), Some("test-token"));
        let saved = read_settings(&app).unwrap();
        assert_eq!(saved["language"], "en");
        assert_eq!(saved["defaults"]["keygenUser"], "example");
        assert_eq!(saved["server"]["accessToken"], "test-token");
        assert_eq!(fs::read_to_string(legacy).unwrap(), legacy_text);
        fs::remove_dir_all(dir).unwrap();
    }
}
