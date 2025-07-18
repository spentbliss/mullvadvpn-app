use crate::net::{Endpoint, GenericTunnelOptions, TransportProtocol};
use base64::{Engine, engine::general_purpose::STANDARD};
use ipnetwork::IpNetwork;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    cmp, fmt,
    hash::{Hash, Hasher},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Tunnel parameters required to start a `WireguardMonitor`.
/// See [`crate::net::TunnelParameters`].
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct TunnelParameters {
    pub connection: ConnectionConfig,
    pub options: TunnelOptions,
    pub generic_options: GenericTunnelOptions,
    pub obfuscation: Option<super::obfuscation::ObfuscatorConfig>,
}

impl TunnelParameters {
    /// Returns the endpoint that will be connected to
    pub fn get_next_hop_endpoint(&self) -> Endpoint {
        self.obfuscation
            .as_ref()
            .map(|proxy| proxy.get_obfuscator_endpoint())
            .unwrap_or_else(|| self.connection.get_endpoint())
    }
}

/// Connection-specific configuration in [`TunnelParameters`].
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ConnectionConfig {
    pub tunnel: TunnelConfig,
    pub peer: PeerConfig,
    pub exit_peer: Option<PeerConfig>,
    /// Gateway used by the tunnel (a private address).
    pub ipv4_gateway: Ipv4Addr,
    pub ipv6_gateway: Option<Ipv6Addr>,
    #[cfg(target_os = "linux")]
    pub fwmark: Option<u32>,
}

impl ConnectionConfig {
    pub fn get_endpoint(&self) -> Endpoint {
        Endpoint {
            address: self.peer.endpoint,
            protocol: TransportProtocol::Udp,
        }
    }

    pub fn get_exit_endpoint(&self) -> Option<Endpoint> {
        self.exit_peer.as_ref().map(|peer| Endpoint {
            address: peer.endpoint,
            protocol: TransportProtocol::Udp,
        })
    }
}

#[derive(Clone, Eq, PartialEq, Deserialize, Serialize, Debug, Hash)]
pub struct PeerConfig {
    /// Peer's public key.
    pub public_key: PublicKey,
    /// Addresses that may be routed to the peer. Use `0.0.0.0/0` to route everything.
    pub allowed_ips: Vec<IpNetwork>,
    /// IP address of the WireGuard server.
    pub endpoint: SocketAddr,
    /// Preshared key (PSK). The PSK should never be persisted, so it does not serialize
    /// or deserialize. A PSK is only used with quantum-resistant tunnels and are then
    /// ephemeral and living in memory only.
    #[serde(skip)]
    pub psk: Option<PresharedKey>,
    /// Enable constant packet sizes for `entry_peer``
    #[cfg(daita)]
    #[serde(skip)]
    pub constant_packet_size: bool,
}

#[derive(Clone, Eq, PartialEq, Deserialize, Serialize, Debug)]
pub struct TunnelConfig {
    pub private_key: PrivateKey,
    /// Local IP addresses associated with a key pair.
    pub addresses: Vec<IpAddr>,
}

/// Options in [`TunnelParameters`] that apply to any WireGuard connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelOptions {
    /// MTU for the wireguard tunnel
    pub mtu: Option<u16>,
    /// Perform PQ-safe PSK exchange when connecting
    pub quantum_resistant: bool,
    /// Enable DAITA during tunnel config
    #[cfg(daita)]
    pub daita: bool,
}

/// Wireguard x25519 private key
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct PrivateKey(x25519_dalek::StaticSecret);

impl PrivateKey {
    /// Get private key as bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn new_from_random() -> Self {
        PrivateKey(x25519_dalek::StaticSecret::random())
    }

    /// Generate public key from private key
    pub fn public_key(&self) -> PublicKey {
        PublicKey::from(&self.0)
    }

    pub fn to_base64(&self) -> String {
        STANDARD.encode(self.0.to_bytes())
    }

    pub fn from_base64(key: &str) -> Result<Self, InvalidKey> {
        key_from_base64(key)
    }
}

impl From<[u8; 32]> for PrivateKey {
    fn from(bytes: [u8; 32]) -> Self {
        Self(x25519_dalek::StaticSecret::from(bytes))
    }
}

impl cmp::PartialEq for PrivateKey {
    fn eq(&self, other: &PrivateKey) -> bool {
        self.0.to_bytes() == other.0.to_bytes()
    }
}

impl cmp::Eq for PrivateKey {}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self)
    }
}

impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &STANDARD.encode((self.0).to_bytes()))
    }
}

impl Serialize for PrivateKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_key(&self.0.to_bytes(), serializer)
    }
}

impl<'de> Deserialize<'de> for PrivateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_key(deserializer)
    }
}

/// Wireguard x25519 public key
#[derive(Clone)]
pub struct PublicKey(x25519_dalek::PublicKey);

/// Error returned if an input represents an invalid key
#[derive(Debug, thiserror::Error)]
pub enum InvalidKey {
    #[error("Invalid key: {0}")]
    Format(#[from] base64::DecodeError),
    #[error("Invalid key length: {0}")]
    Length(usize),
}

impl PublicKey {
    /// Get the public key as bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    pub fn to_base64(&self) -> String {
        STANDARD.encode(self.as_bytes())
    }

    pub fn from_base64(key: &str) -> Result<Self, InvalidKey> {
        key_from_base64(key)
    }
}

impl<'a> From<&'a x25519_dalek::StaticSecret> for PublicKey {
    fn from(private_key: &'a x25519_dalek::StaticSecret) -> PublicKey {
        PublicKey(x25519_dalek::PublicKey::from(private_key))
    }
}

impl From<[u8; 32]> for PublicKey {
    fn from(public_key: [u8; 32]) -> PublicKey {
        PublicKey(x25519_dalek::PublicKey::from(public_key))
    }
}

impl TryFrom<&[u8]> for PublicKey {
    type Error = InvalidKey;

    fn try_from(public_key: &[u8]) -> Result<PublicKey, Self::Error> {
        let key: [u8; 32] =
            <[u8; 32]>::try_from(public_key).map_err(|_| InvalidKey::Length(public_key.len()))?;
        Ok(PublicKey(x25519_dalek::PublicKey::from(key)))
    }
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_key(self.0.as_bytes(), serializer)
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_key(deserializer)
    }
}

impl Hash for PublicKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_bytes().hash(state);
    }
}

impl cmp::PartialEq for PublicKey {
    fn eq(&self, other: &PublicKey) -> bool {
        self.0.as_bytes() == other.0.as_bytes()
    }
}

impl cmp::Eq for PublicKey {}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self)
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.to_base64())
    }
}

/// A WireGuard preshared key (PSK). Used to make the tunnel quantum-resistant.
#[derive(Clone, PartialEq, Eq, Hash, Zeroize, ZeroizeOnDrop)]
pub struct PresharedKey(Box<[u8; 32]>);

impl PresharedKey {
    /// Get the PSK as bytes. Try to move or dereference this data as little as possible,
    /// since copying it to more memory locations potentially leaves the secret in more memory
    /// locations.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<Box<[u8; 32]>> for PresharedKey {
    fn from(key: Box<[u8; 32]>) -> PresharedKey {
        PresharedKey(key)
    }
}

impl fmt::Debug for PresharedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &STANDARD.encode(self.as_bytes()))
    }
}

fn serialize_key<S>(key: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&STANDARD.encode(key))
}

fn deserialize_key<'de, D, K>(deserializer: D) -> Result<K, D::Error>
where
    D: Deserializer<'de>,
    K: From<[u8; 32]>,
{
    use serde::de::Error;

    String::deserialize(deserializer)
        .and_then(|string| key_from_base64(&string).map_err(|err| Error::custom(err.to_string())))
}

fn key_from_base64<K: From<[u8; 32]>>(key: &str) -> Result<K, InvalidKey> {
    let bytes = STANDARD.decode(key).map_err(InvalidKey::Format)?;
    if bytes.len() != 32 {
        return Err(InvalidKey::Length(bytes.len()));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(From::from(key))
}
