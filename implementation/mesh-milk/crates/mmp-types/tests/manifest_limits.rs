use mmp_types::consts::FILE_MAX;
use mmp_types::frame::{Frame, FT_RECON_IDS, FT_WANT};
use mmp_types::{FileEntry, FileManifest, Id16, Key32, Sig64, WireError};

#[test]
fn manifest_rejects_entry_above_file_max() {
    let manifest = FileManifest {
        suite: 1,
        sharer: Key32([1; 32]),
        entries: vec![FileEntry {
            name: b"oversized.bin".to_vec(),
            mime: b"application/octet-stream".to_vec(),
            size: (FILE_MAX as u32) + 1,
            file_root: Id16([2; 16]),
            rs_k: 32,
            rs_n: 48,
            full_hash: [3; 32],
            thumb_root: None,
        }],
        sig: Sig64([4; 64]),
    };

    assert_eq!(
        FileManifest::decode(&manifest.encode()),
        Err(WireError::BadValue("file size"))
    );

    let mut at_limit = manifest;
    at_limit.entries[0].size = FILE_MAX as u32;
    assert!(FileManifest::decode(&at_limit.encode()).is_ok());
}

#[test]
fn recon_ids_count_cannot_allocate_before_truncation_check() {
    let mut payload = vec![0u8]; // domain
    payload.extend_from_slice(&[0u8; 16]); // lo
    payload.extend_from_slice(&[0u8; 16]); // hi
    payload.extend_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(Frame::decode_payload(FT_RECON_IDS, &payload), Err(WireError::Truncated));
}

#[test]
fn want_count_cannot_allocate_before_truncation_check() {
    let payload = u16::MAX.to_le_bytes();
    assert_eq!(Frame::decode_payload(FT_WANT, &payload), Err(WireError::Truncated));
}
