//! Hardware-in-the-loop smoke test: advertise the MMP service with a dummy
//! beacon and echo back every inbound write as notifications.
//!
//! Requires Linux + BlueZ + a real Bluetooth adapter; NEVER runs in the test
//! suite (examples are only built, not executed, by `cargo test`).
//!
//! ```text
//! machine A (Linux):  cargo run -p mmp-tal-peripheral --example serve
//! machine B:          any MMP central (mmp-tal-btleplug) or a GATT tool
//!                     (e.g. nRF Connect): write to …-0001-…, subscribe to
//!                     …-0002-…, and watch the echo.
//! ```

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mmp_tal_peripheral::{PeripheralRole, TalEvent};

    let role = PeripheralRole::new().await?;

    // Placeholder beacon: 24 zero bytes standing in for the real
    // ver_role(1) ‖ ephem_id(8) ‖ epoch(1) ‖ summary(14) payload that
    // `Node::beacon()` mints (doc 03 §5.1) — this example exercises the
    // radio path, not the protocol.
    role.advertise(vec![0u8; 24]).await?;
    println!("advertising MMP dev service; waiting for a central (Ctrl-C to quit)…");

    let mut events = role.events();
    while let Some(ev) = events.recv().await {
        println!("{ev:?}");
        if let TalEvent::LinkData { link, bytes } = ev {
            // Echo, so the central sees its own frame come back.
            if let Err(e) = role.notify(link, &bytes).await {
                eprintln!("echo failed: {e}");
            }
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("the serve example needs Linux/BlueZ (the only implemented peripheral backend)");
    std::process::exit(1);
}
