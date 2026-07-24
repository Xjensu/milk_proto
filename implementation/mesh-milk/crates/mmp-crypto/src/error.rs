//! Crypto-layer errors. Deliberately coarse: callers drop, they don't diagnose
//! an attacker's inputs.

/// A crypto-layer failure. Variants are coarse by design (see module docs):
/// callers drop the offending input rather than branch on why it was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// Malformed public key.
    BadKey,
    /// Signature failed strict (canonical-S) verification.
    BadSignature,
    /// AEAD open failed.
    Aead,
    /// Noise handshake / transport failure.
    Noise,
    /// Ratchet index outside the skipped-key window, or already consumed.
    Ratchet,
    /// Prekey id unknown / batch invalid.
    Prekey,
    /// Merkle path does not connect leaf to root.
    Merkle,
}

impl core::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            CryptoError::BadKey => "bad key",
            CryptoError::BadSignature => "bad signature",
            CryptoError::Aead => "aead failure",
            CryptoError::Noise => "noise failure",
            CryptoError::Ratchet => "ratchet window",
            CryptoError::Prekey => "prekey failure",
            CryptoError::Merkle => "merkle path",
        };
        f.write_str(s)
    }
}

impl core::error::Error for CryptoError {}
