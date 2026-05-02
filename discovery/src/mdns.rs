//! Real mDNS advertiser backed by the pure-Rust `mdns-sd` crate.

use crate::advertiser::{
    AdvertiseHandle, AdvertiseInner, Advertiser, DiscoveryError, Result, ServiceInfo,
};
use mdns_sd::{ServiceDaemon, ServiceInfo as MdnsServiceInfo};
use std::collections::HashMap;
#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn};

/// mDNS advertiser with **single-shot** semantics.
///
/// Holds a `mdns_sd::ServiceDaemon` whose `shutdown()` is global across every
/// `Clone` of the daemon (one underlying background thread, shared command
/// channel). Dropping any [`AdvertiseHandle`] produced by `advertise` therefore
/// terminates the daemon for this advertiser and any other live handles.
///
/// **Construct one `MdnsAdvertiser` per advertise lifecycle.** Drop the
/// advertiser when you drop the handle; do not reuse. Calling [`advertise`]
/// more than once on the same instance is undefined at the protocol level —
/// the second registration shares the daemon thread, and dropping any handle
/// kills it for all of them.
///
/// [`advertise`]: Advertiser::advertise
pub struct MdnsAdvertiser {
    daemon: ServiceDaemon,
    #[cfg(debug_assertions)]
    advertised: AtomicBool,
}

impl MdnsAdvertiser {
    pub fn new() -> Result<Self> {
        let daemon =
            ServiceDaemon::new().map_err(|e| DiscoveryError::ResponderStart(e.to_string()))?;
        Ok(Self {
            daemon,
            #[cfg(debug_assertions)]
            advertised: AtomicBool::new(false),
        })
    }
}

impl Advertiser for MdnsAdvertiser {
    /// Register the per-instance and alias records on the shared daemon.
    ///
    /// **Single-shot:** call this at most once per `MdnsAdvertiser`. The
    /// returned handle owns a clone of the underlying `ServiceDaemon`; when
    /// it is dropped, `shutdown()` tears down the daemon thread for every
    /// clone — including this advertiser. v1 callers must construct a fresh
    /// `MdnsAdvertiser` per advertise cycle. A future revision could enforce
    /// this with interior mutability (e.g. `take()` on an `Option<daemon>`)
    /// if a use case for reuse appears.
    fn advertise(&self, info: ServiceInfo) -> Result<AdvertiseHandle> {
        #[cfg(debug_assertions)]
        {
            // Catch reuse of the same advertiser in dev/test builds. Release
            // builds rely on the documented contract (zero overhead).
            let already = self.advertised.swap(true, Ordering::SeqCst);
            debug_assert!(
                !already,
                "MdnsAdvertiser::advertise called more than once; construct a fresh advertiser per lifecycle"
            );
        }

        let per_instance_host = format!(
            "{}-agentzero.local.",
            crate::network_info::sanitize_for_hostname(&info.instance_name)
        );
        let alias_host = if info.hostname_alias.ends_with('.') {
            info.hostname_alias.clone()
        } else {
            format!("{}.", info.hostname_alias)
        };

        let ipv4s: Vec<std::net::Ipv4Addr> = info.addrs.iter().map(|(_, ip)| *ip).collect();
        if ipv4s.is_empty() {
            return Err(DiscoveryError::Invalid(
                "no IPv4 addresses to advertise on".into(),
            ));
        }
        let ip_addrs: Vec<std::net::IpAddr> =
            ipv4s.iter().copied().map(std::net::IpAddr::V4).collect();

        let txt: HashMap<String, String> = info.txt.into_iter().collect();

        // Per-instance hostname record (always unique).
        let per_instance = MdnsServiceInfo::new(
            &info.service_type,
            &info.instance_name,
            &per_instance_host,
            &ip_addrs[..],
            info.port,
            txt.clone(),
        )
        .map_err(|e| DiscoveryError::Register(e.to_string()))?
        .enable_addr_auto();

        self.daemon
            .register(per_instance)
            .map_err(|e| DiscoveryError::Register(e.to_string()))?;

        // Friendly alias hostname record (best-effort, may collide).
        let alias_label = format!("{}-alias", info.instance_name);
        let alias = MdnsServiceInfo::new(
            &info.service_type,
            &alias_label,
            &alias_host,
            &ip_addrs[..],
            info.port,
            txt,
        )
        .map_err(|e| DiscoveryError::Register(e.to_string()))?
        .enable_addr_auto();

        // mdns-sd performs conflict probing; if collision occurs, the daemon
        // returns success but our alias entry won't be uniquely owned. We
        // can't detect this synchronously, so we optimistically claim and
        // surface the result via runtime monitoring (the daemon's
        // ServiceUnregistered event). For v1 we treat alias_claimed=true as
        // the optimistic default; future enhancement can listen for
        // ServiceConflict.
        let alias_claimed = match self.daemon.register(alias) {
            Ok(()) => true,
            Err(e) => {
                warn!(target: "discovery", "alias hostname collision detected: {}", e);
                false
            }
        };

        info!(
            target: "discovery",
            "advertising {} on {} interface(s) port={}",
            info.service_type,
            ipv4s.len(),
            info.port
        );

        Ok(AdvertiseHandle::new(
            Box::new(MdnsInner {
                daemon: self.daemon.clone(),
                fullname_per_instance: format!("{}.{}", info.instance_name, info.service_type),
                fullname_alias: format!("{}.{}", alias_label, info.service_type),
            }),
            alias_claimed,
        ))
    }
}

struct MdnsInner {
    daemon: ServiceDaemon,
    fullname_per_instance: String,
    fullname_alias: String,
}

impl AdvertiseInner for MdnsInner {
    fn shutdown(&mut self) {
        if let Err(e) = self.daemon.unregister(&self.fullname_per_instance) {
            warn!(target: "discovery", "unregister per-instance failed: {}", e);
        }
        if let Err(e) = self.daemon.unregister(&self.fullname_alias) {
            warn!(target: "discovery", "unregister alias failed: {}", e);
        }
        // Block briefly so goodbye packets flush before shutdown.
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Err(e) = self.daemon.shutdown() {
            warn!(target: "discovery", "responder shutdown failed: {}", e);
        }
        info!(target: "discovery", "withdrawn (goodbye sent)");
    }
}
