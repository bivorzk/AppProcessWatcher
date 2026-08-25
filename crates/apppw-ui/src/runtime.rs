use std::{
    collections::HashMap,
    env, fs,
    net::SocketAddr,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

use apppw_capture::{ConnectionTracker, PacketCapture};
use apppw_core::{
    CapturedPacket, ConnectionDirection, HttpRequestInfo, IpVersion, NetworkConnection,
    NetworkEvent as CoreEvent, NetworkProtocol, ProcessInfo, TrafficCorrelator,
};
use apppw_proxy::{HttpProxy, ProxyEvent};
use apppw_storage::Database;
use apppw_windows::{
    ProcessCollector, SocketProcessResolver, application_icon, open_application_pids,
    relaunch_open_apps_through_proxy, relaunch_through_proxy,
};
use eframe::egui;
use tokio::sync::oneshot;

use crate::domain::{EventKind, NetworkEvent, Process};

pub enum RuntimeEvent {
    Connection(NetworkEvent),
    Closed(u64),
    Processes(Vec<Process>),
    Warning(String),
    Error(String),
}

enum RuntimeAction {
    Relaunch(Option<u32>),
    TrustCertificate,
}

pub struct Runtime {
    pub processes: Vec<Process>,
    events: Receiver<RuntimeEvent>,
    actions: Sender<RuntimeAction>,
    shutdown: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

struct ProxyWorker {
    address: SocketAddr,
    certificate_path: PathBuf,
    events: Receiver<ProxyEvent>,
    shutdown: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl Runtime {
    pub fn start(context: egui::Context) -> Self {
        let (event_sender, events) = mpsc::channel();
        let (shutdown, shutdown_receiver) = mpsc::channel();
        let (actions, action_receiver) = mpsc::channel();
        let core_processes = match ProcessCollector.list_processes() {
            Ok(processes) => processes,
            Err(error) => {
                let _ = event_sender.send(RuntimeEvent::Error(format!(
                    "Could not list Windows processes: {error}"
                )));
                Vec::new()
            }
        };
        let processes = to_ui_processes(&core_processes);
        let worker = thread::spawn(move || {
            run_components(
                event_sender,
                context,
                core_processes,
                shutdown_receiver,
                action_receiver,
            )
        });

        Self {
            processes,
            events,
            actions,
            shutdown,
            worker: Some(worker),
        }
    }

    pub fn try_recv(&self) -> Result<RuntimeEvent, TryRecvError> {
        self.events.try_recv()
    }

    pub fn relaunch_through_proxy(&self, pid: Option<u32>) {
        let _ = self.actions.send(RuntimeAction::Relaunch(pid));
    }

    pub fn trust_https_certificate(&self) {
        let _ = self.actions.send(RuntimeAction::TrustCertificate);
    }
}

fn prioritise_processes(processes: &mut [Process]) {
    processes.sort_by_key(|process| (!process.application, process.name.to_lowercase()));
}

fn to_ui_processes(processes: &[ProcessInfo]) -> Vec<Process> {
    let application_pids = open_application_pids().unwrap_or_default();
    let mut processes = processes
        .iter()
        .map(|process| {
            let application = application_pids.contains(&process.pid);
            Process {
                pid: process.pid,
                name: process.name.clone(),
                active: true,
                application,
                icon_rgba: if application {
                    process
                        .executable_path
                        .as_deref()
                        .and_then(application_icon)
                } else {
                    None
                },
            }
        })
        .collect::<Vec<_>>();
    prioritise_processes(&mut processes);
    processes
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_components(
    sender: Sender<RuntimeEvent>,
    context: egui::Context,
    mut processes: Vec<ProcessInfo>,
    shutdown: Receiver<()>,
    actions: Receiver<RuntimeAction>,
) {
    let capture = match PacketCapture::start() {
        Ok(capture) => Some(capture),
        Err(error) => {
            let _ = sender.send(RuntimeEvent::Error(format!(
                "WinDivert capture could not start: {error}"
            )));
            None
        }
    };
    let proxy = match ProxyWorker::start() {
        Ok(proxy) => {
            let _ = sender.send(RuntimeEvent::Warning(format!(
                "HTTP/HTTPS proxy listening on {}. Configure an application to use this address and trust {} to inspect HTTPS requests. HTTP/3/QUIC is not intercepted.",
                proxy.address,
                proxy.certificate_path.display(),
            )));
            Some(proxy)
        }
        Err(error) => {
            let _ = sender.send(RuntimeEvent::Warning(error));
            None
        }
    };
    let database = Database::open("appwatch.db").ok();
    if database.is_none() {
        let _ = sender.send(RuntimeEvent::Warning(
            "Traffic is live, but appwatch.db could not be opened.".into(),
        ));
    }

    let resolver = SocketProcessResolver;
    let mut tracker = ConnectionTracker::new();
    let mut correlator = TrafficCorrelator::default();
    let mut packet_counts = HashMap::<u64, u64>::new();
    let mut last_process_refresh = Instant::now();

    while shutdown.try_recv().is_err() {
        let mut changed = false;
        if let Some(capture) = &capture {
            match capture.recv_timeout(Duration::from_millis(20)) {
                Ok(CoreEvent::PacketCaptured(mut packet)) => {
                    packet.process = resolve_packet_process(&resolver, &processes, &packet);
                    for event in tracker.process_packet(packet) {
                        handle_network_event(
                            event,
                            &sender,
                            database.as_ref(),
                            &mut correlator,
                            &mut packet_counts,
                        );
                        changed = true;
                    }
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            thread::sleep(Duration::from_millis(20));
        }

        if let Some(proxy) = &proxy {
            while let Ok(event) = proxy.events.try_recv() {
                handle_proxy_event(
                    event,
                    proxy.address,
                    &resolver,
                    &processes,
                    &mut correlator,
                    database.as_ref(),
                    &sender,
                );
                changed = true;
            }
        }
        while let Ok(action) = actions.try_recv() {
            let Some(proxy) = &proxy else {
                let _ = sender.send(RuntimeEvent::Error(
                    "Cannot relaunch applications because the proxy is offline.".into(),
                ));
                continue;
            };
            let message = match action {
                RuntimeAction::Relaunch(pid) => {
                    let proxy_url = format!("http://{}", proxy.address);
                    match pid {
                        Some(pid) => relaunch_through_proxy(pid, &proxy_url)
                            .map(|()| format!("Relaunched PID {pid} through {proxy_url}."))
                            .map_err(|error| format!("Proxy relaunch failed: {error}")),
                        None => relaunch_open_apps_through_proxy(&proxy_url)
                            .map(|summary| {
                                format!(
                                    "Relaunched {} open app(s) through the proxy; {} could not be relaunched.",
                                    summary.relaunched, summary.failed
                                )
                            })
                            .map_err(|error| format!("Proxy relaunch failed: {error}")),
                    }
                }
                RuntimeAction::TrustCertificate => trust_https_certificate(&proxy.certificate_path)
                    .map(|()| "AppWatch HTTPS certificate trusted for the current Windows user. Restart the target application before capturing HTTPS.".into()),
            };
            let _ = sender.send(match message {
                Ok(message) => RuntimeEvent::Warning(message),
                Err(error) => RuntimeEvent::Error(error),
            });
            context.request_repaint();
        }
        if last_process_refresh.elapsed() >= Duration::from_secs(1) {
            if let Ok(current_processes) = ProcessCollector.list_processes() {
                processes = current_processes;
                let _ = sender.send(RuntimeEvent::Processes(to_ui_processes(&processes)));
                changed = true;
            }
            last_process_refresh = Instant::now();
        }
        if changed {
            context.request_repaint();
        }
    }

    drop(proxy);
    drop(capture);
    drop(database);
}

fn handle_network_event(
    event: CoreEvent,
    sender: &Sender<RuntimeEvent>,
    database: Option<&Database>,
    correlator: &mut TrafficCorrelator,
    packet_counts: &mut HashMap<u64, u64>,
) {
    match event {
        CoreEvent::PacketCaptured(_) => {}
        CoreEvent::ConnectionOpened(connection) | CoreEvent::ConnectionUpdated(connection) => {
            let count = packet_counts.entry(connection.id).or_default();
            *count = count.saturating_add(1);
            correlator.observe_connection(connection.clone(), SystemTime::now());
            if let Some(database) = database {
                let _ = database.save_connection(&connection);
            }
            let _ = sender.send(RuntimeEvent::Connection(to_ui_connection(
                &connection,
                *count,
            )));
        }
        CoreEvent::ConnectionClosed(id) => {
            packet_counts.remove(&id);
            let _ = sender.send(RuntimeEvent::Closed(id));
        }
    }
}

fn handle_proxy_event(
    event: ProxyEvent,
    proxy_address: SocketAddr,
    resolver: &SocketProcessResolver,
    processes: &[ProcessInfo],
    correlator: &mut TrafficCorrelator,
    database: Option<&Database>,
    sender: &Sender<RuntimeEvent>,
) {
    let mut request = event.request;
    let correlated = correlator.correlate_http_request(
        &mut request,
        NetworkProtocol::TCP,
        event.client_address,
        proxy_address,
        SystemTime::now(),
    );
    if !correlated {
        request.process = resolve_process(
            resolver,
            processes,
            event.client_address,
            proxy_address,
            NetworkProtocol::TCP,
        );
    }
    if let Some(database) = database {
        let _ = database.save_http_request(&request);
    }
    let _ = sender.send(RuntimeEvent::Connection(to_ui_http(
        &request,
        event.client_address,
        proxy_address,
    )));
}

fn resolve_packet_process(
    resolver: &SocketProcessResolver,
    processes: &[ProcessInfo],
    packet: &CapturedPacket,
) -> Option<ProcessInfo> {
    let source_port = packet.source_port?;
    let destination_port = packet.destination_port?;
    let (local, remote) = match packet.direction {
        ConnectionDirection::Outbound => (
            SocketAddr::new(packet.source_ip, source_port),
            SocketAddr::new(packet.destination_ip, destination_port),
        ),
        ConnectionDirection::Inbound => (
            SocketAddr::new(packet.destination_ip, destination_port),
            SocketAddr::new(packet.source_ip, source_port),
        ),
    };
    resolve_process(resolver, processes, local, remote, packet.protocol)
}

fn resolve_process(
    resolver: &SocketProcessResolver,
    processes: &[ProcessInfo],
    local: SocketAddr,
    remote: SocketAddr,
    protocol: NetworkProtocol,
) -> Option<ProcessInfo> {
    let connection = NetworkConnection {
        id: 0,
        process: None,
        protocol,
        ip_version: if local.is_ipv4() {
            IpVersion::V4
        } else {
            IpVersion::V6
        },
        direction: ConnectionDirection::Outbound,
        local_ip: local.ip(),
        local_port: local.port(),
        remote_ip: remote.ip(),
        remote_port: remote.port(),
        hostname: None,
        bytes_sent: 0,
        bytes_received: 0,
    };
    let pid = resolver.resolve_pid(&connection)?;
    processes
        .iter()
        .find(|process| process.pid == pid)
        .cloned()
        .or_else(|| resolver.resolve(&connection))
}

fn to_ui_connection(connection: &NetworkConnection, packet_count: u64) -> NetworkEvent {
    let kind = match connection.protocol {
        NetworkProtocol::HTTP => EventKind::Http,
        NetworkProtocol::HTTPS => EventKind::Https,
        NetworkProtocol::TLS => EventKind::Tls,
        NetworkProtocol::TCP => EventKind::Tcp,
        NetworkProtocol::UDP => EventKind::Udp,
        NetworkProtocol::QUIC => EventKind::Quic,
        NetworkProtocol::UNKNOWN => EventKind::Unknown,
    };
    let process = connection.process.as_ref();
    NetworkEvent {
        id: connection.id,
        pid: process.map(|process| process.pid),
        process: process.map_or_else(|| "Unknown process".into(), |process| process.name.clone()),
        method: kind
            .label()
            .split_whitespace()
            .next()
            .unwrap_or("Unknown")
            .into(),
        host: connection
            .hostname
            .clone()
            .unwrap_or_else(|| connection.remote_ip.to_string()),
        path: String::new(),
        status: None,
        kind,
        bytes_sent: connection.bytes_sent,
        bytes_received: connection.bytes_received,
        packet_count,
        duration_ms: None,
        local: SocketAddr::new(connection.local_ip, connection.local_port).to_string(),
        remote: connection.remote_address(),
        request_headers: Vec::new(),
        response_headers: Vec::new(),
        request_body: None,
        response_body: None,
        ja4: None,
        ja4_outlier: false,
    }
}

fn to_ui_http(request: &HttpRequestInfo, local: SocketAddr, remote: SocketAddr) -> NetworkEvent {
    let process = request.process.as_ref();
    NetworkEvent {
        id: request.id | (1_u64 << 63),
        pid: process.map(|process| process.pid),
        process: process.map_or_else(|| "Unknown process".into(), |process| process.name.clone()),
        method: request.method.clone(),
        host: request.host.clone(),
        path: request.path.clone(),
        status: request.status_code,
        kind: if request.scheme.eq_ignore_ascii_case("https") {
            EventKind::Https
        } else {
            EventKind::Http
        },
        bytes_sent: request.request_body_size as u64,
        bytes_received: request.response_body_size.unwrap_or_default() as u64,
        packet_count: 0,
        duration_ms: request.duration_ms.and_then(|value| value.try_into().ok()),
        local: local.to_string(),
        remote: remote.to_string(),
        request_headers: request.request_headers.clone(),
        response_headers: request.response_headers.clone(),
        request_body: request.request_body.clone(),
        response_body: request.response_body.clone(),
        ja4: request.ja4.clone(),
        ja4_outlier: false,
    }
}

impl ProxyWorker {
    fn start() -> Result<Self, String> {
        let (events_sender, events) = mpsc::channel();
        let (started_sender, started) = mpsc::sync_channel(1);
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let worker = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = started_sender
                        .send(Err(format!("HTTP proxy runtime could not start: {error}")));
                    return;
                }
            };
            runtime.block_on(async move {
                let (proxy_sender, mut proxy_events) = tokio::sync::mpsc::unbounded_channel();
                let key_path = appwatch_directory().join("AppWatch HTTPS Inspection CA.key");
                let saved_key = fs::read(&key_path).ok();
                let proxy_with_saved_key = saved_key.as_deref().and_then(|key| {
                    HttpProxy::localhost(proxy_sender.clone())
                        .capture_bodies(true)
                        .enable_tls_inspection_with_private_key(key)
                        .ok()
                });
                let proxy = match proxy_with_saved_key.map(Ok).unwrap_or_else(|| {
                    HttpProxy::localhost(proxy_sender.clone())
                        .capture_bodies(true)
                        .enable_tls_inspection()
                }) {
                    Ok(proxy) => proxy,
                    Err(error) => {
                        let _ = started_sender
                            .send(Err(format!("HTTPS inspection could not start: {error}")));
                        return;
                    }
                };
                if let Err(error) = save_ca_private_key(&proxy, &key_path) {
                    let _ = started_sender.send(Err(format!(
                        "HTTPS inspection key could not be saved: {error}"
                    )));
                    return;
                }
                let certificate_path = match export_ca_certificate(&proxy) {
                    Ok(path) => path,
                    Err(error) => {
                        let _ = started_sender.send(Err(format!(
                            "HTTPS inspection certificate could not be exported: {error}"
                        )));
                        return;
                    }
                };
                let handle = match proxy.start().await {
                    Ok(handle) => handle,
                    Err(error) => {
                        let _ = started_sender
                            .send(Err(format!("HTTP proxy could not start: {error}")));
                        return;
                    }
                };
                let address = handle.local_address;
                let _ = started_sender.send(Ok((address, certificate_path)));
                let mut shutdown_receiver = shutdown_receiver;
                loop {
                    tokio::select! {
                        _ = &mut shutdown_receiver => break,
                        event = proxy_events.recv() => {
                            let Some(event) = event else { break };
                            if events_sender.send(event).is_err() { break; }
                        }
                    }
                }
                let _ = handle.shutdown().await;
            });
        });
        let (address, certificate_path) = started
            .recv()
            .map_err(|_| "HTTP proxy stopped during startup".to_owned())??;
        Ok(Self {
            address,
            certificate_path,
            events,
            shutdown: Some(shutdown),
            worker: Some(worker),
        })
    }
}

fn export_ca_certificate(proxy: &HttpProxy) -> std::io::Result<PathBuf> {
    let directory = appwatch_directory();
    fs::create_dir_all(&directory)?;
    let path = directory.join("AppWatch HTTPS Inspection CA.cer");
    let certificate = proxy
        .ca_certificate_der()
        .expect("TLS inspection always provides a CA certificate");
    fs::write(&path, certificate)?;
    Ok(path)
}

fn appwatch_directory() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("AppWatch")
}

fn save_ca_private_key(proxy: &HttpProxy, path: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new(".")))?;
    fs::write(
        path,
        proxy
            .ca_private_key_der()
            .expect("TLS inspection always provides a private key"),
    )
}

fn trust_https_certificate(path: &std::path::Path) -> Result<(), String> {
    let output = certificate_install_command(path)
        .output()
        .map_err(|error| format!("Could not start Windows certificate installer: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        Err(format!(
            "Windows could not trust the AppWatch HTTPS certificate: {}",
            detail
        ))
    }
}

fn certificate_install_command(path: &std::path::Path) -> Command {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new("certutil");
    command
        .args(["-user", "-f", "-addstore", "Root"])
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW);
    command
}

impl Drop for ProxyWorker {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apppw_core::HttpHeader;

    #[test]
    fn applications_are_listed_before_background_processes() {
        let mut processes = vec![
            Process {
                pid: 1,
                name: "svchost.exe".into(),
                active: true,
                application: false,
                icon_rgba: None,
            },
            Process {
                pid: 2,
                name: "Browser.exe".into(),
                active: true,
                application: true,
                icon_rgba: None,
            },
        ];

        prioritise_processes(&mut processes);
        assert_eq!(processes[0].pid, 2);
    }

    #[test]
    fn certificate_install_targets_only_the_current_user_root_store() {
        let command = certificate_install_command(std::path::Path::new("certificate.cer"));
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>();

        assert_eq!(
            arguments,
            ["-user", "-f", "-addstore", "Root", "certificate.cer"]
        );
    }

    #[test]
    fn connection_conversion_does_not_invent_http_data() {
        let connection = NetworkConnection {
            id: 7,
            process: None,
            protocol: NetworkProtocol::TCP,
            ip_version: IpVersion::V4,
            direction: ConnectionDirection::Outbound,
            local_ip: "127.0.0.1".parse().unwrap(),
            local_port: 50_000,
            remote_ip: "1.1.1.1".parse().unwrap(),
            remote_port: 443,
            hostname: None,
            bytes_sent: 100,
            bytes_received: 200,
        };
        let event = to_ui_connection(&connection, 2);
        assert_eq!(event.kind, EventKind::Tcp);
        assert_eq!(event.status, None);
        assert!(event.path.is_empty());
        assert_eq!(event.process, "Unknown process");
    }

    #[test]
    fn proxy_request_conversion_preserves_observed_http_data() {
        let request = HttpRequestInfo {
            id: 3,
            connection_id: None,
            process: None,
            method: "GET".into(),
            scheme: "http".into(),
            host: "example.test".into(),
            path: "/health".into(),
            request_headers: vec![HttpHeader {
                name: "accept".into(),
                value: "application/json".into(),
            }],
            request_body: None,
            request_body_size: 0,
            status_code: Some(204),
            response_headers: Vec::new(),
            response_body: Some(Vec::new()),
            response_body_size: Some(0),
            started_at: SystemTime::now(),
            duration_ms: Some(12),
            ja4: Some("t13d1516h2_example_example".into()),
        };
        let event = to_ui_http(
            &request,
            "127.0.0.1:50000".parse().unwrap(),
            "127.0.0.1:8877".parse().unwrap(),
        );
        assert_eq!(event.kind, EventKind::Http);
        assert_eq!(event.status, Some(204));
        assert_eq!(event.request_headers, request.request_headers);
    }
}
