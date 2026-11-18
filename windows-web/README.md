# Tunnel Helper Windows Web (experimental)

This is a separate browser-controlled Windows service. It does not share connection state or the program data directory with the Windows desktop edition. Both applications can access the current user's SSH directory, so handle shared keys carefully.

## Build and run

From this directory, run `./build.ps1`, then launch `dist/tunnel-helper-web.exe`. The program requires Windows OpenSSH (`ssh.exe` and `ssh-keygen.exe`) and PowerShell. Double-clicking does not create a console window. Right-click the tray icon to open the web interface, copy the access token, or stop the service. If the tray cannot be created, the program attempts to open a fallback console. When launched from an existing terminal, it attempts to attach to that console for diagnostics.

The tray Language menu switches between Chinese and English. The language is shared with the web settings and saved in `app_settings.json`.

By default the service listens on all IPv4 interfaces on port 6789 and requires a persistent access token. It generates the token in `data/app_settings.json` beside the executable. Enter the token when prompted by the browser. The data directory also contains profiles and keys and must not be shared or uploaded.

HTTP does not encrypt the token, passwords, or other data. Use only on a trusted network or through an encrypted channel. To restrict access to the same computer, set `server.listen` in `data/app_settings.json` to `127.0.0.1`, or start with `--listen 127.0.0.1`. Set `server.port` or use `--port` to choose another port. Windows web mode requires token authentication.

The service must remain running and the Windows machine or virtual machine must not sleep. Use the page's Stop Service button or Ctrl+C to stop managed tunnels and exit. Merely closing the browser does not stop the service.

## Current limitations

- Windows reverse tunnels are not supported in this experimental build.
- Windows askpass currently accepts ASCII passwords/passphrases only; newline and NUL characters are rejected.
- Server-password login and multiple-browser interaction need further end-to-end testing before production use.
- A firewall rule may be required for access from other machines; the service does not change the Windows firewall automatically.
- If Windows process inspection is unavailable, the service reports the connection as not confirmed rather than assuming success.
