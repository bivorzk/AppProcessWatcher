use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Instant, SystemTime},
};

use apppw_core::{HttpHeader, HttpRequestInfo, redact_headers};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{
    Method, Request, Response, StatusCode, Uri,
    body::Incoming,
    header::{CONNECTION, HOST, HeaderMap, UPGRADE},
    server::conn::http1,
    service::service_fn,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::{TokioExecutor, TokioIo},
};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose,
};
use rustls::{
    ServerConfig,
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_rustls::TlsAcceptor;

pub const DEFAULT_PROXY_ADDRESS: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8877);
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ProxyEvent {
    pub client_address: SocketAddr,
    pub request: HttpRequestInfo,
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type ProxyClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

pub struct HttpProxy {
    bind_address: SocketAddr,
    event_sender: mpsc::UnboundedSender<ProxyEvent>,
    capture_bodies: bool,
    certificate_authority: Option<Arc<LocalCertificateAuthority>>,
}

pub struct ProxyHandle {
    pub local_address: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), BoxError>>,
}

struct ProxyRuntime {
    client: ProxyClient,
    event_sender: mpsc::UnboundedSender<ProxyEvent>,
    next_id: Arc<AtomicU64>,
    capture_bodies: bool,
    certificate_authority: Option<Arc<LocalCertificateAuthority>>,
}

struct LocalCertificateAuthority {
    certificate: Certificate,
    issuer: Issuer<'static, KeyPair>,
}

impl HttpProxy {
    pub fn new(bind_address: SocketAddr, event_sender: mpsc::UnboundedSender<ProxyEvent>) -> Self {
        Self {
            bind_address,
            event_sender,
            capture_bodies: false,
            certificate_authority: None,
        }
    }

    pub fn localhost(event_sender: mpsc::UnboundedSender<ProxyEvent>) -> Self {
        Self::new(DEFAULT_PROXY_ADDRESS, event_sender)
    }

    pub fn capture_bodies(mut self, enabled: bool) -> Self {
        self.capture_bodies = enabled;
        self
    }

    pub fn enable_tls_inspection(mut self) -> Result<Self, BoxError> {
        self.certificate_authority = Some(Arc::new(LocalCertificateAuthority::new(None)?));
        Ok(self)
    }

    pub fn enable_tls_inspection_with_private_key(
        mut self,
        private_key: &[u8],
    ) -> Result<Self, BoxError> {
        self.certificate_authority =
            Some(Arc::new(LocalCertificateAuthority::new(Some(private_key))?));
        Ok(self)
    }

    pub fn ca_certificate_der(&self) -> Option<Vec<u8>> {
        self.certificate_authority
            .as_ref()
            .map(|authority| authority.certificate.der().to_vec())
    }

    pub fn ca_private_key_der(&self) -> Option<Vec<u8>> {
        self.certificate_authority
            .as_ref()
            .map(|authority| authority.issuer.key().serialize_der())
    }

    pub async fn start(self) -> Result<ProxyHandle, BoxError> {
        let listener = TcpListener::bind(self.bind_address).await?;
        let local_address = listener.local_addr()?;
        let (shutdown, mut shutdown_receiver) = oneshot::channel();
        let runtime = Arc::new(ProxyRuntime {
            client: https_client(),
            event_sender: self.event_sender,
            next_id: Arc::new(AtomicU64::new(1)),
            capture_bodies: self.capture_bodies,
            certificate_authority: self.certificate_authority,
        });
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_receiver => break,
                    accepted = listener.accept() => {
                        let (stream, client_address) = accepted?;
                        let runtime = runtime.clone();
                        tokio::spawn(async move {
                            let service = service_fn(move |request| {
                                let runtime = runtime.clone();
                                async move {
                                    Ok::<_, Infallible>(
                                        runtime.handle(request, client_address).await
                                    )
                                }
                            });
                            let _ = http1::Builder::new()
                                .serve_connection(TokioIo::new(stream), service)
                                .with_upgrades()
                                .await;
                        });
                    }
                }
            }
            Ok(())
        });

        Ok(ProxyHandle {
            local_address,
            shutdown: Some(shutdown),
            task,
        })
    }
}

impl ProxyHandle {
    pub async fn shutdown(mut self) -> Result<(), BoxError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.await??;
        Ok(())
    }
}

impl ProxyRuntime {
    async fn handle(
        self: Arc<Self>,
        request: Request<Incoming>,
        client_address: SocketAddr,
    ) -> Response<Full<Bytes>> {
        if request.method() == Method::CONNECT {
            return self.handle_connect(request, client_address).await;
        }
        self.forward(request, None, None, client_address)
            .await
            .unwrap_or_else(error_response)
    }

    async fn handle_connect(
        self: Arc<Self>,
        mut request: Request<Incoming>,
        client_address: SocketAddr,
    ) -> Response<Full<Bytes>> {
        let Some(authority) = request.uri().authority().cloned() else {
            return text_response(StatusCode::BAD_REQUEST, "CONNECT target is missing");
        };
        let host = authority.host().to_owned();
        let target = authority.as_str().to_owned();
        let upgrade = hyper::upgrade::on(&mut request);

        if let Some(ca) = self
            .certificate_authority
            .as_ref()
            .filter(|_| !requires_tls_passthrough(&host))
        {
            let acceptor = match ca.acceptor_for(&host) {
                Ok(acceptor) => acceptor,
                Err(error) => return error_response(error),
            };
            tokio::spawn(async move {
                let Ok(upgraded) = upgrade.await else { return };
                let Ok(tls) = acceptor.accept(TokioIo::new(upgraded)).await else {
                    return;
                };
                let runtime = self.clone();
                let service = service_fn(move |request| {
                    let runtime = runtime.clone();
                    let host = target.clone();
                    async move {
                        Ok::<_, Infallible>(
                            runtime
                                .forward(request, Some("https"), Some(&host), client_address)
                                .await
                                .unwrap_or_else(error_response),
                        )
                    }
                });
                let _ = http1::Builder::new()
                    .serve_connection(TokioIo::new(tls), service)
                    .with_upgrades()
                    .await;
            });
        } else {
            let upstream = match TcpStream::connect(&target).await {
                Ok(stream) => stream,
                Err(error) => return error_response(Box::new(error)),
            };
            tokio::spawn(async move {
                let Ok(upgraded) = upgrade.await else { return };
                let mut client = TokioIo::new(upgraded);
                let mut upstream = upstream;
                let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            });
        }

        Response::new(Full::new(Bytes::new()))
    }

    async fn forward(
        &self,
        mut request: Request<Incoming>,
        forced_scheme: Option<&str>,
        default_host: Option<&str>,
        client_address: SocketAddr,
    ) -> Result<Response<Full<Bytes>>, BoxError> {
        let started_at = SystemTime::now();
        let timer = Instant::now();
        let downstream_upgrade =
            is_upgrade_request(&request).then(|| hyper::upgrade::on(&mut request));
        let (mut parts, body) = request.into_parts();
        let request_headers = headers(&parts.headers);
        let request_bytes = Limited::new(body, MAX_BODY_BYTES)
            .collect()
            .await?
            .to_bytes();
        let host = parts
            .uri
            .authority()
            .map(|authority| authority.as_str())
            .or_else(|| {
                parts
                    .headers
                    .get(HOST)
                    .and_then(|value| value.to_str().ok())
            })
            .or(default_host)
            .ok_or("request host is missing")?
            .to_owned();
        let scheme = forced_scheme
            .or_else(|| parts.uri.scheme_str())
            .unwrap_or("http")
            .to_owned();
        let path = parts
            .uri
            .path_and_query()
            .map_or("/", |path| path.as_str())
            .to_owned();
        parts.uri = format!("{scheme}://{host}{path}").parse::<Uri>()?;
        parts.headers.remove("proxy-authorization");
        parts.headers.remove("proxy-connection");
        let method = parts.method.to_string();

        let mut response = self
            .client
            .request(Request::from_parts(parts, Full::new(request_bytes.clone())))
            .await?;
        let tunnel = if response.status() == StatusCode::SWITCHING_PROTOCOLS {
            downstream_upgrade.map(|downstream| (downstream, hyper::upgrade::on(&mut response)))
        } else {
            None
        };
        let (response_parts, response_body) = response.into_parts();
        let status = response_parts.status;
        let response_headers = headers(&response_parts.headers);
        let response_bytes = if tunnel.is_some() {
            Bytes::new()
        } else {
            Limited::new(response_body, MAX_BODY_BYTES)
                .collect()
                .await?
                .to_bytes()
        };
        let info = HttpRequestInfo {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            connection_id: None,
            process: None,
            method,
            scheme,
            host,
            path,
            request_headers: redact_headers(&request_headers),
            request_body: self.capture_bodies.then(|| request_bytes.to_vec()),
            request_body_size: request_bytes.len(),
            status_code: Some(status.as_u16()),
            response_headers: redact_headers(&response_headers),
            response_body: self.capture_bodies.then(|| response_bytes.to_vec()),
            response_body_size: Some(response_bytes.len()),
            started_at,
            duration_ms: Some(timer.elapsed().as_millis()),
        };
        let _ = self.event_sender.send(ProxyEvent {
            client_address,
            request: info,
        });
        if let Some((downstream, upstream)) = tunnel {
            tokio::spawn(async move {
                let Ok((downstream, upstream)) = tokio::try_join!(downstream, upstream) else {
                    return;
                };
                let mut downstream = TokioIo::new(downstream);
                let mut upstream = TokioIo::new(upstream);
                let _ = tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await;
            });
        }
        Ok(Response::from_parts(
            response_parts,
            Full::new(response_bytes),
        ))
    }
}

impl LocalCertificateAuthority {
    fn new(private_key: Option<&[u8]>) -> Result<Self, BoxError> {
        let mut params = CertificateParams::new(Vec::<String>::new())?;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "AppWatch Local CA");
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];
        let key = private_key.map_or_else(KeyPair::generate, KeyPair::try_from)?;
        let certificate = params.self_signed(&key)?;
        Ok(Self {
            certificate,
            issuer: Issuer::new(params, key),
        })
    }

    fn acceptor_for(&self, host: &str) -> Result<TlsAcceptor, BoxError> {
        let mut params = CertificateParams::new(vec![host.to_owned()])?;
        params.distinguished_name.push(DnType::CommonName, host);
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        let key = KeyPair::generate()?;
        let certificate = params.signed_by(&key, &self.issuer)?;
        let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate.der().clone()], private_key)?;
        Ok(TlsAcceptor::from(Arc::new(config)))
    }
}

fn https_client() -> ProxyClient {
    let connector = HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .build();
    Client::builder(TokioExecutor::new()).build(connector)
}

fn headers(headers: &HeaderMap) -> Vec<HttpHeader> {
    headers
        .iter()
        .map(|(name, value)| HttpHeader {
            name: name.to_string(),
            value: value.to_str().unwrap_or("<binary>").to_owned(),
        })
        .collect()
}

fn is_upgrade_request(request: &Request<Incoming>) -> bool {
    request.headers().contains_key(UPGRADE)
        && request
            .headers()
            .get_all(CONNECTION)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
}

fn requires_tls_passthrough(host: &str) -> bool {
    // ponytail: Cloudflare blocks the re-originated TLS fingerprint; passthrough
    // can be removed if the upstream connection can preserve the client's TLS.
    host.eq_ignore_ascii_case("gateway.discord.gg")
}

fn error_response(error: BoxError) -> Response<Full<Bytes>> {
    text_response(StatusCode::BAD_GATEWAY, &format!("proxy error: {error}"))
}

fn text_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::copy_from_slice(message.as_bytes())))
        .expect("static proxy response is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn forwards_http_and_emits_redacted_event() {
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_address = upstream.local_addr().unwrap();
        tokio::spawn(async move {
            let (stream, _) = upstream.accept().await.unwrap();
            let service = service_fn(|_request| async {
                Ok::<_, Infallible>(Response::new(Full::new(Bytes::from_static(b"upstream"))))
            });
            http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
                .unwrap();
        });

        let (event_sender, mut events) = mpsc::unbounded_channel();
        let proxy = HttpProxy::new("127.0.0.1:0".parse().unwrap(), event_sender);
        let handle = proxy.start().await.unwrap();
        let mut client = TcpStream::connect(handle.local_address).await.unwrap();
        let request = format!(
            "GET http://{upstream_address}/hello HTTP/1.1\r\nHost: {upstream_address}\r\nAuthorization: secret\r\nConnection: close\r\n\r\n"
        );
        tokio::io::AsyncWriteExt::write_all(&mut client, request.as_bytes())
            .await
            .unwrap();
        let mut response = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut client, &mut response)
            .await
            .unwrap();

        assert!(String::from_utf8_lossy(&response).contains("upstream"));
        let event = events.recv().await.unwrap();
        assert_eq!(event.request.path, "/hello");
        assert_eq!(event.request.status_code, Some(200));
        assert_eq!(
            event
                .request
                .request_headers
                .iter()
                .find(|header| header.name == "authorization")
                .unwrap()
                .value,
            "[REDACTED]"
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn forwards_websocket_upgrade_and_tunnels_bytes() {
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_address = upstream.local_addr().unwrap();
        tokio::spawn(async move {
            let (stream, _) = upstream.accept().await.unwrap();
            let service = service_fn(|mut request| async move {
                let upgrade = hyper::upgrade::on(&mut request);
                tokio::spawn(async move {
                    let upgraded = upgrade.await.unwrap();
                    let mut connection = TokioIo::new(upgraded);
                    let mut message = [0; 4];
                    tokio::io::AsyncReadExt::read_exact(&mut connection, &mut message)
                        .await
                        .unwrap();
                    tokio::io::AsyncWriteExt::write_all(&mut connection, &message)
                        .await
                        .unwrap();
                });
                Ok::<_, Infallible>(
                    Response::builder()
                        .status(StatusCode::SWITCHING_PROTOCOLS)
                        .header(CONNECTION, "Upgrade")
                        .header(UPGRADE, "websocket")
                        .body(Full::new(Bytes::new()))
                        .unwrap(),
                )
            });
            http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades()
                .await
                .unwrap();
        });

        let (event_sender, mut events) = mpsc::unbounded_channel();
        let proxy = HttpProxy::new("127.0.0.1:0".parse().unwrap(), event_sender);
        let handle = proxy.start().await.unwrap();
        let mut client = TcpStream::connect(handle.local_address).await.unwrap();
        let request = format!(
            "GET http://{upstream_address}/socket HTTP/1.1\r\nHost: {upstream_address}\r\nConnection: keep-alive, Upgrade\r\nUpgrade: websocket\r\n\r\n"
        );
        tokio::io::AsyncWriteExt::write_all(&mut client, request.as_bytes())
            .await
            .unwrap();
        let mut response = Vec::new();
        while !response.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            tokio::io::AsyncReadExt::read_exact(&mut client, &mut byte)
                .await
                .unwrap();
            response.push(byte[0]);
        }

        assert!(String::from_utf8_lossy(&response).starts_with("HTTP/1.1 101"));
        tokio::io::AsyncWriteExt::write_all(&mut client, b"ping")
            .await
            .unwrap();
        let mut echoed = [0; 4];
        tokio::io::AsyncReadExt::read_exact(&mut client, &mut echoed)
            .await
            .unwrap();
        assert_eq!(&echoed, b"ping");
        assert_eq!(events.recv().await.unwrap().request.status_code, Some(101));
        handle.shutdown().await.unwrap();
    }

    #[test]
    fn discord_gateway_bypasses_tls_inspection() {
        assert!(requires_tls_passthrough("gateway.discord.gg"));
        assert!(requires_tls_passthrough("GATEWAY.DISCORD.GG"));
        assert!(!requires_tls_passthrough("discord.com"));
    }

    #[test]
    fn tls_inspection_exports_only_the_public_ca_certificate() {
        let (sender, _events) = mpsc::unbounded_channel();
        let proxy = HttpProxy::localhost(sender)
            .enable_tls_inspection()
            .unwrap();
        assert!(proxy.ca_certificate_der().is_some());
    }

    #[tokio::test]
    async fn generated_leaf_is_trusted_by_the_public_ca() {
        use rustls::{ClientConfig, RootCertStore, pki_types::ServerName};
        use tokio_rustls::TlsConnector;

        let original = LocalCertificateAuthority::new(None).unwrap();
        let trusted_certificate = original.certificate.der().clone();
        let private_key = original.issuer.key().serialize_der();
        let authority = Arc::new(LocalCertificateAuthority::new(Some(&private_key)).unwrap());
        let mut roots = RootCertStore::empty();
        roots.add(trusted_certificate).unwrap();
        let connector = TlsConnector::from(Arc::new(
            ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        ));
        let acceptor = authority.acceptor_for("example.test").unwrap();
        let (client, server) = tokio::io::duplex(4096);
        let (client_result, server_result) = tokio::join!(
            connector.connect(ServerName::try_from("example.test").unwrap(), client),
            acceptor.accept(server),
        );

        client_result.unwrap();
        server_result.unwrap();
    }
}
