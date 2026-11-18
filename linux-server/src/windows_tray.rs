use std::{io::Cursor, net::IpAddr, sync::{atomic::{AtomicBool, Ordering}, Arc}, time::{Duration, Instant}};
use tokio::sync::mpsc::UnboundedSender;
use tray_icon::{menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, Submenu}, Icon, TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};
use crate::service::CommandService;
use windows_sys::Win32::{
    Foundation::GlobalFree,
    System::{Console::{AllocConsole, AttachConsole, GetConsoleWindow, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE}, DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData}, Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE}, Ole::CF_UNICODETEXT},
    UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, DispatchMessageW, MessageBoxW, PeekMessageW, TranslateMessage, HWND_MESSAGE, MB_OK, MSG, PM_REMOVE},
};

pub struct TrayGuard(Arc<AtomicBool>);

impl Drop for TrayGuard {
    fn drop(&mut self) { self.0.store(false, Ordering::Relaxed); }
}

pub fn attach_parent_console() -> bool {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    unsafe {
        if !GetConsoleWindow().is_null() || AttachConsole(ATTACH_PARENT_PROCESS) != 0 { return true; }
        let output = GetStdHandle(STD_OUTPUT_HANDLE);
        !output.is_null() && output != INVALID_HANDLE_VALUE
    }
}

pub fn start(host: IpAddr, port: u16, token: String, shutdown: UnboundedSender<()>, service: CommandService) -> Option<TrayGuard> {
    let running = Arc::new(AtomicBool::new(true));
    let thread_running = running.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = run_tray(host, port, token, shutdown, service, thread_running, &ready_tx);
        if let Err(error) = result { let _ = ready_tx.send(Err(error)); }
    });
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Some(TrayGuard(running)),
        _ => { running.store(false, Ordering::Relaxed); None }
    }
}

pub fn allocate_fallback_console() -> bool {
    unsafe { AllocConsole() != 0 }
}

pub fn show_start_error(error: &str) {
    let text = wide(error);
    let title = wide("Tunnel Helper");
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK); }
}

fn run_tray(host: IpAddr, port: u16, token: String, shutdown: UnboundedSender<()>, service: CommandService, running: Arc<AtomicBool>, ready: &std::sync::mpsc::SyncSender<Result<(), String>>) -> Result<(), String> {
    let menu = Menu::new();
    let open = MenuItem::with_id("open", "打开网页", true, None);
    let copy = MenuItem::with_id("copy", "复制 Token", true, None);
    let quit = MenuItem::with_id("quit", "关闭连接并退出", true, None);
    let zh = CheckMenuItem::with_id("lang-zh", "中文", true, false, None);
    let en = CheckMenuItem::with_id("lang-en", "English", true, false, None);
    let language_menu = Submenu::with_items("语言", true, &[&zh, &en]).map_err(|error| error.to_string())?;
    menu.append_items(&[&open, &copy, &language_menu, &quit]).map_err(|error| error.to_string())?;
    let mut language = service.tray_language().unwrap_or_else(|_| "zh".to_string());
    set_menu_language(&language, &open, &copy, &quit, &language_menu, &zh, &en);
    let dir = ico::IconDir::read(Cursor::new(include_bytes!("../../src-tauri/icons/icon.ico"))).map_err(|error| error.to_string())?;
    let image = dir.entries().iter().min_by_key(|entry| entry.width().abs_diff(32)).ok_or("Icon has no images")?.decode().map_err(|error| error.to_string())?;
    let icon = Icon::from_rgba(image.rgba_data().to_vec(), image.width(), image.height()).map_err(|error| error.to_string())?;
    let _tray = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Tunnel Helper Web").with_icon(icon).build().map_err(|error| error.to_string())?;
    let class = wide("STATIC");
    let owner = unsafe { CreateWindowExW(0, class.as_ptr(), std::ptr::null(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null()) };
    if owner.is_null() { return Err("Cannot create clipboard window".to_string()); }
    if !running.load(Ordering::Relaxed) { unsafe { DestroyWindow(owner); } return Ok(()); }
    let _ = ready.send(Ok(()));
    let url = format!("http://{host}:{port}");
    let mut last_language_check = Instant::now();
    while running.load(Ordering::Relaxed) {
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.0.as_str() {
                "open" => open_url(&url),
                "copy" => {
                    let message = match (copy_to_clipboard(owner, &token), language.as_str()) {
                        (true, "en") => "Token copied to clipboard.",
                        (false, "en") => "Could not copy the Token. Check the clipboard and try again.",
                        (true, _) => "Token 已复制到剪贴板。",
                        (false, _) => "复制失败，请检查剪贴板后重试。",
                    };
                    let text = wide(message); let title = wide("Tunnel Helper");
                    unsafe { MessageBoxW(owner, text.as_ptr(), title.as_ptr(), MB_OK); }
                }
                "quit" => { let _ = shutdown.send(()); running.store(false, Ordering::Relaxed); }
                "lang-zh" | "lang-en" => {
                    let selected = if event.id.0 == "lang-en" { "en" } else { "zh" };
                    if service.set_tray_language(selected).is_ok() {
                        language = selected.to_string();
                    } else {
                        let text = wide(if language == "en" { "Could not save the language setting." } else { "无法保存语言设置。" });
                        let title = wide("Tunnel Helper");
                        unsafe { MessageBoxW(owner, text.as_ptr(), title.as_ptr(), MB_OK); }
                    }
                    set_menu_language(&language, &open, &copy, &quit, &language_menu, &zh, &en);
                }
                _ => {}
            }
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(event, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) { open_url(&url); }
        }
        if last_language_check.elapsed() >= Duration::from_secs(1) {
            if let Ok(current) = service.tray_language() {
                if current != language {
                    language = current;
                    set_menu_language(&language, &open, &copy, &quit, &language_menu, &zh, &en);
                }
            }
            last_language_check = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    unsafe { DestroyWindow(owner); }
    Ok(())
}

fn set_menu_language(language: &str, open: &MenuItem, copy: &MenuItem, quit: &MenuItem, language_menu: &Submenu, zh: &CheckMenuItem, en: &CheckMenuItem) {
    let english = language == "en";
    open.set_text(if english { "Open web interface" } else { "打开网页" });
    copy.set_text(if english { "Copy Token" } else { "复制 Token" });
    quit.set_text(if english { "Stop connections and exit" } else { "关闭连接并退出" });
    language_menu.set_text(if english { "Language" } else { "语言" });
    zh.set_checked(!english);
    en.set_checked(english);
}

fn open_url(url: &str) { let _ = std::process::Command::new("explorer.exe").arg(url).spawn(); }

fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(std::iter::once(0)).collect() }

fn copy_to_clipboard(owner: windows_sys::Win32::Foundation::HWND, value: &str) -> bool {
    let text = wide(value);
    unsafe {
        if OpenClipboard(owner) == 0 { return false; }
        let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * std::mem::size_of::<u16>());
        if memory.is_null() { CloseClipboard(); return false; }
        let buffer = GlobalLock(memory).cast::<u16>();
        if buffer.is_null() { GlobalFree(memory); CloseClipboard(); return false; }
        std::ptr::copy_nonoverlapping(text.as_ptr(), buffer, text.len());
        GlobalUnlock(memory);
        let ok = EmptyClipboard() != 0 && !SetClipboardData(u32::from(CF_UNICODETEXT), memory).is_null();
        if !ok { GlobalFree(memory); }
        CloseClipboard();
        ok
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn tray_starts_and_drops_without_shutdown() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let dir = std::env::temp_dir().join(format!("tunnel-tray-test-{}-{}", std::process::id(), rand::random::<u64>()));
        let config = crate::config::ServerConfig { listen: "127.0.0.1".parse().unwrap(), port: 16819,
            data_dir: dir.clone(), auth_mode: crate::config::AuthMode::Token, access_token: Some("test-token".into()) };
        let service = crate::service::CommandService::new(&config).unwrap();
        let tray = super::start("127.0.0.1".parse().unwrap(), 16819, "test-token".into(), sender, service);
        assert!(tray.is_some(), "system tray should initialize on the interactive desktop");
        drop(tray);
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(receiver.try_recv().is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
