//! Rust-internal typed aggregate for future keystore-backed Node snapshots.
//!
//! This module intentionally stops before provider I/O: it serializes and
//! restores the complete local conversation/prekey state, but callers must
//! authenticate/encrypt the returned bytes and commit one generation
//! atomically. No function here is exported through UniFFI or JavaScript.

use std::collections::{HashMap, HashSet};

use mmp_crypto::prekey::PrekeyStore;
use mmp_crypto::snapshot::MAX_PLAINTEXT_BYTES;
use mmp_crypto::ratchet::{RecvChain, SendChain};
use mmp_core::ConversationDag;
use mmp_types::ids::{Id16, Key32};
use zeroize::Zeroizing;

use crate::conv::Conv;
use crate::{DeliveryState, NodeCore, SentMessage};

const MAGIC: [u8; 8] = *b"MMPNSV01";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 8 + 1 + 2 + 2 + 2 + 2 + 4 + 4;
const MAX_CONVERSATIONS: usize = 256;
const MAX_ESTABLISHED: usize = 256;
const MAX_PENDING: usize = 256;
const MAX_SENT: usize = 4096;
const MAX_COMPONENT_BYTES: usize = MAX_PLAINTEXT_BYTES;

/// Failure to encode/decode a Node secret-state aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecretStateError {
    /// Malformed, non-canonical, inconsistent, or oversized state.
    Invalid,
    /// Restore was attempted after live sessions or queued wire output existed.
    Busy,
}

pub(super) struct RestoredState {
    pub(super) convs: HashMap<Id16, Conv>,
    pub(super) established_convs: HashSet<Id16>,
    pub(super) prekeys: PrekeyStore,
    pub(super) pending_async: HashMap<Id16, (Key32, u16)>,
    pub(super) pending_first_contact: HashMap<Id16, Id16>,
    pub(super) sent: HashMap<Id16, SentMessage>,
}

pub(crate) fn encode(core: &NodeCore) -> Result<Zeroizing<Vec<u8>>, SecretStateError> {
    if core.convs.len() > MAX_CONVERSATIONS
        || core.established_convs.len() > MAX_ESTABLISHED
        || core.pending_async.len() > MAX_PENDING
        || core.pending_first_contact.len() > MAX_PENDING
        || core.sent.len() > MAX_SENT
    {
        return Err(SecretStateError::Invalid);
    }
    let prekeys = core.prekeys.snapshot().map_err(|_| SecretStateError::Invalid)?;
    let mut out = Vec::with_capacity(HEADER_LEN);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    put_u16(&mut out, core.convs.len())?;
    put_u16(&mut out, core.established_convs.len())?;
    put_u16(&mut out, core.pending_async.len())?;
    put_u16(&mut out, core.pending_first_contact.len())?;
    put_u32(&mut out, core.sent.len())?;
    put_len(&mut out, prekeys.len())?;

    let mut conv_ids: Vec<Id16> = core.convs.keys().copied().collect();
    conv_ids.sort_unstable();
    for id in conv_ids {
        let conv = core.convs.get(&id).ok_or(SecretStateError::Invalid)?;
        let send = conv.send.snapshot().map_err(|_| SecretStateError::Invalid)?;
        let recv = conv.recv.snapshot().map_err(|_| SecretStateError::Invalid)?;
        let dag = conv.dag.snapshot().map_err(|_| SecretStateError::Invalid)?;
        put_id(&mut out, &id);
        out.extend_from_slice(&conv.peer.0);
        out.extend_from_slice(&*conv.receipt_key);
        put_len(&mut out, send.len())?;
        out.extend_from_slice(&send);
        put_len(&mut out, recv.len())?;
        out.extend_from_slice(&recv);
        put_len(&mut out, dag.len())?;
        out.extend_from_slice(&dag);
    }

    let mut established: Vec<Id16> = core.established_convs.iter().copied().collect();
    established.sort_unstable();
    for id in established {
        put_id(&mut out, &id);
    }

    let mut pending_async: Vec<(Id16, Key32, u16)> = core
        .pending_async
        .iter()
        .map(|(id, (eph, prekey))| (*id, *eph, *prekey))
        .collect();
    pending_async.sort_unstable_by_key(|entry| entry.0);
    for (id, eph, prekey) in pending_async {
        put_id(&mut out, &id);
        out.extend_from_slice(&eph.0);
        out.extend_from_slice(&prekey.to_le_bytes());
    }

    let mut pending_first: Vec<(Id16, Id16)> = core
        .pending_first_contact
        .iter()
        .map(|(conv, first)| (*conv, *first))
        .collect();
    pending_first.sort_unstable_by_key(|entry| entry.0);
    for (conv, first) in pending_first {
        put_id(&mut out, &conv);
        put_id(&mut out, &first);
    }

    let mut sent: Vec<(Id16, Id16, DeliveryState)> = core
        .sent
        .iter()
        .map(|(id, message)| (*id, message.conv, message.state))
        .collect();
    sent.sort_unstable_by_key(|entry| entry.0);
    for (id, conv, state) in sent {
        put_id(&mut out, &id);
        put_id(&mut out, &conv);
        out.push(state_to_byte(state));
    }
    out.extend_from_slice(&prekeys);
    if out.len() > MAX_PLAINTEXT_BYTES {
        return Err(SecretStateError::Invalid);
    }
    Ok(Zeroizing::new(out))
}

pub(crate) fn decode(bytes: &[u8]) -> Result<RestoredState, SecretStateError> {
    if bytes.len() < HEADER_LEN || bytes[..MAGIC.len()] != MAGIC || bytes[MAGIC.len()] != VERSION {
        return Err(SecretStateError::Invalid);
    }
    let mut pos = MAGIC.len() + 1;
    let conv_count = read_u16(bytes, &mut pos)? as usize;
    let established_count = read_u16(bytes, &mut pos)? as usize;
    let pending_async_count = read_u16(bytes, &mut pos)? as usize;
    let pending_first_count = read_u16(bytes, &mut pos)? as usize;
    let sent_count = read_u32(bytes, &mut pos)? as usize;
    let prekeys_len = read_u32(bytes, &mut pos)? as usize;
    if conv_count > MAX_CONVERSATIONS
        || established_count > MAX_ESTABLISHED
        || pending_async_count > MAX_PENDING
        || pending_first_count > MAX_PENDING
        || sent_count > MAX_SENT
        || prekeys_len > MAX_COMPONENT_BYTES
    {
        return Err(SecretStateError::Invalid);
    }

    let mut convs = HashMap::new();
    let mut previous_conv = None;
    for _ in 0..conv_count {
        let id = read_id(bytes, &mut pos)?;
        if previous_conv.is_some_and(|old| old >= id) {
            return Err(SecretStateError::Invalid);
        }
        let peer = Key32(read_array(bytes, &mut pos)?);
        let receipt_key = read_array(bytes, &mut pos)?;
        let send_bytes = read_component(bytes, &mut pos)?;
        let recv_bytes = read_component(bytes, &mut pos)?;
        let dag_bytes = read_component(bytes, &mut pos)?;
        let send = SendChain::restore(send_bytes).map_err(|_| SecretStateError::Invalid)?;
        let recv = RecvChain::restore(recv_bytes).map_err(|_| SecretStateError::Invalid)?;
        let dag = ConversationDag::restore(dag_bytes).map_err(|_| SecretStateError::Invalid)?;
        if convs.insert(id, Conv::from_parts(id, peer, receipt_key, send, recv, dag)).is_some() {
            return Err(SecretStateError::Invalid);
        }
        previous_conv = Some(id);
    }

    let mut established_convs = HashSet::new();
    let mut previous_established = None;
    for _ in 0..established_count {
        let id = read_id(bytes, &mut pos)?;
        if previous_established.is_some_and(|old| old >= id) || !established_convs.insert(id) {
            return Err(SecretStateError::Invalid);
        }
        previous_established = Some(id);
    }
    if established_convs.iter().any(|id| !convs.contains_key(id)) {
        return Err(SecretStateError::Invalid);
    }

    let mut pending_async = HashMap::new();
    let mut previous_pending = None;
    for _ in 0..pending_async_count {
        let id = read_id(bytes, &mut pos)?;
        if previous_pending.is_some_and(|old| old >= id) {
            return Err(SecretStateError::Invalid);
        }
        let eph = Key32(read_array(bytes, &mut pos)?);
        let prekey = read_u16(bytes, &mut pos)?;
        if !convs.contains_key(&id) || pending_async.insert(id, (eph, prekey)).is_some() {
            return Err(SecretStateError::Invalid);
        }
        previous_pending = Some(id);
    }

    let mut pending_first_contact = HashMap::new();
    let mut previous_first = None;
    for _ in 0..pending_first_count {
        let conv = read_id(bytes, &mut pos)?;
        if previous_first.is_some_and(|old| old >= conv) {
            return Err(SecretStateError::Invalid);
        }
        let first = read_id(bytes, &mut pos)?;
        if !convs.contains_key(&conv) || pending_first_contact.insert(conv, first).is_some() {
            return Err(SecretStateError::Invalid);
        }
        previous_first = Some(conv);
    }

    let mut sent = HashMap::new();
    let mut previous_sent = None;
    for _ in 0..sent_count {
        let id = read_id(bytes, &mut pos)?;
        if previous_sent.is_some_and(|old| old >= id) {
            return Err(SecretStateError::Invalid);
        }
        let conv = read_id(bytes, &mut pos)?;
        let state = byte_to_state(read_byte(bytes, &mut pos)?)?;
        if !convs.contains_key(&conv) || sent.insert(id, SentMessage { conv, state }).is_some() {
            return Err(SecretStateError::Invalid);
        }
        previous_sent = Some(id);
    }

    let prekey_end = pos.checked_add(prekeys_len).ok_or(SecretStateError::Invalid)?;
    if prekey_end != bytes.len() {
        return Err(SecretStateError::Invalid);
    }
    let prekeys = PrekeyStore::restore(&bytes[pos..prekey_end]).map_err(|_| SecretStateError::Invalid)?;
    Ok(RestoredState { convs, established_convs, prekeys, pending_async, pending_first_contact, sent })
}

fn put_id(out: &mut Vec<u8>, id: &Id16) {
    out.extend_from_slice(&id.0);
}

fn put_u16(out: &mut Vec<u8>, value: usize) -> Result<(), SecretStateError> {
    out.extend_from_slice(&u16::try_from(value).map_err(|_| SecretStateError::Invalid)?.to_le_bytes());
    Ok(())
}

fn put_u32(out: &mut Vec<u8>, value: usize) -> Result<(), SecretStateError> {
    out.extend_from_slice(&u32::try_from(value).map_err(|_| SecretStateError::Invalid)?.to_le_bytes());
    Ok(())
}

fn put_len(out: &mut Vec<u8>, value: usize) -> Result<(), SecretStateError> {
    put_u32(out, value)
}

fn read_byte(bytes: &[u8], pos: &mut usize) -> Result<u8, SecretStateError> {
    let value = *bytes.get(*pos).ok_or(SecretStateError::Invalid)?;
    *pos += 1;
    Ok(value)
}

fn read_u16(bytes: &[u8], pos: &mut usize) -> Result<u16, SecretStateError> {
    let end = pos.checked_add(2).ok_or(SecretStateError::Invalid)?;
    let value = u16::from_le_bytes(bytes.get(*pos..end).ok_or(SecretStateError::Invalid)?.try_into().map_err(|_| SecretStateError::Invalid)?);
    *pos = end;
    Ok(value)
}

fn read_u32(bytes: &[u8], pos: &mut usize) -> Result<u32, SecretStateError> {
    let end = pos.checked_add(4).ok_or(SecretStateError::Invalid)?;
    let value = u32::from_le_bytes(bytes.get(*pos..end).ok_or(SecretStateError::Invalid)?.try_into().map_err(|_| SecretStateError::Invalid)?);
    *pos = end;
    Ok(value)
}

fn read_id(bytes: &[u8], pos: &mut usize) -> Result<Id16, SecretStateError> {
    Ok(Id16(read_array(bytes, pos)?))
}

fn read_array<const N: usize>(bytes: &[u8], pos: &mut usize) -> Result<[u8; N], SecretStateError> {
    let end = pos.checked_add(N).ok_or(SecretStateError::Invalid)?;
    let mut value = [0u8; N];
    value.copy_from_slice(bytes.get(*pos..end).ok_or(SecretStateError::Invalid)?);
    *pos = end;
    Ok(value)
}

fn read_component<'a>(bytes: &'a [u8], pos: &mut usize) -> Result<&'a [u8], SecretStateError> {
    let len = read_u32(bytes, pos)? as usize;
    if len > MAX_COMPONENT_BYTES {
        return Err(SecretStateError::Invalid);
    }
    let end = pos.checked_add(len).ok_or(SecretStateError::Invalid)?;
    let value = bytes.get(*pos..end).ok_or(SecretStateError::Invalid)?;
    *pos = end;
    Ok(value)
}

fn state_to_byte(state: DeliveryState) -> u8 {
    match state {
        DeliveryState::Queued => 0,
        DeliveryState::InMesh => 1,
        DeliveryState::Delivered => 2,
        DeliveryState::Expired => 3,
    }
}

fn byte_to_state(value: u8) -> Result<DeliveryState, SecretStateError> {
    match value {
        0 => Ok(DeliveryState::Queued),
        1 => Ok(DeliveryState::InMesh),
        2 => Ok(DeliveryState::Delivered),
        3 => Ok(DeliveryState::Expired),
        _ => Err(SecretStateError::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmp_core::MemStore;
    use crate::{Node, Tier};

    fn introduce(a: &mut Node, b: &mut Node) {
        for claim in a.own_claims() {
            b.ingest_claim(claim, true);
        }
        for claim in b.own_claims() {
            a.ingest_claim(claim, true);
        }
    }

    #[test]
    fn aggregate_roundtrip_preserves_first_contact_and_delivery_tracking() {
        let mut original = Node::new([81; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut peer = Node::new([82; 32], Box::new(MemStore::new()), Tier::Relay);
        introduce(&mut original, &mut peer);
        let message = original.send_text(&peer.node_id(), "persist me").expect("first send");
        let snapshot = original.secret_state_snapshot().expect("aggregate snapshot");

        let mut restored = Node::new([81; 32], Box::new(MemStore::new()), Tier::Relay);
        restored.restore_secret_state(&snapshot).expect("aggregate restore");
        assert_eq!(restored.secret_state_snapshot().unwrap().as_slice(), snapshot.as_slice());
        assert_eq!(restored.message_state(&message), Some(DeliveryState::Queued));
    }

    #[test]
    fn malformed_or_busy_restore_is_atomic() {
        let mut original = Node::new([83; 32], Box::new(MemStore::new()), Tier::Relay);
        let mut peer = Node::new([84; 32], Box::new(MemStore::new()), Tier::Relay);
        introduce(&mut original, &mut peer);
        original.send_text(&peer.node_id(), "atomic").unwrap();
        let snapshot = original.secret_state_snapshot().unwrap();

        let mut restored = Node::new([83; 32], Box::new(MemStore::new()), Tier::Relay);
        let before = restored.secret_state_snapshot().unwrap();
        let mut malformed = snapshot.to_vec();
        malformed.push(0);
        assert_eq!(restored.restore_secret_state(&malformed), Err(SecretStateError::Invalid));
        assert_eq!(restored.secret_state_snapshot().unwrap().as_slice(), before.as_slice());

        restored.on_tal_event(mmp_tal::TalEvent::LinkUp {
            link: 7,
            initiator: true,
            quality: Default::default(),
        });
        assert_eq!(restored.restore_secret_state(&snapshot), Err(SecretStateError::Busy));
    }
}
