use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct KeyBundle {
    pub name: String,
    pub private_path: String,
    pub public_path: Option<String>,
    pub fingerprint: Option<String>,
    pub source_dir: String,
}

#[derive(Debug, Serialize)]
pub struct VmIpCandidate {
    pub ip: String,
    pub source: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub user: String,
    pub host: String,
    pub server_port: u16,
    pub base_local_port: u16,
    pub identity_file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_passphrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_password: Option<String>,
    pub auth_mode: Option<String>,
    pub host_alias: String,
    #[serde(default)]
    pub reverse_tunnel: bool,
    #[serde(default = "default_ssh_port")]
    pub reverse_target_port: u16,
    #[serde(default)]
    pub reverse_target_user: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct AppDefaults {
    pub keygen_user: String,
    pub keygen_host: String,
    pub keygen_offset: String,
    pub install_key_name: String,
    pub install_user: String,
    pub install_host: String,
    pub install_port: u16,
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
pub struct AppSettings {
    pub language: String,
    pub cursor: String,
    pub defaults: AppDefaults,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "zh".to_string(),
            cursor: "default".to_string(),
            defaults: AppDefaults::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TunnelRequest {
    pub user: String,
    pub host: String,
    pub server_port: u16,
    pub local_port: u16,
    pub identity_file: Option<String>,
    pub identity_passphrase: Option<String>,
    pub reconnect: bool,
    pub reconnect_delay: Option<u64>,
    #[serde(default)]
    pub reverse_tunnel: bool,
    #[serde(default = "default_ssh_port")]
    pub reverse_target_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct PasswordTunnelRequest {
    pub user: String,
    pub host: String,
    pub server_port: u16,
    pub local_port: u16,
    pub password: Option<String>,
    pub reconnect: bool,
    pub reconnect_delay: Option<u64>,
    #[serde(default)]
    pub reverse_tunnel: bool,
    #[serde(default = "default_ssh_port")]
    pub reverse_target_port: u16,
}

#[derive(Debug, Serialize)]
pub struct TunnelStartResult {
    pub pid: u32,
    pub command: String,
    pub connected_marker: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TunnelStatus {
    pub pid: u32,
    pub running: bool,
    pub exit_code: Option<i32>,
    pub connected: bool,
}

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
}

fn default_ssh_port() -> u16 {
    22
}

#[cfg(test)]
mod tests {
    use super::Profile;

    #[test]
    fn old_profiles_default_to_local_forwarding() {
        let profile: Profile = serde_json::from_str(
            r#"{
                "name":"legacy",
                "user":"user",
                "host":"server-a",
                "serverPort":22,
                "baseLocalPort":2222,
                "identityFileName":null,
                "identityFilePath":null,
                "identityPassphrase":null,
                "serverPassword":null,
                "authMode":"key",
                "hostAlias":"legacy"
            }"#,
        )
        .expect("legacy profile should remain readable");

        assert!(!profile.reverse_tunnel);
        assert_eq!(profile.reverse_target_port, 22);
        assert!(profile.reverse_target_user.is_empty());
    }
}
