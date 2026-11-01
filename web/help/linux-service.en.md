# Linux Service Help

## Start the service
Run this command in the program directory. The default listener is localhost port 6789:
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
