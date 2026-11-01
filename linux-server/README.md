# Tunnel Helper Linux Server

This directory contains the headless Linux edition of Tunnel Helper. It embeds the shared `../web` frontend into one Rust executable and exposes the existing command bridge over a local HTTP API.

## Requirements

- A recent Rust toolchain
- OpenSSH client commands: `ssh`, `ssh-keygen`
- `ping`

## Local development

```bash
cargo run -- --data-dir ../.dev-data/linux
```

Open `http://127.0.0.1:6789`. The default mode deliberately disables authentication only while listening on a loopback address.

## Portable build

For a self-contained x86_64 binary, install a musl linker once and run the build script:

```bash
sudo apt install musl-tools
sh build-portable.sh
mkdir portable
tar -xzf dist/tunnel-helper-linux-x86_64.tar.gz -C portable
cd portable
./tunnel-helper
```

The generated executable is static PIE and embeds the complete browser frontend. It does not require glibc or shared application libraries on the destination machine. The tar archive preserves the executable bit and contains a single file named `tunnel-helper`. Tunnel and key operations still call the standard Linux commands `ssh` and `ssh-keygen`; install the OpenSSH client if the destination distribution does not provide them.

The default portable layout is:

```text
portable/
  tunnel-helper
  data/
    profiles.json
    app_settings.json
    ssh_keyfiles_export/
    ssh_public_keys_bundle.pub
```

Files under `data/` are created only when needed. On Linux, the service forces the data directories to mode `0700`, JSON configuration files and private keys to `0600`, and temporary askpass scripts to `0700`. Do not publish a populated data directory because profiles may contain server details, passwords, private-key passphrases, and private keys.

## Access Token

Token support is implemented but not enabled in the default local test mode. A non-loopback listener requires it:

```bash
./tunnel-helper --listen 0.0.0.0 --port 6789 --auth token
```

The service generates and prints a token at startup. To supply a stable token:

```bash
TUNNEL_HELPER_ACCESS_TOKEN='replace-with-a-long-random-value' \
  ./tunnel-helper --listen 0.0.0.0 --auth token
```

The browser sends the token in `X-Tunnel-Token` and keeps it in `sessionStorage`, so closing the browser session clears it. Requests without a valid token receive HTTP 401. The server refuses to bind a non-loopback address when authentication is disabled.

## Reverse tunnels

Linux browser mode supports a reverse SSH tunnel for the case where server B has no stable or reachable address but can initiate SSH connections to a fixed-address server A.

1. Create or edit a profile for server A.
2. Set **Reverse Tunnel** to **Yes**.
3. Use the tunnel-port field as the loopback-only remote port on A, for example `22022`.
4. Set the user and SSH port of server B for the generated client configuration.
5. Start the tunnel on B.

The server runs the equivalent of:

```bash
ssh -N -R 127.0.0.1:22022:127.0.0.1:22 user-a@server-a
```

`ExitOnForwardFailure=yes` prevents a port conflict on A from being reported as a successful connection. An independent OpenSSH control socket is created for each connection; the UI reports the reverse tunnel as connected only after that socket exists. Automatic reconnect uses the existing reconnect setting.

The SSH configuration panel generates two entries for the personal computer: one for A and one for B with `ProxyJump`. It is intended to be copied to the personal computer; the write-to-`~/.ssh/config` button is disabled in reverse mode because writing it on B would not be useful.

Server A must allow TCP forwarding in `sshd_config`. The remote port is deliberately bound to `127.0.0.1` on A, so it is reachable through `ProxyJump` but is not directly exposed to the campus network.

## Command-line options

```text
--listen <IP>           Listener address (default: 127.0.0.1)
--port <PORT>           Browser/API port (default: 6789)
--data-dir <PATH>       Override the isolated data directory
--auth disabled|token   Authentication mode (default: disabled)
--access-token <TOKEN>  Explicit Access Token
```

## Linux VM validation

The code can be compiled and API-tested on Windows, but these behaviors must be verified in a real Linux VM before publishing a Linux binary:

- private-key and askpass file permissions (`0600` and `0700`)
- OpenSSH authentication and public-key installation
- process-group shutdown and tunnel port release
- automatic reconnect after an SSH process exits
- SIGINT/SIGTERM graceful shutdown of all managed tunnels
