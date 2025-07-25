use hyper_util::client::legacy::connect::{Connected, Connection};
use serde::{Deserialize, Serialize};
use std::{
    io,
    net::SocketAddr,
    path::Path,
    pin::Pin,
    task::{self, Poll},
};
use talpid_types::{
    ErrorExt,
    net::{Endpoint, TransportProtocol, proxy},
};
use tokio::{
    fs,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf},
};

const CURRENT_CONFIG_FILENAME: &str = "api-endpoint.json";

pub trait ConnectionModeProvider: Send {
    /// Initial connection mode
    fn initial(&self) -> ApiConnectionMode;

    /// Request a new connection mode from the provider
    fn rotate(&self) -> impl std::future::Future<Output = ()> + Send;

    /// Receive changes to the connection mode, announced by the provider
    fn receive(&mut self) -> impl std::future::Future<Output = Option<ApiConnectionMode>> + Send;
}

pub struct StaticConnectionModeProvider {
    mode: ApiConnectionMode,
}

impl StaticConnectionModeProvider {
    pub fn new(mode: ApiConnectionMode) -> Self {
        Self { mode }
    }
}

impl ConnectionModeProvider for StaticConnectionModeProvider {
    fn initial(&self) -> ApiConnectionMode {
        self.mode.clone()
    }

    fn rotate(&self) -> impl std::future::Future<Output = ()> + Send {
        futures::future::ready(())
    }

    fn receive(&mut self) -> impl std::future::Future<Output = Option<ApiConnectionMode>> + Send {
        futures::future::pending()
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ApiConnectionMode {
    /// Connect directly to the target.
    Direct,
    /// Connect to the destination via a proxy.
    Proxied(ProxyConfig),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ProxyConfig {
    Shadowsocks(proxy::Shadowsocks),
    Socks5Local(proxy::Socks5Local),
    Socks5Remote(proxy::Socks5Remote),
    EncryptedDnsProxy(mullvad_encrypted_dns_proxy::config::ProxyConfig),
}

impl ProxyConfig {
    /// Returns the remote endpoint describing how to reach the proxy.
    fn get_endpoint(&self) -> Endpoint {
        match self {
            ProxyConfig::Shadowsocks(shadowsocks) => {
                Endpoint::from_socket_address(shadowsocks.endpoint, TransportProtocol::Tcp)
            }
            ProxyConfig::Socks5Local(local) => local.remote_endpoint,
            ProxyConfig::Socks5Remote(remote) => {
                Endpoint::from_socket_address(remote.endpoint, TransportProtocol::Tcp)
            }
            ProxyConfig::EncryptedDnsProxy(proxy) => {
                let addr = SocketAddr::V4(proxy.addr);
                Endpoint::from_socket_address(addr, TransportProtocol::Tcp)
            }
        }
    }
}

impl From<proxy::CustomProxy> for ProxyConfig {
    fn from(value: proxy::CustomProxy) -> Self {
        match value {
            proxy::CustomProxy::Shadowsocks(shadowsocks) => ProxyConfig::Shadowsocks(shadowsocks),
            proxy::CustomProxy::Socks5Local(socks) => ProxyConfig::Socks5Local(socks),
            proxy::CustomProxy::Socks5Remote(socks) => ProxyConfig::Socks5Remote(socks),
        }
    }
}

impl From<mullvad_encrypted_dns_proxy::config::ProxyConfig> for ProxyConfig {
    fn from(value: mullvad_encrypted_dns_proxy::config::ProxyConfig) -> Self {
        ProxyConfig::EncryptedDnsProxy(value)
    }
}

impl ApiConnectionMode {
    /// Reads the proxy config from `CURRENT_CONFIG_FILENAME`.
    /// This returns `ApiConnectionMode::Direct` if reading from disk fails for any reason.
    pub async fn try_from_cache(cache_dir: &Path) -> Self {
        Self::from_cache(cache_dir).await.unwrap_or_else(|error| {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to read API endpoint cache")
            );
            ApiConnectionMode::Direct
        })
    }

    /// Reads the proxy config from `CURRENT_CONFIG_FILENAME`.
    /// If the file does not exist, this returns `Ok(ApiConnectionMode::Direct)`.
    async fn from_cache(cache_dir: &Path) -> io::Result<Self> {
        let path = cache_dir.join(CURRENT_CONFIG_FILENAME);
        match fs::read_to_string(path).await {
            Ok(s) => serde_json::from_str(&s).map_err(|error| {
                log::error!(
                    "{}",
                    error.display_chain_with_msg(&format!(
                        "Failed to deserialize \"{CURRENT_CONFIG_FILENAME}\""
                    ))
                );
                io::Error::other("deserialization failed")
            }),
            Err(error) => {
                if error.kind() == io::ErrorKind::NotFound {
                    Ok(ApiConnectionMode::Direct)
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Stores this config to `CURRENT_CONFIG_FILENAME`.
    pub async fn save(&self, cache_dir: &Path) -> io::Result<()> {
        let mut file = mullvad_fs::AtomicFile::new(cache_dir.join(CURRENT_CONFIG_FILENAME)).await?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|_| io::Error::other("serialization failed"))?;
        file.write_all(json.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.finalize().await
    }

    /// Attempts to remove `CURRENT_CONFIG_FILENAME`, if it exists.
    pub async fn try_delete_cache(cache_dir: &Path) {
        let path = cache_dir.join(CURRENT_CONFIG_FILENAME);
        if let Err(err) = fs::remove_file(path).await {
            if err.kind() != std::io::ErrorKind::NotFound {
                log::error!(
                    "{}",
                    err.display_chain_with_msg("Failed to remove old API config")
                );
            }
        }
    }

    /// Returns the remote endpoint required to reach the API, or `None` for
    /// `ApiConnectionMode::Direct`.
    pub fn get_endpoint(&self) -> Option<Endpoint> {
        match self {
            ApiConnectionMode::Direct => None,
            ApiConnectionMode::Proxied(proxy_config) => Some(proxy_config.get_endpoint()),
        }
    }

    pub fn is_proxy(&self) -> bool {
        *self != ApiConnectionMode::Direct
    }

    pub fn into_provider(self) -> StaticConnectionModeProvider {
        StaticConnectionModeProvider::new(self)
    }
}

/// Implements `Connection` by wrapping a type.
pub struct ConnectionDecorator<T: AsyncRead + AsyncWrite>(pub T);

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for ConnectionDecorator<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for ConnectionDecorator<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl<T: AsyncRead + AsyncWrite> Connection for ConnectionDecorator<T> {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

trait ConnectionMullvad: AsyncRead + AsyncWrite + Unpin + Connection + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Connection + Send> ConnectionMullvad for T {}

/// Stream that represents a Mullvad API connection
pub struct ApiConnection(Box<dyn ConnectionMullvad>);

impl ApiConnection {
    pub fn new<T: AsyncRead + AsyncWrite + Unpin + Connection + Send + 'static>(
        conn: Box<T>,
    ) -> Self {
        Self(conn)
    }
}

impl AsyncRead for ApiConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for ApiConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl Connection for ApiConnection {
    fn connected(&self) -> Connected {
        self.0.connected()
    }
}
