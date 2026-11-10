use std::net::Ipv4Addr;
use std::process::Command;

const TAILSCALE_CLIS: &[&str] = &["tailscale"];

pub fn computer_name() -> String {
    let mut command = Command::new("hostname");
    command
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "this computer".to_string())
}

/// This machine's MagicDNS name via the Tailscale CLI (spawns).
pub fn magic_dns_name() -> Option<String> {
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

pub(crate) fn interface_ipv4s() -> Vec<Ipv4Addr> {
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
