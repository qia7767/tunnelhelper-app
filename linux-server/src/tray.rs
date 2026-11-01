use ksni::{menu::StandardItem, TrayMethods};
use std::{net::IpAddr, process::Command};
use tokio::sync::oneshot;

pub struct DesktopTray {
    url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl DesktopTray {
    fn open(&self) {
        if let Err(error) = Command::new("xdg-open").arg(&self.url).spawn() {
            eprintln!("Cannot open browser: {error}");
        }
    }
}

impl ksni::Tray for DesktopTray {
    fn id(&self) -> String {
        "tunnel-helper".into()
    }
    fn title(&self) -> String {
        "Tunnel Helper".into()
    }
    fn icon_name(&self) -> String {
        "network-workgroup".into()
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        self.open();
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "打开 Tunnel Helper".into(),
                activate: Box::new(|tray: &mut Self| tray.open()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "关闭所有连接并退出".into(),
                activate: Box::new(|tray: &mut Self| {
                    if let Some(shutdown) = tray.shutdown.take() {
                        let _ = shutdown.send(());
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub async fn start(
    host: IpAddr,
    port: u16,
) -> (
    std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
    Option<ksni::Handle<DesktopTray>>,
) {
    let (tx, rx) = oneshot::channel();
    let desktop =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if desktop {
        let tray = DesktopTray {
            url: format!("http://{host}:{port}"),
            shutdown: Some(tx),
        };
        tray.open();
        match tray.spawn().await {
            Ok(handle) => {
                return (
                    Box::pin(async move {
                        let _ = rx.await;
                    }),
                    Some(handle),
                )
            }
            Err(error) => eprintln!("System tray unavailable: {error}"),
        }
    }
    // A closed channel must not shut down headless operation.
    (Box::pin(std::future::pending()), None)
}
