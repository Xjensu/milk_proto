//! Delivery-receipt MAC (doc 08 §7): only conversation-key holders can mint;
//! relays check the receipt *bundle's* origin signature, the origin checks the MAC.

use mmp_types::ids::Id16;
use zeroize::Zeroizing;

pub fn receipt_key(conv_root: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    Zeroizing::new(blake3::derive_key("mmp1 receipt", conv_root))
}

/// `BLAKE3.keyed(receipt_key, delivered_id)[0..16]`.
pub fn receipt_mac(key: &[u8; 32], delivered: &Id16) -> [u8; 16] {
    let mut mac = [0u8; 16];
    mac.copy_from_slice(&blake3::keyed_hash(key, &delivered.0).as_bytes()[..16]);
    mac
}

/// Constant-time MAC comparison.
pub fn mac_eq(a: &[u8; 16], b: &[u8; 16]) -> bool {
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
