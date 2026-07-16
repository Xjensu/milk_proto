//! Hardware smoke test for `mmp-tal-btleplug` — HANDOFF M5.
//!
//! This example is built by CI (`cargo build -p mmp-tal-btleplug --examples`)
//! so it never bit-rots, but it is **only ever run manually** on a host with
//! a real Bluetooth adapter:
//!
//!   cargo run -p mmp-tal-btleplug --example scan
//!
//! It scans for MMP beacons ([`DEV_SERVICE_UUID`]) for 10 seconds and prints
//! one line per sighting (payload length + RSSI). No hardware means no
//! adapter, which is a runtime error here — not a build failure.

use std::time::Duration;

use mmp_tal_btleplug::{BleCentral, TalEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let central = BleCentral::new().await?;
    let mut events = central.events();
    central.start_scan().await?;
    println!("scanning for MMP beacons for 10s...");

    let deadline = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => break,
            maybe_event = events.recv() => {
                match maybe_event {
                    Some(TalEvent::Beacon { payload, rssi }) => {
                        println!("beacon: {} bytes, rssi={rssi}", payload.len());
                    }
                    Some(other) => println!("event: {other:?}"),
                    None => break,
                }
            }
        }
    }

    println!("done");
    Ok(())
}
