use anyhow::{Context, anyhow};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use itertools::Itertools;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::{
    ConnectionConfig, CustomTunnelEndpoint, settings,
    wireguard::{DaitaSettings, QuantumResistantState},
};
use talpid_types::net::wireguard;
use test_macro::test_function;
use test_rpc::ServiceClient;

use super::{
    Error, TestContext,
    helpers::{self, connect_and_wait, set_custom_endpoint},
};
use crate::{
    TEST_CONFIG,
    network_monitor::{
        Direction, IpHeaderProtocols, MonitorOptions, start_packet_monitor_until,
        start_tunnel_packet_monitor_until,
    },
    vm::network::{
        CUSTOM_TUN_GATEWAY, CUSTOM_TUN_LOCAL_PRIVKEY, CUSTOM_TUN_LOCAL_TUN_ADDR,
        CUSTOM_TUN_REMOTE_PUBKEY, CUSTOM_TUN_REMOTE_REAL_PORT, CUSTOM_TUN_REMOTE_TUN_ADDR,
    },
};

/// How long to wait for expected "DNS queries" to appear
const MONITOR_TIMEOUT: Duration = Duration::from_secs(5);

/// Test whether DNS leaks can be produced when using the default resolver. It does this by
/// connecting to a custom WireGuard relay on localhost and monitoring outbound DNS traffic in (and
/// outside of) the tunnel interface.
///
/// The test succeeds if and only if expected outbound packets inside the tunnel on port 53 are
/// observed. If traffic on port 53 is observed outside the tunnel or to an unexpected destination,
/// the test fails.
///
/// # Limitations
///
/// This test only detects outbound DNS leaks in the connected state.
#[test_function]
pub async fn test_dns_leak_default(
    _: TestContext,
    rpc: ServiceClient,
    mut mullvad_client: MullvadProxyClient,
) -> Result<(), Error> {
    leak_test_dns(
        &rpc,
        &mut mullvad_client,
        true,
        IpAddr::V4(CUSTOM_TUN_REMOTE_TUN_ADDR),
    )
    .await
}

/// Test whether DNS leaks can be produced when using a custom public IP. This test succeeds if and
/// only if outgoing packets are only observed on the tunnel interface to the expected IP.
///
/// See `test_dns_leak_default` for more details.
///
/// # Limitations
///
/// This test only detects outbound DNS leaks in the connected state.
#[test_function]
pub async fn test_dns_leak_custom_public_ip(
    _: TestContext,
    rpc: ServiceClient,
    mut mullvad_client: MullvadProxyClient,
) -> Result<(), Error> {
    const CONFIG_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 3, 3, 7));

    log::debug!("Setting custom DNS resolver to {CONFIG_IP}");

    mullvad_client
        .set_dns_options(settings::DnsOptions {
            default_options: settings::DefaultDnsOptions::default(),
            custom_options: settings::CustomDnsOptions {
                addresses: vec![CONFIG_IP],
            },
            state: settings::DnsState::Custom,
        })
        .await
        .expect("failed to configure DNS server");

    leak_test_dns(&rpc, &mut mullvad_client, true, CONFIG_IP).await
}

/// Test whether DNS leaks can be produced when using a custom private IP. This test succeeds if and
/// only if outgoing packets are only observed on the non-tunnel interface to the expected IP.
///
/// See `test_dns_leak_default` for more details.
///
/// # Limitations
///
/// This test only detects outbound DNS leaks in the connected state.
#[test_function]
pub async fn test_dns_leak_custom_private_ip(
    _: TestContext,
    rpc: ServiceClient,
    mut mullvad_client: MullvadProxyClient,
) -> Result<(), Error> {
    const CONFIG_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 64, 10, 1));

    log::debug!("Setting custom DNS resolver to {CONFIG_IP}");

    mullvad_client
        .set_dns_options(settings::DnsOptions {
            default_options: settings::DefaultDnsOptions::default(),
            custom_options: settings::CustomDnsOptions {
                addresses: vec![CONFIG_IP],
            },
            state: settings::DnsState::Custom,
        })
        .await
        .expect("failed to configure DNS server");

    leak_test_dns(&rpc, &mut mullvad_client, false, CONFIG_IP).await
}

/// See whether it is possible to send "DNS queries" to a particular whitelisted destination on
/// either the tunnel interface or a non-tunnel interface on port 53. This test fails if:
/// * No packets to the whitelisted destination are observed, or
/// * Packets to any other destination or a non-matching interface are observed.
async fn leak_test_dns(
    rpc: &ServiceClient,
    mullvad_client: &mut MullvadProxyClient,
    use_tun: bool,
    whitelisted_dest: IpAddr,
) -> Result<(), Error> {
    // Connect to local wireguard relay
    //

    connect_local_wg_relay(mullvad_client)
        .await
        .expect("failed to connect to custom wg relay");

    let nontun_iface = rpc
        .get_default_interface()
        .await
        .expect("failed to find non-tun interface");
    let tunnel_iface = helpers::get_tunnel_interface(mullvad_client)
        .await
        .expect("failed to find tunnel interface");

    let nontun_ip = rpc
        .get_interface_ip(nontun_iface.clone())
        .await
        .expect("failed to obtain guest IP");
    let tunnel_ip = rpc
        .get_interface_ip(tunnel_iface.clone())
        .await
        .expect("failed to obtain tunnel IP");

    log::debug!("Tunnel (guest) IP: {tunnel_ip}");
    log::debug!("Non-tunnel (guest) IP: {nontun_ip}");

    // Spoof DNS packets
    //

    let tun_bind_addr = SocketAddr::new(tunnel_ip, 0);
    let nontun_bind_addr = SocketAddr::new(nontun_ip, 0);

    let whitelisted_dest = SocketAddr::new(whitelisted_dest, 53);
    let blocked_dest_local = "10.64.100.100:53".parse().unwrap();
    let blocked_dest_public = "1.1.1.1:53".parse().unwrap();

    // Capture all outgoing DNS
    let mut pkt_counter = DnsPacketsFound::new(1, 1);

    let (tunnel_monitor, non_tunnel_monitor) = if use_tun {
        let tunnel_monitor = start_tunnel_packet_monitor_until(
            move |packet| packet.destination.port() == 53,
            move |packet| pkt_counter.handle_packet(packet),
            MonitorOptions {
                direction: Some(Direction::In),
                timeout: Some(MONITOR_TIMEOUT),
                ..Default::default()
            },
        )
        .await;
        let non_tunnel_monitor = start_packet_monitor_until(
            move |packet| packet.destination.port() == 53,
            |_packet| false,
            MonitorOptions {
                direction: Some(Direction::In),
                ..Default::default()
            },
        )
        .await;
        (tunnel_monitor, non_tunnel_monitor)
    } else {
        let tunnel_monitor = start_tunnel_packet_monitor_until(
            move |packet| packet.destination.port() == 53,
            |_packet| false,
            MonitorOptions {
                direction: Some(Direction::In),
                ..Default::default()
            },
        )
        .await;
        let non_tunnel_monitor = start_packet_monitor_until(
            move |packet| packet.destination.port() == 53,
            move |packet| pkt_counter.handle_packet(packet),
            MonitorOptions {
                direction: Some(Direction::In),
                timeout: Some(MONITOR_TIMEOUT),
                ..Default::default()
            },
        )
        .await;
        (tunnel_monitor, non_tunnel_monitor)
    };

    // We should observe 2 outgoing packets to the whitelisted destination
    // on port 53, and only inside the desired interface.

    let rpc = rpc.clone();
    let probes = tokio::spawn(async move {
        tokio::join!(
            // send to allowed dest
            spoof_packets(
                &rpc,
                Some(tunnel_iface.clone()),
                tun_bind_addr,
                whitelisted_dest,
            ),
            spoof_packets(
                &rpc,
                Some(nontun_iface.clone()),
                nontun_bind_addr,
                whitelisted_dest,
            ),
            // send to blocked local dest
            spoof_packets(
                &rpc,
                Some(tunnel_iface.clone()),
                tun_bind_addr,
                blocked_dest_local,
            ),
            spoof_packets(
                &rpc,
                Some(nontun_iface.clone()),
                nontun_bind_addr,
                blocked_dest_local,
            ),
            // send to blocked public dest
            spoof_packets(&rpc, Some(tunnel_iface), tun_bind_addr, blocked_dest_public,),
            spoof_packets(
                &rpc,
                Some(nontun_iface),
                nontun_bind_addr,
                blocked_dest_public,
            ),
        )
    });

    if use_tun {
        // Examine tunnel traffic
        //

        let tunnel_result = tunnel_monitor.wait().await.unwrap();

        probes.abort();
        let _ = probes.await;

        assert!(
            tunnel_result.packets.len() >= 2,
            "expected at least 2 in-tunnel packets to allowed destination only"
        );

        for pkt in tunnel_result.packets {
            assert_eq!(
                pkt.destination, whitelisted_dest,
                "unexpected tunnel packet on port 53"
            );
        }

        // Examine non-tunnel traffic
        //

        let non_tunnel_result = non_tunnel_monitor.into_result().await.unwrap();
        assert_eq!(
            non_tunnel_result.packets.len(),
            0,
            "expected no non-tunnel packets on port 53"
        );
    } else {
        let non_tunnel_result = non_tunnel_monitor.wait().await.unwrap();

        probes.abort();
        let _ = probes.await;

        // Examine tunnel traffic
        //

        let tunnel_result = tunnel_monitor.into_result().await.unwrap();
        assert_eq!(
            tunnel_result.packets.len(),
            0,
            "expected no tunnel packets on port 53"
        );

        // Examine non-tunnel traffic
        //

        assert!(
            non_tunnel_result.packets.len() >= 2,
            "expected at least 2 non-tunnel packets to allowed destination only"
        );

        for pkt in non_tunnel_result.packets {
            assert_eq!(
                pkt.destination, whitelisted_dest,
                "unexpected non-tunnel packet on port 53"
            );
        }
    }

    Ok(())
}

/// Test whether the expected default DNS resolver is used by `getaddrinfo` (via `ToSocketAddrs`).
///
/// # Limitations
///
/// This only examines outbound packets.
#[test_function]
pub async fn test_dns_config_default(
    _: TestContext,
    rpc: ServiceClient,
    mut mullvad_client: MullvadProxyClient,
) -> anyhow::Result<()> {
    run_dns_config_tunnel_test(
        &rpc,
        &mut mullvad_client,
        IpAddr::V4(CUSTOM_TUN_REMOTE_TUN_ADDR),
    )
    .await
}

/// Test whether the expected custom DNS works for private IPs.
///
/// # Limitations
///
/// This only examines outbound packets.
#[test_function]
pub async fn test_dns_config_custom_private(
    _: TestContext,
    rpc: ServiceClient,
    mut mullvad_client: MullvadProxyClient,
) -> anyhow::Result<()> {
    log::debug!(
        "Setting custom DNS resolver to {}",
        TEST_CONFIG.host_bridge_ip
    );

    mullvad_client
        .set_dns_options(settings::DnsOptions {
            default_options: settings::DefaultDnsOptions::default(),
            custom_options: settings::CustomDnsOptions {
                addresses: vec![IpAddr::V4(TEST_CONFIG.host_bridge_ip)],
            },
            state: settings::DnsState::Custom,
        })
        .await
        .context("failed to configure DNS server")?;

    run_dns_config_non_tunnel_test(
        &rpc,
        &mut mullvad_client,
        IpAddr::V4(TEST_CONFIG.host_bridge_ip),
    )
    .await
}

/// Test whether the expected custom DNS works for public IPs.
///
/// # Limitations
///
/// This only examines outbound packets.
#[test_function]
pub async fn test_dns_config_custom_public(
    _: TestContext,
    rpc: ServiceClient,
    mut mullvad_client: MullvadProxyClient,
) -> anyhow::Result<()> {
    let custom_ip = IpAddr::V4(Ipv4Addr::new(1, 3, 3, 7));

    log::debug!("Setting custom DNS resolver to {custom_ip}");

    mullvad_client
        .set_dns_options(settings::DnsOptions {
            default_options: settings::DefaultDnsOptions::default(),
            custom_options: settings::CustomDnsOptions {
                addresses: vec![custom_ip],
            },
            state: settings::DnsState::Custom,
        })
        .await
        .context("failed to configure DNS server")?;

    run_dns_config_tunnel_test(&rpc, &mut mullvad_client, custom_ip).await
}

/// Test whether the correct IPs are configured as system resolver when
/// content blockers are enabled.
#[test_function]
pub async fn test_content_blockers(
    _: TestContext,
    rpc: ServiceClient,
    mut mullvad_client: MullvadProxyClient,
) -> anyhow::Result<()> {
    const DNS_BLOCKING_IP_BASE: Ipv4Addr = Ipv4Addr::new(100, 64, 0, 0);
    let content_blockers = [
        (
            "adblocking",
            1 << 0,
            settings::DefaultDnsOptions {
                block_ads: true,
                ..Default::default()
            },
        ),
        (
            "tracker",
            1 << 1,
            settings::DefaultDnsOptions {
                block_trackers: true,
                ..Default::default()
            },
        ),
        (
            "malware",
            1 << 2,
            settings::DefaultDnsOptions {
                block_malware: true,
                ..Default::default()
            },
        ),
        (
            "adult",
            1 << 3,
            settings::DefaultDnsOptions {
                block_adult_content: true,
                ..Default::default()
            },
        ),
        (
            "gambling",
            1 << 4,
            settings::DefaultDnsOptions {
                block_gambling: true,
                ..Default::default()
            },
        ),
    ];

    let combine_cases = |v: Vec<&(&str, u8, settings::DefaultDnsOptions)>| {
        let mut combination_name = String::new();
        let mut last_byte = 0;
        let mut options = settings::DefaultDnsOptions::default();

        for case in v {
            if !combination_name.is_empty() {
                combination_name.push_str(" + ");
            }
            combination_name.push_str(case.0);

            last_byte |= case.1;

            options.block_ads |= case.2.block_ads;
            options.block_trackers |= case.2.block_trackers;
            options.block_malware |= case.2.block_malware;
            options.block_adult_content |= case.2.block_adult_content;
            options.block_gambling |= case.2.block_gambling;
        }

        let mut dns_ip = DNS_BLOCKING_IP_BASE.octets();
        dns_ip[dns_ip.len() - 1] |= last_byte;

        (
            combination_name,
            IpAddr::V4(Ipv4Addr::from(dns_ip)),
            options,
        )
    };

    // Test all combinations

    for case in content_blockers.iter().powerset() {
        if case.is_empty() {
            continue;
        }
        let (test_name, test_ip, test_opts) = combine_cases(case);

        log::debug!("Testing content blocker: {test_name}, {test_ip}");

        mullvad_client
            .set_dns_options(settings::DnsOptions {
                default_options: test_opts,
                custom_options: settings::CustomDnsOptions::default(),
                state: settings::DnsState::Default,
            })
            .await
            .context("failed to configure DNS server")?;

        run_dns_config_tunnel_test(&rpc, &mut mullvad_client, test_ip).await?;
    }

    Ok(())
}

async fn run_dns_config_tunnel_test(
    rpc: &ServiceClient,
    mullvad_client: &mut MullvadProxyClient,
    expected_dns_resolver: IpAddr,
) -> anyhow::Result<()> {
    run_dns_config_test(
        rpc,
        || {
            start_tunnel_packet_monitor_until(
                move |packet| packet.destination.port() == 53,
                |packet| packet.destination.port() != 53,
                MonitorOptions {
                    direction: Some(Direction::In),
                    timeout: Some(MONITOR_TIMEOUT),
                    ..Default::default()
                },
            )
        },
        mullvad_client,
        expected_dns_resolver,
    )
    .await
}

async fn run_dns_config_non_tunnel_test(
    rpc: &ServiceClient,
    mullvad_client: &mut MullvadProxyClient,
    expected_dns_resolver: IpAddr,
) -> anyhow::Result<()> {
    run_dns_config_test(
        rpc,
        || {
            start_packet_monitor_until(
                move |packet| packet.destination.port() == 53,
                |packet| packet.destination.port() != 53,
                MonitorOptions {
                    direction: Some(Direction::In),
                    timeout: Some(MONITOR_TIMEOUT),
                    ..Default::default()
                },
            )
        },
        mullvad_client,
        expected_dns_resolver,
    )
    .await
}

async fn run_dns_config_test<
    F: std::future::Future<Output = crate::network_monitor::PacketMonitor>,
>(
    rpc: &ServiceClient,
    create_monitor: impl FnOnce() -> F,
    mullvad_client: &mut MullvadProxyClient,
    expected_dns_resolver: IpAddr,
) -> anyhow::Result<()> {
    match mullvad_client.get_tunnel_state().await {
        // prevent reconnect
        Ok(mullvad_types::states::TunnelState::Connected { .. }) => (),
        _ => {
            connect_local_wg_relay(mullvad_client)
                .await
                .context("failed to connect to custom wg relay")?;
        }
    }

    let nontun_iface = rpc
        .get_default_interface()
        .await
        .context("failed to find non-tun interface")?;
    let tunnel_iface = helpers::get_tunnel_interface(mullvad_client)
        .await
        .context("failed to find tunnel interface")?;

    let nontun_ip = rpc
        .get_interface_ip(nontun_iface)
        .await
        .context("failed to obtain guest IP")?;
    let tunnel_ip = rpc
        .get_interface_ip(tunnel_iface)
        .await
        .context("failed to obtain tunnel IP")?;

    log::debug!("Tunnel (guest) IP: {tunnel_ip}");
    log::debug!("Non-tunnel (guest) IP: {nontun_ip}");

    let monitor = create_monitor().await;

    let next_nonce = {
        static NONCE: AtomicUsize = AtomicUsize::new(0);
        || NONCE.fetch_add(1, Ordering::Relaxed)
    };

    let rpc_client = rpc.clone();
    let handle = tokio::spawn(async move {
        // Resolve a "random" domain name to prevent caching.
        // Try multiple times, as the DNS config change may not take effect immediately.
        for _ in 0..2 {
            let _ = rpc_client
                .resolve_hostname(format!("test{}.mullvad.net", next_nonce()))
                .await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    assert_eq!(
        monitor
            .wait()
            .await?
            .packets
            .first()
            .ok_or(anyhow!("Expected at least one packet, saw none"))?
            .destination,
        SocketAddr::new(expected_dns_resolver, 53),
        "expected tunnel packet to expected destination {expected_dns_resolver}"
    );

    handle.abort();
    let _ = handle.await;

    Ok(())
}

/// Connect to the WireGuard relay that is set up in test-manager/src/vm/network
/// See those files for details.
async fn connect_local_wg_relay(mullvad_client: &mut MullvadProxyClient) -> Result<(), Error> {
    // the local wg relay doesn't support negotiating an ephemeral peer
    // which means we can't use PQ or daita.
    mullvad_client
        .set_quantum_resistant_tunnel(QuantumResistantState::Off)
        .await?;
    mullvad_client
        .set_daita_settings(DaitaSettings::default())
        .await?;

    let peer_addr: SocketAddr = SocketAddr::new(
        IpAddr::V4(TEST_CONFIG.host_bridge_ip),
        CUSTOM_TUN_REMOTE_REAL_PORT,
    );

    let custom_tunnel_endpoint = CustomTunnelEndpoint {
        host: peer_addr.ip().to_string(),
        config: ConnectionConfig::Wireguard(wireguard::ConnectionConfig {
            tunnel: wireguard::TunnelConfig {
                addresses: vec![IpAddr::V4(CUSTOM_TUN_LOCAL_TUN_ADDR)],
                private_key: wireguard::PrivateKey::from(CUSTOM_TUN_LOCAL_PRIVKEY),
            },
            peer: wireguard::PeerConfig {
                public_key: wireguard::PublicKey::from(CUSTOM_TUN_REMOTE_PUBKEY),
                allowed_ips: vec!["0.0.0.0/0".parse().unwrap()],
                endpoint: peer_addr,
                psk: None,
                constant_packet_size: false,
            },
            ipv4_gateway: CUSTOM_TUN_GATEWAY,
            exit_peer: None,
            #[cfg(target_os = "linux")]
            fwmark: None,
            ipv6_gateway: None,
        }),
    };
    set_custom_endpoint(mullvad_client, custom_tunnel_endpoint)
        .await
        .expect("failed to update relay settings");

    connect_and_wait(mullvad_client).await?;

    Ok(())
}

async fn spoof_packets(
    rpc: &ServiceClient,
    interface: Option<String>,
    bind_addr: SocketAddr,
    dest: SocketAddr,
) {
    let tcp_rpc = rpc.clone();
    let tcp_interface = interface.clone();
    let tcp_send = async move {
        log::debug!("sending to {}/tcp from {}", dest, bind_addr);
        let _ = tcp_rpc.send_tcp(tcp_interface, bind_addr, dest).await;
    };
    let udp_rpc = rpc.clone();
    let udp_interface = interface.clone();
    let udp_send = async move {
        log::debug!("sending to {}/udp from {}", dest, bind_addr);
        let _ = udp_rpc.send_udp(udp_interface, bind_addr, dest).await;
    };
    let _ = tokio::join!(tcp_send, udp_send);
}

type ShouldContinue = bool;

struct DnsPacketsFound {
    tcp_count: usize,
    udp_count: usize,
    min_tcp_count: usize,
    min_udp_count: usize,
}

impl DnsPacketsFound {
    fn new(min_udp_count: usize, min_tcp_count: usize) -> Self {
        Self {
            tcp_count: 0,
            udp_count: 0,
            min_tcp_count,
            min_udp_count,
        }
    }

    fn handle_packet(&mut self, pkt: &crate::network_monitor::ParsedPacket) -> ShouldContinue {
        if pkt.destination.port() != 53 && pkt.source.port() != 53 {
            return true;
        }
        match pkt.protocol {
            IpHeaderProtocols::Udp => self.udp_count += 1,
            IpHeaderProtocols::Tcp => self.tcp_count += 1,
            _ => return true,
        }
        self.udp_count < self.min_udp_count || self.tcp_count < self.min_tcp_count
    }
}
