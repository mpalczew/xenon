use std::net::{IpAddr, Ipv4Addr};

use crate::names::interface_ipv4s;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reachability {
    pub tailscale_ip: Option<Ipv4Addr>,
    pub magic_dns: Option<String>,
}

impl Reachability {
    pub fn probe_local() -> Self {
        let addrs = interface_ipv4s();
        Self {
            tailscale_ip: addrs.iter().copied().find(is_tailscale),
            magic_dns: None,
        }
    }

    pub fn bind_ips(&self) -> Vec<IpAddr> {
        self.tailscale_ip
            .into_iter()
            .chain([Ipv4Addr::LOCALHOST])
            .map(IpAddr::V4)
            .collect()
    }

    pub fn phone_host(&self, override_host: &str) -> String {
        if !override_host.is_empty() {
            return override_host.to_string();
        }
        if let Some(name) = &self.magic_dns {
            return name.clone();
        }
        match self.tailscale_ip {
            Some(ip) => ip.to_string(),
            None => Ipv4Addr::LOCALHOST.to_string(),
        }
    }

    pub fn reachable(&self) -> bool {
        self.tailscale_ip.is_some()
    }
}

pub fn base_url(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("http://{host}")
    } else {
        format!("http://{host}:{port}")
    }
}

pub fn pairing_url(base: &str, secret: &str) -> String {
    format!("{base}/pair#{secret}")
}

pub fn normalize_hostname(raw: &str) -> String {
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

fn is_tailscale(ip: &Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 100 && (64..128).contains(&b)
}
