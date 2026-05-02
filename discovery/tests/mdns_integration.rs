//! Integration tests that exercise real multicast. Gated by `#[ignore]` so
//! they don't run on every `cargo test`. Run them locally via:
//!
//!     cargo test -p discovery -- --ignored
//!
//! CI runs them on Linux only — macOS GitHub runners are unreliable for
//! multicast, and Windows runners block it.

use discovery::advertiser::Advertiser;
use discovery::mdns::MdnsAdvertiser;
use discovery::ServiceInfo;
use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::time::Duration;

fn sample_info(port: u16) -> ServiceInfo {
    let mut txt = BTreeMap::new();
    txt.insert("version".into(), "test".into());
    txt.insert(
        "instance".into(),
        "00000000-0000-0000-0000-000000000001".into(),
    );
    ServiceInfo {
        instance_name: "test-instance".into(),
        service_type: "_agentzero._tcp.local.".into(),
        hostname_alias: "agentzero-test.local".into(),
        port,
        txt,
        addrs: vec![("lo0".into(), Ipv4Addr::new(127, 0, 0, 1))],
    }
}

#[test]
#[ignore]
fn advertise_then_browse_finds_self() {
    let adv = MdnsAdvertiser::new().expect("responder starts");
    let _handle = adv
        .advertise(sample_info(18999))
        .expect("advertise succeeds");

    // Use mdns-sd's own browse API to confirm the service is visible.
    let daemon = mdns_sd::ServiceDaemon::new().expect("browser daemon");
    let receiver = daemon
        .browse("_agentzero._tcp.local.")
        .expect("browse starts");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        if let Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) =
            receiver.recv_timeout(Duration::from_millis(200))
        {
            if info.get_port() == 18999 {
                found = true;
                break;
            }
        }
    }
    daemon.shutdown().ok();
    assert!(found, "service was not discovered within 5s");
}

#[test]
#[ignore]
fn drop_handle_sends_goodbye() {
    let adv = MdnsAdvertiser::new().expect("responder starts");
    let handle = adv
        .advertise(sample_info(19000))
        .expect("advertise succeeds");
    drop(handle);

    // Allow goodbye packet to flush.
    std::thread::sleep(Duration::from_millis(300));

    // We trust mdns-sd to send the goodbye; assert the responder shut down
    // by confirming we can re-bind the same instance name without conflict.
    let adv2 = MdnsAdvertiser::new().expect("responder starts again");
    let _h2 = adv2
        .advertise(sample_info(19000))
        .expect("re-advertise after goodbye");
}
