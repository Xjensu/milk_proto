//! Fixed-size wire primitives (doc 10 §1) and NodeID derivation (doc 03 §3.2).

/// 128-bit identifier: BundleID / NodeID / GroupID / blob-root prefix.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Id16(pub [u8; 16]);

/// Ed25519 or X25519 public key.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key32(pub [u8; 32]);

/// Ed25519 signature.
#[derive(Clone, Copy)]
pub struct Sig64(pub [u8; 64]);

impl Id16 {
    pub const MIN: Id16 = Id16([0x00; 16]);
    pub const MAX: Id16 = Id16([0xff; 16]);

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl Key32 {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Sig64 {
    pub const ZERO: Sig64 = Sig64([0; 64]);

    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

impl PartialEq for Sig64 {
    fn eq(&self, other: &Self) -> bool {
        self.0[..] == other.0[..]
    }
}
impl Eq for Sig64 {}

impl core::fmt::Debug for Id16 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Id16(")?;
        for b in &self.0[..4] {
            write!(f, "{b:02x}")?;
        }
        f.write_str("…)")
    }
}
impl core::fmt::Debug for Key32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Key32(")?;
        for b in &self.0[..4] {
            write!(f, "{b:02x}")?;
        }
        f.write_str("…)")
    }
}
impl core::fmt::Debug for Sig64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Sig64(…)")
    }
}

/// A NodeID is an `Id16` derived from an identity key — never an authenticator
/// (doc 03 §3.2).
pub type NodeId = Id16;

/// `NodeID = BLAKE3-256(identity_pubkey)[0..16]` (doc 03 §3.2).
pub fn node_id_of(identity_pubkey: &Key32) -> NodeId {
    let hash = blake3::hash(&identity_pubkey.0);
    let mut id = [0u8; 16];
    id.copy_from_slice(&hash.as_bytes()[..16]);
    Id16(id)
}

/// Maximum value representable in the 5-byte `u40` lifetime field.
pub const U40_MAX: u64 = (1 << 40) - 1;
