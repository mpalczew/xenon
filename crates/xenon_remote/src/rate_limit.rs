use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

pub(crate) struct RateLimiter {
    per_ip: HashMap<IpAddr, Vec<Instant>>,
    global: Vec<Instant>,
}

const WINDOW: Duration = Duration::from_secs(60);
const PER_IP_LIMIT: usize = 5;
const GLOBAL_LIMIT: usize = 20;

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            per_ip: HashMap::new(),
            global: Vec::new(),
        }
    }

    /// False once this IP (or everyone) failed too often in the last minute.
    pub fn allow(&mut self, ip: IpAddr, now: Instant) -> bool {
        self.prune(now);
        let ip_fails = self.per_ip.get(&ip).map_or(0, Vec::len);
        ip_fails < PER_IP_LIMIT && self.global.len() < GLOBAL_LIMIT
    }

    pub(crate) fn record_failure(&mut self, ip: IpAddr, now: Instant) {
        self.per_ip.entry(ip).or_default().push(now);
        self.global.push(now);
    }

    fn prune(&mut self, now: Instant) {
        let fresh = |t: &Instant| now.duration_since(*t) < WINDOW;
        self.global.retain(fresh);
        self.per_ip.retain(|_, v| {
            v.retain(fresh);
            !v.is_empty()
        });
    }
}
