//! Where the phone remote listens and how the phone reaches it: Tailscale
//! address (by interface, no CLI needed), MagicDNS name, and the pairing URL.

use std::net::{IpAddr, Ipv4Addr};
use std::process::Command;

use xenon_store::RemoteNetwork;

/// CLI locations: Homebrew/standalone, then the Mac App Store app bundle.
const TAILSCALE_CLIS: &[&str] = &[
    "tailscale",
    "/usr/local/bin/tailscale",
    "/opt/homebrew/bin/tailscale",
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
];

/// What the phone needs to reach this Mac.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Reachability {
    /// This Mac's Tailscale IPv4 (100.64.0.0/10), if connected.
    pub tailscale_ip: Option<Ipv4Addr>,
    /// MagicDNS name, e.g. `mbp.tail7c2e.ts.net`.
    pub magic_dns: Option<String>,
    /// First private LAN IPv4 (for the LAN opt-in).
    pub lan_ip: Option<Ipv4Addr>,
}

impl Reachability {
    /// Interface addresses only (cheap; no subprocess). MagicDNS comes later
    /// from `magic_dns_name` off the UI thread.
    pub(crate) fn probe_local() -> Self {
        let addrs = interface_ipv4s();
        Self {
            tailscale_ip: addrs.iter().copied().find(is_tailscale),
            magic_dns: None,
            lan_ip: addrs.iter().copied().find(|ip| ip.is_private()),
        }
    }

    /// Addresses the server binds for `network`: specific interfaces only
    /// (never 0.0.0.0), loopback always.
    pub(crate) fn bind_ips(&self, network: RemoteNetwork) -> Vec<IpAddr> {
        let lan = match network {
            RemoteNetwork::TailscaleAndLan => self.lan_ip,
            RemoteNetwork::Tailscale => None,
        };
        self.tailscale_ip
            .into_iter()
            .chain(lan)
            .chain([Ipv4Addr::LOCALHOST])
            .map(IpAddr::V4)
            .collect()
    }

    /// Best host for the phone: override, MagicDNS, Tailscale IP, LAN, loopback.
    pub(crate) fn phone_host(&self, network: RemoteNetwork, override_host: &str) -> String {
        if !override_host.is_empty() {
            return override_host.to_string();
        }
        if let Some(name) = &self.magic_dns {
            return name.clone();
        }
        if let Some(ip) = self.tailscale_ip {
            return ip.to_string();
        }
        match (network, self.lan_ip) {
            (RemoteNetwork::TailscaleAndLan, Some(ip)) => ip.to_string(),
            _ => Ipv4Addr::LOCALHOST.to_string(),
        }
    }

    /// The phone can reach us from off this Mac.
    pub(crate) fn reachable(&self, network: RemoteNetwork) -> bool {
        self.tailscale_ip.is_some()
            || (network == RemoteNetwork::TailscaleAndLan && self.lan_ip.is_some())
    }
}

/// `http://host:port` (host may already carry a port).
pub(crate) fn base_url(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("http://{host}")
    } else {
        format!("http://{host}:{port}")
    }
}

/// QR target: the fragment never reaches the server's request line or logs.
pub(crate) fn pairing_url(base: &str, secret: &str) -> String {
    format!("{base}/pair#{secret}")
}

/// Strip scheme/path noise; keep `host` or `host:port`.
pub(crate) fn normalize_hostname(raw: &str) -> String {
    let s = raw.trim();
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    s.split('/')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches(':')
        .to_string()
}

/// This Mac's user-facing name ("Michal's MacBook Pro").
pub(crate) fn computer_name() -> String {
    Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "this Mac".to_string())
}

fn is_tailscale(ip: &Ipv4Addr) -> bool {
    // CGNAT range 100.64.0.0/10.
    let [a, b, ..] = ip.octets();
    a == 100 && (64..128).contains(&b)
}

/// This Mac's MagicDNS name via the Tailscale CLI (spawns; call off-thread).
pub(crate) fn magic_dns_name() -> Option<String> {
    TAILSCALE_CLIS.iter().find_map(|cli| {
        let out = Command::new(cli)
            .args(["status", "--json", "--peers=false"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        let name = json.get("Self")?.get("DNSName")?.as_str()?;
        let name = name.trim_end_matches('.');
        (!name.is_empty()).then(|| name.to_string())
    })
}

/// IPv4 addresses of up, non-loopback interfaces.
fn interface_ipv4s() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs allocates a list we walk read-only and free once.
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return out;
        }
        let mut cursor = head;
        while !cursor.is_null() {
            let ifa = &*cursor;
            let up = ifa.ifa_flags & (libc::IFF_UP as u32) != 0;
            if up
                && !ifa.ifa_addr.is_null()
                && i32::from((*ifa.ifa_addr).sa_family) == libc::AF_INET
            {
                let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                if !ip.is_loopback() {
                    out.push(ip);
                }
            }
            cursor = ifa.ifa_next;
        }
        libc::freeifaddrs(head);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tailnet() -> Reachability {
        Reachability {
            tailscale_ip: Some(Ipv4Addr::new(100, 101, 2, 3)),
            magic_dns: Some("mbp.tail7c2e.ts.net".into()),
            lan_ip: Some(Ipv4Addr::new(192, 168, 1, 9)),
        }
    }

    #[test]
    fn tailscale_mode_binds_tailnet_and_loopback_only() {
        let ips = tailnet().bind_ips(RemoteNetwork::Tailscale);
        assert_eq!(
            ips,
            vec![
                IpAddr::V4(Ipv4Addr::new(100, 101, 2, 3)),
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            ]
        );
        let offline = Reachability::default().bind_ips(RemoteNetwork::Tailscale);
        assert_eq!(offline, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);
    }

    #[test]
    fn lan_mode_adds_the_lan_address_only() {
        let ips = tailnet().bind_ips(RemoteNetwork::TailscaleAndLan);
        assert_eq!(
            ips,
            vec![
                IpAddr::V4(Ipv4Addr::new(100, 101, 2, 3)),
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 9)),
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            ]
        );
    }

    #[test]
    fn phone_host_prefers_override_then_magic_dns() {
        let r = tailnet();
        assert_eq!(r.phone_host(RemoteNetwork::Tailscale, "mac.lan"), "mac.lan");
        assert_eq!(
            r.phone_host(RemoteNetwork::Tailscale, ""),
            "mbp.tail7c2e.ts.net"
        );
        let lan_only = Reachability {
            lan_ip: Some(Ipv4Addr::new(192, 168, 1, 9)),
            ..Default::default()
        };
        assert_eq!(
            lan_only.phone_host(RemoteNetwork::Tailscale, ""),
            "127.0.0.1"
        );
        assert_eq!(
            lan_only.phone_host(RemoteNetwork::TailscaleAndLan, ""),
            "192.168.1.9"
        );
    }

    #[test]
    fn detects_cgnat_range() {
        assert!(is_tailscale(&Ipv4Addr::new(100, 64, 0, 1)));
        assert!(is_tailscale(&Ipv4Addr::new(100, 127, 255, 1)));
        assert!(!is_tailscale(&Ipv4Addr::new(100, 128, 0, 1)));
        assert!(!is_tailscale(&Ipv4Addr::new(10, 0, 0, 1)));
    }

    #[test]
    fn urls() {
        let base = base_url("mbp.ts.net", 17890);
        assert_eq!(base, "http://mbp.ts.net:17890");
        assert_eq!(base_url("host:9000", 17890), "http://host:9000");
        assert_eq!(
            pairing_url(&base, "abc"),
            "http://mbp.ts.net:17890/pair#abc"
        );
        assert_eq!(normalize_hostname(" https://mac.lan:81/x "), "mac.lan:81");
    }
}
