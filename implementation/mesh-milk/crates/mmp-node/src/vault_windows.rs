//! Windows DPAPI-backed implementation of the internal [`SnapshotVault`]
//! contract.
//!
//! DPAPI protects only the fresh DEK.  The surrounding append-only journal is
//! deliberately owned by this adapter: every complete entry is checksummed,
//! flushed, and protected by an exclusive Windows file handle.  A partial
//! trailing entry is ignored as a crash artifact; a complete corrupt entry is
//! rejected rather than silently rolled back.

#![allow(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use zeroize::Zeroizing;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, GENERIC_READ, GENERIC_WRITE, HANDLE, HLOCAL,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, MoveFileExW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE, MOVEFILE_REPLACE_EXISTING,
    MOVEFILE_WRITE_THROUGH, OPEN_ALWAYS,
};
use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;

use super::{
    validate_commit_generation, validate_record_bounds, RestoreOperation, SealedCandidate,
    SealedRecord, SnapshotVault, VaultError, MAX_CONTEXT_BYTES,
};

const JOURNAL_MAGIC: [u8; 8] = *b"MMPVLT01";
const JOURNAL_VERSION: u8 = 1;
const JOURNAL_HEADER_LEN: usize = JOURNAL_MAGIC.len() + 1 + 8 + 8 + 4 + 4 + 4;
const JOURNAL_CHECKSUM_LEN: usize = 32;
const MAX_WRAPPED_DEK_BYTES: usize = 4096;
const MAX_JOURNAL_BYTES: u64 = 8 * 1024 * 1024;
const FILETIME_UNIX_EPOCH_100NS: u64 = 11644473600 * 10_000_000;

struct ExclusiveLock {
    handle: HANDLE,
}

impl Drop for ExclusiveLock {
    fn drop(&mut self) {
        // SAFETY: The handle was returned by CreateFileW and is closed exactly once.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// A Windows implementation of the Rust-internal vault seam.  It is not
/// exported through UniFFI and cannot be constructed by a remote peer.
pub(crate) struct WindowsDpapiVault {
    journal_path: PathBuf,
    lock_path: PathBuf,
    lock: Option<ExclusiveLock>,
}

#[derive(Debug, Default, PartialEq)]
struct JournalState {
    base_floor: u64,
    floor: u64,
    record: Option<SealedRecord>,
}

impl WindowsDpapiVault {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, VaultError> {
        let journal_path = path.as_ref().to_path_buf();
        if journal_path.as_os_str().is_empty() {
            return Err(VaultError::Invalid);
        }
        if let Some(parent) = journal_path.parent() {
            fs::create_dir_all(parent).map_err(|_| VaultError::Unavailable)?;
        }
        let mut lock_os = journal_path.as_os_str().to_os_string();
        lock_os.push(".lock");
        Ok(Self {
            journal_path,
            lock_path: PathBuf::from(lock_os),
            lock: None,
        })
    }

    fn lock(&mut self) -> Result<(), VaultError> {
        if self.lock.is_some() {
            return Ok(());
        }
        let path = wide_path(&self.lock_path)?;
        // An exclusive share mode turns the lock file into a real cross-
        // process transaction boundary.  OpenOrCreate keeps the file stable;
        // the OS handle, not deletion of a sentinel, owns the lease.
        // SAFETY: `path` is NUL-terminated and all pointer arguments are
        // either null or point to live values for the duration of the call.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_NONE,
                null(),
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            // SAFETY: GetLastError has no pointer arguments and is thread-local.
            let _ = unsafe { GetLastError() };
            return Err(VaultError::Unavailable);
        }
        self.lock = Some(ExclusiveLock { handle });
        Ok(())
    }

    fn unlock(&mut self) {
        self.lock.take();
    }

    fn commit_locked(
        &mut self,
        expected_generation: u64,
        candidate: SealedCandidate,
        dek: &[u8; 32],
    ) -> Result<(), VaultError> {
        validate_candidate(&candidate)?;
        validate_commit_generation(expected_generation, &candidate)?;
        let current = read_journal(&self.journal_path)?;
        if current.floor != expected_generation {
            return Err(VaultError::Generation);
        }
        let wrapped_dek = protect_dek(&candidate.context, dek)?;
        let record = SealedRecord {
            generation: candidate.generation,
            context: candidate.context,
            envelope: candidate.envelope,
            wrapped_dek,
        };
        let entry = encode_entry(&record, current.base_floor)?;
        let current_len = fs::metadata(&self.journal_path)
            .map(|m| m.len())
            .unwrap_or(0);
        if current_len.saturating_add(entry.len() as u64) > MAX_JOURNAL_BYTES {
            // Compaction carries the previous floor in the first retained
            // entry, so replacing a long journal cannot make generation N
            // look like an invalid first generation.
            let compacted = encode_entry(&record, current.floor)?;
            replace_journal(&self.journal_path, &compacted)?;
        } else {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.journal_path)
                .map_err(|_| VaultError::Unavailable)?;
            file.write_all(&entry)
                .map_err(|_| VaultError::Unavailable)?;
            file.sync_all().map_err(|_| VaultError::Unavailable)?;
        }
        Ok(())
    }
}

impl SnapshotVault for WindowsDpapiVault {
    fn with_restore_lock(
        &mut self,
        operation: &mut RestoreOperation<'_>,
    ) -> Result<u64, VaultError> {
        let already_locked = self.lock.is_some();
        if !already_locked {
            self.lock()?;
        }
        let result = operation(self);
        if !already_locked {
            self.unlock();
        }
        result
    }

    fn load(&mut self) -> Result<Option<SealedRecord>, VaultError> {
        Ok(read_journal(&self.journal_path)?.record)
    }

    fn generation_floor(&mut self) -> Result<u64, VaultError> {
        Ok(read_journal(&self.journal_path)?.floor)
    }

    fn trusted_now_ms(&mut self) -> Result<u64, VaultError> {
        let mut filetime = windows_sys::Win32::Foundation::FILETIME::default();
        // SAFETY: `filetime` is a valid writable FILETIME for the duration of the call.
        unsafe { GetSystemTimeAsFileTime(&mut filetime) };
        let ticks = (u64::from(filetime.dwHighDateTime) << 32) | u64::from(filetime.dwLowDateTime);
        let unix_ticks = ticks
            .checked_sub(FILETIME_UNIX_EPOCH_100NS)
            .ok_or(VaultError::Unavailable)?;
        Ok(unix_ticks / 10_000)
    }

    fn commit(
        &mut self,
        expected_generation: u64,
        candidate: SealedCandidate,
        dek: &[u8; 32],
    ) -> Result<(), VaultError> {
        let already_locked = self.lock.is_some();
        if !already_locked {
            self.lock()?;
        }
        let result = self.commit_locked(expected_generation, candidate, dek);
        if !already_locked {
            self.unlock();
        }
        result
    }

    fn unwrap_dek(
        &mut self,
        context: &[u8],
        wrapped_dek: &[u8],
    ) -> Result<Zeroizing<[u8; 32]>, VaultError> {
        if context.is_empty()
            || context.len() > MAX_CONTEXT_BYTES
            || wrapped_dek.is_empty()
            || wrapped_dek.len() > MAX_WRAPPED_DEK_BYTES
        {
            return Err(VaultError::Invalid);
        }
        unprotect_dek(context, wrapped_dek)
    }
}

fn validate_candidate(candidate: &SealedCandidate) -> Result<(), VaultError> {
    if candidate.context.is_empty()
        || candidate.context.len() > MAX_CONTEXT_BYTES
        || candidate.envelope.len() > mmp_crypto::snapshot::MAX_ENVELOPE_BYTES
    {
        return Err(VaultError::Invalid);
    }
    Ok(())
}

fn wide_path(path: &Path) -> Result<Vec<u16>, VaultError> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(VaultError::Invalid);
    }
    wide.push(0);
    Ok(wide)
}

fn protect_dek(context: &[u8], dek: &[u8; 32]) -> Result<Vec<u8>, VaultError> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: 32,
        pbData: dek.as_ptr() as *mut u8,
    };
    let entropy_bytes = *blake3::hash(context).as_bytes();
    let optional_entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: All blobs point to live buffers; the output blob is initialized
    // and its provider allocation is released by `copy_and_free_blob`.
    let ok = unsafe {
        CryptProtectData(
            &input,
            null(),
            &optional_entropy,
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    copy_and_free_blob(ok != 0, output)
}

fn unprotect_dek(context: &[u8], wrapped: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: wrapped.len() as u32,
        pbData: wrapped.as_ptr() as *mut u8,
    };
    let entropy_bytes = *blake3::hash(context).as_bytes();
    let optional_entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: All blobs point to live buffers; the output blob is initialized
    // and its provider allocation is released by `copy_and_free_blob`.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            &optional_entropy,
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    let bytes = Zeroizing::new(copy_and_free_blob(ok != 0, output)?);
    if bytes.len() != 32 {
        return Err(VaultError::Invalid);
    }
    let mut dek = Zeroizing::new([0u8; 32]);
    dek.copy_from_slice(&bytes);
    Ok(dek)
}

fn copy_and_free_blob(ok: bool, output: CRYPT_INTEGER_BLOB) -> Result<Vec<u8>, VaultError> {
    let ptr = output.pbData;
    let len = output.cbData as usize;
    let result = if !ok || ptr.is_null() || len == 0 || len > MAX_WRAPPED_DEK_BYTES {
        Err(VaultError::Unavailable)
    } else {
        // The pointer is owned by CryptProtectData/CryptUnprotectData and is
        // copied before LocalFree releases the provider allocation.
        // SAFETY: The provider returned `ptr` with `len` bytes and ownership
        // remains with this function until the immediate LocalFree below.
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len).to_vec() };
        Ok(bytes)
    };
    if !ptr.is_null() {
        // SAFETY: `ptr` is the provider allocation returned in the output blob.
        unsafe {
            let _ = LocalFree(ptr as HLOCAL);
        }
    }
    result
}

fn encode_entry(record: &SealedRecord, base_floor: u64) -> Result<Vec<u8>, VaultError> {
    validate_record_bounds(record)?;
    if record.wrapped_dek.is_empty() || record.wrapped_dek.len() > MAX_WRAPPED_DEK_BYTES {
        return Err(VaultError::Invalid);
    }
    if base_floor >= record.generation {
        return Err(VaultError::Generation);
    }
    let context_len = u32::try_from(record.context.len()).map_err(|_| VaultError::Invalid)?;
    let envelope_len = u32::try_from(record.envelope.len()).map_err(|_| VaultError::Invalid)?;
    let wrapped_len = u32::try_from(record.wrapped_dek.len()).map_err(|_| VaultError::Invalid)?;
    let payload_len = record
        .context
        .len()
        .saturating_add(record.envelope.len())
        .saturating_add(record.wrapped_dek.len());
    let total = JOURNAL_HEADER_LEN
        .checked_add(payload_len)
        .and_then(|n| n.checked_add(JOURNAL_CHECKSUM_LEN))
        .ok_or(VaultError::Invalid)?;
    if total as u64 > MAX_JOURNAL_BYTES {
        return Err(VaultError::Invalid);
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&JOURNAL_MAGIC);
    bytes.push(JOURNAL_VERSION);
    bytes.extend_from_slice(&base_floor.to_le_bytes());
    bytes.extend_from_slice(&record.generation.to_le_bytes());
    bytes.extend_from_slice(&context_len.to_le_bytes());
    bytes.extend_from_slice(&envelope_len.to_le_bytes());
    bytes.extend_from_slice(&wrapped_len.to_le_bytes());
    bytes.extend_from_slice(&record.context);
    bytes.extend_from_slice(&record.envelope);
    bytes.extend_from_slice(&record.wrapped_dek);
    bytes.extend_from_slice(blake3::hash(&bytes).as_bytes());
    Ok(bytes)
}

fn read_journal(path: &Path) -> Result<JournalState, VaultError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(JournalState::default())
        }
        Err(_) => return Err(VaultError::Unavailable),
    };
    if file.metadata().map_err(|_| VaultError::Unavailable)?.len() > MAX_JOURNAL_BYTES {
        return Err(VaultError::Invalid);
    }
    let mut bytes = Vec::new();
    file.take(MAX_JOURNAL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| VaultError::Unavailable)?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(VaultError::Invalid);
    }
    let mut state = JournalState::default();
    let mut pos = 0usize;
    while bytes.len().saturating_sub(pos) >= JOURNAL_HEADER_LEN {
        let start = pos;
        if bytes[pos..pos + JOURNAL_MAGIC.len()] != JOURNAL_MAGIC
            || bytes[pos + JOURNAL_MAGIC.len()] != JOURNAL_VERSION
        {
            return Err(VaultError::Invalid);
        }
        pos += JOURNAL_MAGIC.len() + 1;
        let first_entry = state.record.is_none();
        let base_floor = u64::from_le_bytes(
            bytes[pos..pos + 8]
                .try_into()
                .map_err(|_| VaultError::Invalid)?,
        );
        pos += 8;
        if first_entry {
            state.base_floor = base_floor;
            state.floor = base_floor;
        } else if base_floor != state.base_floor {
            return Err(VaultError::Invalid);
        }
        let generation = u64::from_le_bytes(
            bytes[pos..pos + 8]
                .try_into()
                .map_err(|_| VaultError::Invalid)?,
        );
        pos += 8;
        let context_len = u32::from_le_bytes(
            bytes[pos..pos + 4]
                .try_into()
                .map_err(|_| VaultError::Invalid)?,
        ) as usize;
        pos += 4;
        let envelope_len = u32::from_le_bytes(
            bytes[pos..pos + 4]
                .try_into()
                .map_err(|_| VaultError::Invalid)?,
        ) as usize;
        pos += 4;
        let wrapped_len = u32::from_le_bytes(
            bytes[pos..pos + 4]
                .try_into()
                .map_err(|_| VaultError::Invalid)?,
        ) as usize;
        pos += 4;
        if context_len == 0
            || context_len > MAX_CONTEXT_BYTES
            || envelope_len > mmp_crypto::snapshot::MAX_ENVELOPE_BYTES
            || wrapped_len == 0
            || wrapped_len > MAX_WRAPPED_DEK_BYTES
        {
            return Err(VaultError::Invalid);
        }
        let payload_len = context_len
            .checked_add(envelope_len)
            .and_then(|n| n.checked_add(wrapped_len))
            .ok_or(VaultError::Invalid)?;
        let end_without_checksum = pos.checked_add(payload_len).ok_or(VaultError::Invalid)?;
        let end = end_without_checksum
            .checked_add(JOURNAL_CHECKSUM_LEN)
            .ok_or(VaultError::Invalid)?;
        if end > bytes.len() {
            // A process may have died after writing only a prefix of the
            // final entry. Earlier flushed entries remain authoritative.
            break;
        }
        let expected = blake3::hash(&bytes[start..end_without_checksum]);
        if bytes[end_without_checksum..end] != expected.as_bytes()[..] {
            return Err(VaultError::Invalid);
        }
        if generation == 0 || state.floor.checked_add(1) != Some(generation) {
            return Err(VaultError::Generation);
        }
        let context_start = pos;
        let envelope_start = context_start + context_len;
        let wrapped_start = envelope_start + envelope_len;
        let record = SealedRecord {
            generation,
            context: bytes[context_start..envelope_start].to_vec(),
            envelope: bytes[envelope_start..wrapped_start].to_vec(),
            wrapped_dek: bytes[wrapped_start..end_without_checksum].to_vec(),
        };
        validate_record_bounds(&record)?;
        state.floor = generation;
        state.record = Some(record);
        pos = end;
    }
    Ok(state)
}

fn replace_journal(path: &Path, entry: &[u8]) -> Result<(), VaultError> {
    let mut temp_os = path.as_os_str().to_os_string();
    temp_os.push(".part");
    let temp = PathBuf::from(temp_os);
    let _ = fs::remove_file(&temp);
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|_| VaultError::Unavailable)?;
        file.write_all(entry).map_err(|_| VaultError::Unavailable)?;
        file.sync_all().map_err(|_| VaultError::Unavailable)?;
        let from = wide_path(&temp)?;
        let to = wide_path(path)?;
        // SAFETY: Both paths are NUL-terminated and remain live for the call;
        // the flags request replacement with write-through semantics.
        let ok = unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            return Err(VaultError::Unavailable);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn temp_path(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("mmp-vault-windows-{label}-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut lock = path.as_os_str().to_os_string();
        lock.push(".lock");
        let _ = fs::remove_file(lock);
        path
    }

    #[test]
    fn dpapi_round_trip_is_context_bound() {
        let context = b"MMPCTX01-test-context";
        let dek = [7u8; 32];
        let wrapped = protect_dek(context, &dek).unwrap();
        let opened = unprotect_dek(context, &wrapped).unwrap();
        assert_eq!(&*opened, &dek);
        assert!(unprotect_dek(b"different-context", &wrapped).is_err());
    }

    #[test]
    fn truncated_tail_keeps_last_complete_generation() {
        let path = temp_path("tail");
        let record = SealedRecord {
            generation: 1,
            context: vec![1],
            envelope: vec![2],
            wrapped_dek: vec![3],
        };
        let first = encode_entry(&record, 0).unwrap();
        fs::write(&path, [&first[..], &first[..first.len() / 2]].concat()).unwrap();
        let state = read_journal(&path).unwrap();
        assert_eq!(state.floor, 1);
        assert_eq!(state.record, Some(record));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn complete_corruption_fails_closed() {
        let path = temp_path("corrupt");
        let record = SealedRecord {
            generation: 1,
            context: vec![1],
            envelope: vec![2],
            wrapped_dek: vec![3],
        };
        let mut bytes = encode_entry(&record, 0).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&path, bytes).unwrap();
        assert_eq!(read_journal(&path), Err(VaultError::Invalid));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn compacted_journal_retains_a_nonzero_generation_floor() {
        let path = temp_path("compact");
        let record = SealedRecord {
            generation: 100,
            context: vec![1],
            envelope: vec![2],
            wrapped_dek: vec![3],
        };
        let entry = encode_entry(&record, 99).unwrap();
        replace_journal(&path, &entry).unwrap();
        let state = read_journal(&path).unwrap();
        assert_eq!(state.base_floor, 99);
        assert_eq!(state.floor, 100);
        assert_eq!(state.record, Some(record));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn lock_is_held_through_restore_callback() {
        let path = temp_path("lock");
        let mut first = WindowsDpapiVault::open(&path).unwrap();
        let observed = Arc::new(Mutex::new(false));
        let inside = Arc::clone(&observed);
        let mut operation = |_: &mut dyn SnapshotVault| {
            let second = WindowsDpapiVault::open(&path).unwrap();
            let mut second = second;
            let mut nested = |_op: &mut dyn SnapshotVault| Ok(0);
            *inside.lock().unwrap() = second.with_restore_lock(&mut nested).is_err();
            Ok(0)
        };
        assert_eq!(first.with_restore_lock(&mut operation).unwrap(), 0);
        assert!(*observed.lock().unwrap());
        let _ = fs::remove_file(&path);
        let mut lock = path.as_os_str().to_os_string();
        lock.push(".lock");
        let _ = fs::remove_file(lock);
    }

    #[test]
    fn commit_wraps_the_dek_and_advances_the_durable_floor() {
        let path = temp_path("commit");
        let mut vault = WindowsDpapiVault::open(&path).unwrap();
        let candidate = SealedCandidate {
            generation: 1,
            context: b"MMPCTX01-commit-context".to_vec(),
            envelope: vec![9, 8, 7],
        };
        let dek = [0x55; 32];
        vault.commit(0, candidate.clone(), &dek).unwrap();
        assert_eq!(vault.generation_floor().unwrap(), 1);
        let record = vault.load().unwrap().unwrap();
        assert_eq!(record.generation, 1);
        assert_eq!(record.context, candidate.context);
        assert_eq!(
            *vault
                .unwrap_dek(&record.context, &record.wrapped_dek)
                .unwrap(),
            dek
        );
        let candidate_two = SealedCandidate {
            generation: 2,
            context: b"MMPCTX01-commit-context".to_vec(),
            envelope: vec![6, 5, 4],
        };
        vault.commit(1, candidate_two, &dek).unwrap();
        assert_eq!(vault.generation_floor().unwrap(), 2);
        assert_eq!(vault.load().unwrap().unwrap().generation, 2);
        assert_eq!(
            vault.commit(0, candidate, &dek),
            Err(VaultError::Generation)
        );
        let _ = fs::remove_file(&path);
        let mut lock = path.as_os_str().to_os_string();
        lock.push(".lock");
        let _ = fs::remove_file(lock);
    }
}
