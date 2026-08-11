//! Wire-format errors (doc 10 §6 canonical-encoding rules).
//!
//! Canonical rule: there is exactly one valid encoding of any value. Every
//! deviation is a distinct, testable rejection reason — parsers never "repair".

/// Decode or fallible-encoding rejection. Compatibility encoding wrappers may
/// still panic for invalid caller-constructed values; production paths should
/// use the `try_*` encoder APIs and handle this error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// Input ended before the structure did.
    Truncated,
    /// Bytes remained after the structure ended (doc 10 §6.1).
    Trailing,
    /// A length field exceeds a protocol cap (MAX_BUNDLE, varbytes cap, …).
    Oversize,
    /// Wire version nibble is not 1.
    BadVersion,
    /// Non-zero reserved bits (doc 10 §6.1).
    ReservedBits,
    /// TLV types not strictly ascending (covers duplicates; doc 10 §6.1).
    TlvOrder,
    /// Unknown TLV in the critical range 0x00–0x7F (doc 03 §4.4).
    TlvUnknownCritical,
    /// TLV value length inconsistent with its type's fixed layout.
    TlvLength,
    /// A field value outside its domain (priority > 3, count = 0, …).
    BadValue(&'static str),
}

impl core::fmt::Display for WireError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WireError::Truncated => f.write_str("truncated"),
            WireError::Trailing => f.write_str("trailing bytes"),
            WireError::Oversize => f.write_str("length over cap"),
            WireError::BadVersion => f.write_str("bad wire version"),
            WireError::ReservedBits => f.write_str("non-zero reserved bits"),
            WireError::TlvOrder => f.write_str("TLV order/duplicate"),
            WireError::TlvUnknownCritical => f.write_str("unknown critical TLV"),
            WireError::TlvLength => f.write_str("TLV length"),
            WireError::BadValue(what) => write!(f, "bad value: {what}"),
        }
    }
}

impl core::error::Error for WireError {}
