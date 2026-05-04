//! Public surface for advertising a service on the LAN. Implementations are
//! injected via `Arc<dyn Advertiser>`.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("mdns responder failed to start: {0}")]
    ResponderStart(String),
    #[error("failed to register service: {0}")]
    Register(String),
    #[error("invalid service info: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, DiscoveryError>;

/// What the gateway hands to the advertiser at startup.
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub instance_name: String,
    pub service_type: String,
    pub hostname_alias: String,
    pub port: u16,
    pub txt: BTreeMap<String, String>,
    pub addrs: Vec<(String, Ipv4Addr)>, // (interface name, ipv4)
}

/// Outcome of a successful advertise call.
pub struct AdvertiseHandle {
    /// Held to keep the responder thread alive. Drop sends goodbye and stops it.
    inner: Box<dyn AdvertiseInner + Send + Sync>,
    /// True if the friendly alias was claimed without collision.
    pub alias_claimed: bool,
}

/// Implementation detail trait — boxed inside `AdvertiseHandle`.
pub trait AdvertiseInner: Send + Sync {
    /// Send goodbye + stop the responder. Called from `Drop`.
    fn shutdown(&mut self);
}

impl AdvertiseHandle {
    pub fn new(inner: Box<dyn AdvertiseInner + Send + Sync>, alias_claimed: bool) -> Self {
        Self {
            inner,
            alias_claimed,
        }
    }
}

impl Drop for AdvertiseHandle {
    fn drop(&mut self) {
        self.inner.shutdown();
    }
}

/// Public trait the gateway depends on.
pub trait Advertiser: Send + Sync {
    fn advertise(&self, info: ServiceInfo) -> Result<AdvertiseHandle>;
}

/// Used when `expose_to_lan = false`. Returns a handle that does nothing on drop.
#[derive(Debug, Default)]
pub struct NoopAdvertiser;

struct NoopInner;
impl AdvertiseInner for NoopInner {
    fn shutdown(&mut self) {}
}

impl Advertiser for NoopAdvertiser {
    fn advertise(&self, _info: ServiceInfo) -> Result<AdvertiseHandle> {
        Ok(AdvertiseHandle::new(Box::new(NoopInner), true))
    }
}

/// Convenience constructor for the noop case.
pub fn noop() -> Arc<dyn Advertiser> {
    Arc::new(NoopAdvertiser)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn sample_info() -> ServiceInfo {
        ServiceInfo {
            instance_name: "test".into(),
            service_type: "_zbot._tcp.local.".into(),
            hostname_alias: "zbot.local".into(),
            port: 18791,
            txt: BTreeMap::new(),
            addrs: vec![],
        }
    }

    #[test]
    fn noop_advertiser_returns_handle_with_alias_claimed() {
        let adv = NoopAdvertiser;
        let handle = adv.advertise(sample_info()).expect("noop never errors");
        assert!(handle.alias_claimed);
    }

    #[test]
    fn dropping_handle_calls_inner_shutdown_exactly_once() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Counting;
        impl AdvertiseInner for Counting {
            fn shutdown(&mut self) {
                COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        let handle = AdvertiseHandle::new(Box::new(Counting), true);
        drop(handle);
        assert_eq!(COUNT.load(Ordering::SeqCst), 1);
    }
}
