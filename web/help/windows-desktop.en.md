# Tunnel Helper Windows Help

## Overview
Tunnel Helper manages SSH keys, connection profiles, and SSH tunnels. The Windows desktop edition uses the system OpenSSH client.

## Key management
User keys are stored in the current user's ~/.ssh; portable keys are stored in ssh_keyfiles_export beside the program. Open Folder opens the directory in Explorer. A public key bundle contains public keys only. Back up keys before deleting them.

## Generate and install keys
An optional passphrase protects a generated private key. Install the public key using a server password or an existing login key. Never send the private key to the server. Verify key login after installation.

## Profiles
Profiles store the server address, ports, login user, and authentication. Duplicate creates and saves a copy, including any server password and private-key passphrase. Apply to Connection saves the profile first. profiles.json can contain sensitive information and should not be shared publicly.

## Connect and exit
Select a profile in Connection, check the local port and authentication, and start the tunnel. Use Connections to inspect or stop tunnels. Minimize hides the window in the system tray; click the tray icon to restore it. If the tray is unavailable, ordinary minimize is used. The tray menu can stop connections and exit. The window close button also stops managed connections before exiting.

## Troubleshooting
For key authentication failures, check the server user, installed public key, private-key permissions, and passphrase. For a busy port, check other listeners. For password authentication failures, check the account and password. SSH output in Logs can help diagnose failures; remove sensitive details before sharing it.
