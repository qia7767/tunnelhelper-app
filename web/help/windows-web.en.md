# Tunnel Helper Windows Web

## Use and security
This service runs on Windows and lets browsers manage SSH keys, profiles, and tunnels. Double-clicking creates no console; use the system tray menu to open the page, copy the token, or stop and exit. When launched from an existing terminal, the service attempts to attach to it for diagnostics. If the tray is unavailable, it attempts to open a fallback console. The token is stored in data/app_settings.json beside the program; do not share that file.
The tray Language menu switches between Chinese and English and shares the setting with the web interface.
By default the service listens on all IPv4 addresses on port 6789. HTTP does not encrypt the token or passwords. Use a trusted network or an encrypted channel. Set server.listen to 127.0.0.1 in the settings file, or start with --listen 127.0.0.1, to restrict access to this computer.

## Keys and profiles
Key directory paths refer to the Windows host running the service, not the browser computer. Copy Path does not transfer key files. Profiles and private keys may contain sensitive information; do not publish the data directory. Duplicating a profile also copies stored passwords.

## Tunnels and shutdown
Select a profile under Connection and start a tunnel. The service shares its Connections list across browsers. Closing the browser does not stop the service. Stop Service in the page or Ctrl+C in the service window stops managed tunnels and exits. A sleeping Windows virtual machine cannot serve requests.

## Current limitations
This is an experimental version and does not support Windows reverse tunnels. Windows askpass currently accepts ASCII passwords and passphrases only. Multiple-browser use, server-password authentication, and recovery from network interruptions need additional real-world testing. Configure Windows Firewall separately if needed.
