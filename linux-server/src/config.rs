use clap::{Parser, ValueEnum};
use rand::{distr::Alphanumeric, Rng};
use std::{
    env,
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
    #[arg(long, default_value = "127.0.0.1")]
    pub listen: IpAddr,

    #[arg(long, default_value_t = 6789)]
    pub port: u16,

    #[arg(long)]
    pub data_dir: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = AuthMode::Disabled)]
    pub auth: AuthMode,

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
        if !cli.listen.is_loopback() && cli.auth == AuthMode::Disabled {
            return Err(
                "Refusing to listen on a non-loopback address without Access Token authentication. Use --auth token."
                    .to_string(),
            );
        }

        let data_dir = match cli.data_dir {
            Some(path) => absolutize(path)?,
            None => executable_dir()?.join("data"),
        };

        let access_token = match cli.auth {
            AuthMode::Disabled => None,
            AuthMode::Token => Some(
                cli.access_token
                    .filter(|token| !token.trim().is_empty())
                    .unwrap_or_else(generate_access_token),
            ),
        };

        Ok(Self {
            listen: cli.listen,
            port: cli.port,
            data_dir,
            auth_mode: cli.auth,
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
