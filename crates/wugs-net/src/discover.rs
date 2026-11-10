//! mDNS discovery: each device announces `_wugs._tcp.local.` with a tag of
//! its project id in TXT, and browsing keeps only peers with the same tag.
//! The tag is a hash, so the café Wi-Fi learns that a wugs project is
//! present, not which one. Discovery is a convenience; `sync_with` takes a
//! plain address, so a network that drops multicast (some hotspots, this
//! repo's CI sandbox) still syncs.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

pub const SERVICE_TYPE: &str = "_wugs._tcp.local.";
const PROJECT_KEY: &str = "project";
const VERSION_KEY: &str = "v";

/// What the announcement carries instead of the project id: the first 16
/// hex digits of blake3(project_id || "wugs-mdns-v1").
pub fn project_tag(project_id: &str) -> String {
    let h = blake3::hash(&[project_id.as_bytes(), b"wugs-mdns-v1"].concat());
    h.to_hex()[..16].to_string()
}

fn version() -> String {
    format!("{}", crate::NET_VERSION as u32)
}

/// Keeps the announcement alive; dropping it unregisters and stops the daemon.
pub struct Announcement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for Announcement {
    fn drop(&mut self) {
        self.daemon.unregister(&self.fullname).ok();
        self.daemon.shutdown().ok();
    }
}

pub fn announce(project_id: &str, device_id: &str, port: u16) -> Result<Announcement, String> {
    let daemon = ServiceDaemon::new().map_err(|e| format!("mdns daemon: {e}"))?;
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        device_id,
        &format!("{device_id}.local."),
        "",
        port,
        &[(PROJECT_KEY, project_tag(project_id).as_str()), (VERSION_KEY, version().as_str())][..],
    )
    .map_err(|e| format!("mdns service info: {e}"))?
    .enable_addr_auto();
    let fullname = info.get_fullname().to_string();
    daemon.register(info).map_err(|e| format!("mdns register: {e}"))?;
    Ok(Announcement { daemon, fullname })
}

/// Peers announcing `project_id`, collected for the whole `timeout`, since
/// mDNS answers trickle in and there is no "last answer" signal. Includes
/// this device's own address if it is announcing too.
pub fn browse(project_id: &str, timeout: Duration) -> Result<Vec<SocketAddr>, String> {
    let daemon = ServiceDaemon::new().map_err(|e| format!("mdns daemon: {e}"))?;
    let events = daemon.browse(SERVICE_TYPE).map_err(|e| format!("mdns browse: {e}"))?;
    let (tag, v) = (project_tag(project_id), version());
    let deadline = Instant::now() + timeout;
    let mut found = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = events.recv_timeout(left) else { break };
        if let ServiceEvent::ServiceResolved(svc) = event {
            if svc.get_property_val_str(PROJECT_KEY) != Some(tag.as_str()) || svc.get_property_val_str(VERSION_KEY) != Some(v.as_str()) {
                continue;
            }
            for ip in svc.get_addresses() {
                let addr = SocketAddr::new(ip.to_ip_addr(), svc.get_port());
                if !found.contains(&addr) {
                    found.push(addr);
                }
            }
        }
    }
    daemon.shutdown().ok();
    Ok(found)
}
