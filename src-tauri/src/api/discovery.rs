use futures_util::future::join_all;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

const IMMICH_PORT: u16 = 2283;

/// Scan the local network for running Immich servers.
/// Deduplicates results by MAC address (via ARP cache) so a host with
/// multiple network interfaces only appears once.
pub async fn discover_immich_servers() -> Vec<String> {
    let mut candidates: Vec<String> = vec![
        format!("http://immich.local:{}", IMMICH_PORT),
        format!("http://immich:{}", IMMICH_PORT),
    ];

    if let Some(local_ip) = get_local_ipv4() {
        let octets = local_ip.octets();
        for i in 1u8..=254 {
            candidates.push(format!(
                "http://{}.{}.{}.{}:{}",
                octets[0], octets[1], octets[2], i, IMMICH_PORT
            ));
        }
    }

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    // Phase 1: parallel TCP + Immich ping check
    let scan_tasks = candidates.into_iter().map(|url| {
        let client = http_client.clone();
        async move {
            let addr = url
                .trim_start_matches("http://")
                .trim_start_matches("https://");

            let tcp_ok = tokio::time::timeout(
                Duration::from_millis(500),
                tokio::net::TcpStream::connect(addr),
            )
            .await
            .is_ok();

            if !tcp_ok {
                return None;
            }

            let ping = format!("{}/api/server/ping", url);
            match client.get(&ping).send().await {
                Ok(r) if r.status().is_success() => Some(url),
                _ => None,
            }
        }
    });

    let found: Vec<String> = join_all(scan_tasks).await.into_iter().flatten().collect();

    if found.len() <= 1 {
        return found;
    }

    // Phase 2: deduplicate using ARP cache.
    // TCP connections made above cause the OS to populate the ARP table,
    // so we can look up each IP's MAC address. Same MAC = same physical host.
    let arp = read_arp_cache();

    let mut seen_macs: HashSet<String> = HashSet::new();
    found
        .into_iter()
        .filter(|url| {
            let ip = extract_ip(url);
            // Use MAC address as dedup key; fall back to the IP itself
            // (e.g. for hostname-based entries like immich.local)
            let key = arp.get(&ip).cloned().unwrap_or_else(|| ip.clone());
            seen_macs.insert(key)
        })
        .collect()
}

/// Extract the host portion from a URL like "http://192.168.1.1:2283"
fn extract_ip(url: &str) -> String {
    url.trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .unwrap_or(url)
        .to_string()
}

/// Read the OS ARP cache and return a map of IP → MAC address.
/// Works on Windows (`arp -a`) and Linux/macOS.
fn read_arp_cache() -> HashMap<String, String> {
    let mut map = HashMap::new();

    let Ok(output) = std::process::Command::new("arp").arg("-a").output() else {
        return map;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let ip = parts[0];
            let mac = parts[1];
            // Basic sanity check: IP has 3 dots, MAC has 5 separators (- or :)
            let looks_like_ip = ip.chars().filter(|&c| c == '.').count() == 3;
            let looks_like_mac = mac.chars().filter(|&c| c == '-' || c == ':').count() == 5;
            if looks_like_ip && looks_like_mac {
                map.insert(ip.to_string(), mac.to_string());
            }
        }
    }

    map
}

/// Determine the local IPv4 address by probing the routing table.
/// No packets are actually sent.
fn get_local_ipv4() -> Option<std::net::Ipv4Addr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    }
}
