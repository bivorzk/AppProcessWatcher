# AppProcessWatcher v0.2

> A Windows per-process network traffic monitor and HTTP(S) inspection tool
> built with Rust, egui, Tokio, WinDivert, Hyper and SQLite.

The interface is still in development.

![AppProcessWatcher screenshot](docs/screenshot.png)

## Features

- Monitor network traffic by running process
- View live upload and download statistics
- Observe TCP, UDP and QUIC traffic
- Inspect active connections and HTTP requests
- Forward WebSocket upgrade connections through the proxy
- Inspect HTTPS through a local man-in-the-middle (MITM) proxy
- Use a persistent local certificate authority (CA)
- Calculate and store JA4 TLS fingerprints, and flag fingerprints that differ
   from a process's baseline
- Detect Chromium, Electron, CEF, Qt WebEngine and WebView2 applications
- Store local traffic history in SQLite

## How It Works

AppProcessWatcher combines packet capture with a local HTTP(S) proxy.

WinDivert captures network traffic, and AppProcessWatcher associates packets
with running Windows processes. This provides per-process traffic statistics
without requiring applications to use the proxy.

HTTP(S) inspection is separate. Applications must use the AppProcessWatcher
proxy before application-layer requests and responses can be inspected.

```text
Target application
        |
        +-- Raw network traffic --> WinDivert --> Process attribution
        |
        +-- HTTP(S) traffic -----> AppWatch proxy --> Remote server
                                      |
                                      +--> Request and response inspection
```

## Requirements

- Windows 10 or 11
- Administrator privileges for WinDivert packet capture
- A Rust toolchain when building from source
- The AppWatch CA certificate trusted for HTTPS inspection

## Usage

1. Launch AppWatch with administrator privileges to enable TCP, UDP and QUIC
   packet capture.
2. Select an application or process to inspect its connections and live
   upload and download traffic.
3. Open **Proxy > Trust HTTPS certificate...** and confirm the security warning.
4. Relaunch the target application through the proxy at
   `http://127.0.0.1:8877`.

HTTPS inspection works only for applications using the AppWatch proxy.
HTTP/3 and QUIC payloads cannot currently be decrypted. Binary or encoded
responses are hidden by default but can be revealed from the response panel.
WebSocket upgrade connections are forwarded by the proxy, but their tunneled
payloads are not decoded as HTTP messages.
The Discord gateway at `gateway.discord.gg` bypasses TLS interception because
Discord rejects the proxy's re-originated TLS fingerprint.

## Chromium-based Applications

When relaunching an application through the proxy, AppWatch checks the
executable's directory and its immediate subdirectories for Chromium,
Electron or CEF files. Detected applications receive `--proxy-server` and
`--disable-quic` command-line switches.

AppWatch also detects Qt WebEngine through `Qt5WebEngineCore.dll` or
`Qt6WebEngineCore.dll`, and WebView2 through `WebView2Loader.dll` in common
runtime locations. Qt WebEngine receives its flags through
`QTWEBENGINE_CHROMIUM_FLAGS`; WebView2 receives them through
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`.

Detection is best-effort because applications can package or configure these
runtimes differently.

## HTTPS Inspection

AppProcessWatcher uses a locally generated CA to inspect HTTPS traffic passing
through its proxy. Its private key and public certificate persist under
`%LOCALAPPDATA%\AppWatch` so the trusted certificate remains valid after the
application restarts.

The CA must be explicitly trusted through the Proxy menu. Trusting it allows
AppWatch to decrypt HTTPS traffic for the current Windows user. Only enable
this on a device you control, and remove the certificate from the Windows
trusted root store when you no longer use HTTPS inspection.

Applications using certificate pinning may reject AppProcessWatcher's
generated certificates and cannot be inspected through this proxy.

## Limitations

- Windows only
- HTTP/3 and QUIC payloads cannot currently be decrypted
- HTTPS inspection requires the target application to use the AppWatch proxy
- JA4 fingerprints are available only for TLS connections that reach the
   inspecting proxy
- WebSocket payloads are tunneled but are not decoded or displayed as HTTP
   request and response bodies
- `gateway.discord.gg` bypasses TLS inspection and is not decrypted
- Applications using certificate pinning may reject intercepted HTTPS traffic
- Some applications may ignore proxy configuration
- Protocols other than HTTP(S) can be observed at the network level, but their
  application payloads are not decoded

## Building

```powershell
git clone https://github.com/bivorzk/AppProcessWatcher.git
cd AppProcessWatcher
cargo build --release
cargo run --release -p apppw-ui
```
