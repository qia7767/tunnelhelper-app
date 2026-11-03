# Tunnel Helper Guide

## About the application
Tunnel Helper is a visual tool for managing SSH keys, connection profiles, and SSH tunnels. The Linux edition can run on a headless server and be controlled through a browser; the Windows edition is a desktop application.
For example, a VPN or other network client can run in a virtual machine, which establishes an SSH tunnel for a personal computer to access an authorized server. Tunnel Helper is not itself a VPN and does not bypass network access restrictions.

## Main features
Key management: inspect keys in the user's ~/.ssh and the program directory, import, export or delete key pairs, and combine selected public keys into a bundle. A public-key bundle does not contain private keys.
Key generation and installation: create Ed25519 keys with an optional private-key passphrase, install a public key on a server, and test key authentication.
Profiles: save server, port, authentication and key settings. Drag profiles to reorder them; reverse-tunnel profiles have a distinct color.
Connection configuration: select a profile, test the server, start an SSH tunnel, configure reconnect, and generate or copy SSH configuration.
Connections and Logs: inspect or stop tunnels managed by the current service and view operation logs. Settings controls language and other interface preferences.

## Getting started
1. Make sure the machine running the program has an OpenSSH client and can reach the server's SSH port.
2. Inspect an existing key in Key Management, or generate a key, install its public key and verify login under Key Generation and Installation. Server-password authentication is also available.
3. Create a profile with server details, authentication and ports. Save it and select Apply to Connection.
4. Review the connection settings, enable reconnect if needed and start the tunnel. Use Connections to inspect its status or stop it.
5. Use the generated SSH configuration on the personal computer as appropriate. Consult Logs for network, authentication and port errors.

## Keys and authentication
The private key stays on the machine initiating an SSH connection; the public key goes in the destination account's ~/.ssh/authorized_keys. Do not share private keys or commit them to a public repository.
A server password authenticates a server account; a private-key passphrase unlocks an encrypted private key. They are different. A generated key can have no passphrase or an optional passphrase that must be supplied when the key is used.
The Linux service invokes local ssh and ssh-keygen commands. A supplied server password can install a public key in the background; an existing login key can also install it. Headless Linux does not provide an interactive terminal installation, so use a password or an existing login key.
Enable the import switch before importing to ~/.ssh. Deleting a selected key removes its private key and corresponding public key, so check your selection and backups first.

## Profiles and data locations
By default the Linux portable edition stores profiles.json, app_settings.json and the ssh_keyfiles_export key directory in data next to the executable. Use --data-dir to select another location. The Windows portable edition uses its own program directory; the configurations are separate.
Profiles can contain server addresses, usernames, passwords or private-key passphrases. Both data and ~/.ssh can contain sensitive information. Review files before migrating them and do not upload private keys or personal configuration to GitHub.
Apply to Connection saves the current profile before applying its settings. Duplicate saves the current profile's settings (including passwords) as a new profile. Reordering profiles affects which one is selected first by default.
In the web interface, Copy Path copies a directory on the Linux host running the service; it does not open a folder on the browser computer. A LAN HTTP page may be unable to access the clipboard, in which case the path can be copied manually from the prompt.

## Browser access and security
By default the Linux service listens on 0.0.0.0:6789 (all IPv4 interfaces) with Token authentication disabled. Other devices can connect through the host IP and port 6789 when network and firewall rules permit.
When authentication is disabled, anyone who can reach the page can manage tunnels, configuration and keys, including deleting keys or stopping the service. 0.0.0.0 is not limited to the LAN; use a trusted network and firewall.
To enable a persistent Token, add a server object to data/app_settings.json while retaining other settings, then restart. The program generates server.accessToken in the same file and reuses it on future starts; the startup terminal also displays it.
```json
{ "server": { "listen": "0.0.0.0", "tokenEnabled": true } }
```
Token authentication defaults to disabled when server.tokenEnabled is missing, empty, null or false. Set server.listen to 127.0.0.1 for local-only access; other values default to all IPv4 interfaces. Command-line options override the file. If app_settings.json has no server field, legacy server_settings.json is migrated while retaining the original file.
HTTP itself is not encrypted, so page data including the Token travels in plaintext. Use an encrypted channel such as SSH across untrusted networks.
Stop Service in the browser stops tunnels managed by that service and exits. Closing the browser page alone does not stop the background service.

## Choosing a tunnel direction
A is a server with a fixed address; B is the Linux host running Tunnel Helper; C is your computer. Replace all example addresses with your actual addresses.
Forward tunnel: B can reach A, and C can reach B. C connects to B's listening port, which forwards to A's SSH service.
Reverse tunnel: B can reach A, but C cannot reach B directly. B creates a loopback listener on A; C connects to A and jumps through that port to B.

## Forward tunnel example
Suppose A is 198.51.100.10, its SSH port is 22, and its user is alice. B is reachable at 192.168.1.20 from C.
On B, use these profile or connection settings:
Reverse Tunnel: No.
Server User: alice; Server Address: 198.51.100.10; Server Port: 22.
Local Start Port / Local Port: 2222, an available listening port on B.
Authentication: key or password. For key authentication, select the private key on B that can log in to A, and provide its passphrase if needed.
SSH Alias: for example, server-a; this names the generated configuration.
After starting, C connects to 192.168.1.20:2222 and logs in to A as alice. C also needs a key or password accepted by A, and must be able to reach B's listening port.

## Reverse tunnel example
Suppose A has fixed address 198.51.100.10, SSH port 22, and user relay. C cannot reach B directly. B's SSH port is 22 and its login user is bob. C can reach A.
On B, use these profile or connection settings:
Reverse Tunnel: Yes.
Server User: relay; Server Address: 198.51.100.10; Server Port: 22. These are A's login details.
Relay Server Listening Port: 22022, an unused port on A. The program listens only on A's 127.0.0.1.
Target Host Login User: bob; Target Host SSH Port: 22. The target host is B, the machine running the Tunnel Helper service and being accessed remotely, not C, the computer opening the browser.
Authentication and optional key: select the method and private key B uses to log in to A. Its public key must already be authorized for relay on A. Provide the private-key passphrase if needed.
SSH Alias: for example, server-b; this names the generated SSH configuration for C.
After starting, copy the generated SSH configuration into C's SSH config and connect to server-b. C must authenticate separately to A and B. The generated configuration does not contain B's private-key path; add IdentityFile entries for keys on C if needed. A must allow TCP forwarding, and B's SSH service must be running.

## Troubleshooting
Connection timed out or was refused: check the server address, SSH port, network and firewall. First confirm the machine running Tunnel Helper can reach that server.
Key authentication failed: check the username, selected private key, public-key installation and file permissions. If the key has a passphrase, verify it. The private key used by the program to reach the relay need not be the same key used by your personal computer.
Port already in use: for forward mode check the listening port on the machine running the program; for reverse mode check the listening port on the relay server. Choose a free port and restart the tunnel.
Reverse tunnel runs but login fails: check the local SSH service and port, TCP-forwarding permission on the relay, and your personal computer's credentials for both the relay and local host. An established tunnel does not guarantee the final SSH login will succeed.
The webpage still shows an old interface: identify the process listening on the port. After updating the executable, stop the old service, start the new program and refresh the browser.

## Start the service
Run this command in the program directory. It listens on port 6789 on all IPv4 interfaces by default:
```bash
./tunnel-helper
```
To choose another port, for example 6790:
```bash
./tunnel-helper --port 6790
```
On a graphical desktop you can also double-click the program. Tray availability depends on your desktop environment.

## Find a running service
List process IDs (PIDs) and their command lines:
```bash
pgrep -af tunnel-helper
```
Check the listener on port 6789, or replace it with your configured port:
```bash
ss -ltnp | grep ':6789'
```
To find the actual executable, replace PID with the process ID:
```bash
readlink -f /proc/PID/exe
```

## Stop the service and its connections
Select “Stop Service” at the top right of the browser page and confirm to stop this service and its managed connections.
On a graphical desktop, choose “Close all connections and exit” from the tray menu. Closing the browser window only closes the page.
For a foreground terminal process, press Ctrl+C.
For a background or double-clicked process, first find the correct PID above, then send SIGTERM (for example, PID 1234):
```bash
kill 1234
```
The service attempts to stop its managed tunnels before exiting. Check the process and port again after a moment. Avoid kill -9, which prevents graceful cleanup.

## Command-line help
```bash
./tunnel-helper --help
```
