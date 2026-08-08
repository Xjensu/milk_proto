//! Hardware-independent fencing for BlueZ reader/writer callbacks.
//!
//! A single stable [`LinkId`] can outlive multiple BlueZ socket pairs.  The
//! endpoint identity and monotonic generation in [`StreamToken`] make delayed
//! callbacks from an older socket fail closed instead of removing or feeding
//! a replacement link.  Reader and writer tokens are intentionally separate:
//! BlueZ presents the two GATT directions as independent control events.

use std::collections::HashMap;

use mmp_tal::LinkId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamSide {
    Reader,
    Writer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StreamToken {
    pub(crate) link: LinkId,
    pub(crate) generation: u64,
    pub(crate) side: StreamSide,
    pub(crate) endpoint: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttachResult {
    pub(crate) token: StreamToken,
    /// The same direction arrived with a different endpoint. The previous
    /// logical link is retired and the caller must emit `LinkDown` before the
    /// new `LinkUp`.
    pub(crate) restarted: bool,
    /// No record existed for this stable LinkId before this attachment.
    pub(crate) link_was_new: bool,
}

#[derive(Debug, Default)]
struct LinkRecord {
    reader: Option<StreamToken>,
    writer: Option<StreamToken>,
    // Retain endpoint identities until the logical link closes. Silently
    // capping these sets could re-admit a delayed callback; their growth under
    // repeated same-direction replacement is therefore an explicit memory
    // follow-up, not hidden lossy eviction.
    retired_reader_endpoints: Vec<i64>,
    retired_writer_endpoints: Vec<i64>,
}

#[derive(Debug, Default)]
pub(crate) struct LinkLifecycle {
    next_generation: u64,
    records: HashMap<LinkId, LinkRecord>,
}

impl LinkLifecycle {
    fn allocate_generation(&mut self) -> u64 {
        self.next_generation = self.next_generation.checked_add(1).unwrap_or(1);
        self.next_generation
    }

    /// Attach one direction and return its current token.
    pub(crate) fn attach(
        &mut self,
        link: LinkId,
        side: StreamSide,
        endpoint: i64,
    ) -> Option<AttachResult> {
        if !self.records.contains_key(&link) {
            let token = StreamToken {
                link,
                generation: self.allocate_generation(),
                side,
                endpoint,
            };
            let mut record = LinkRecord::default();
            match side {
                StreamSide::Reader => record.reader = Some(token),
                StreamSide::Writer => record.writer = Some(token),
            }
            self.records.insert(link, record);
            return Some(AttachResult { token, restarted: false, link_was_new: true });
        }

        let record = self.records.get(&link).expect("record exists");
        let retired = match side {
            StreamSide::Reader => &record.retired_reader_endpoints,
            StreamSide::Writer => &record.retired_writer_endpoints,
        };
        if retired.contains(&endpoint) {
            return None;
        }
        let existing = self.records.get(&link).and_then(|record| match side {
            StreamSide::Reader => record.reader,
            StreamSide::Writer => record.writer,
        });
        if let Some(token) = existing {
            if token.endpoint == endpoint {
                // Duplicate control notifications must not spawn a second
                // reader over the same socket or replace a live writer.
                return None;
            }
        }

        let token = StreamToken {
            link,
            generation: self.allocate_generation(),
            side,
            endpoint,
        };
        let record = self.records.get_mut(&link).expect("record exists");
        let old = match side {
            StreamSide::Reader => record.reader.replace(token),
            StreamSide::Writer => record.writer.replace(token),
        };
        let restarted = old.is_some();
        if restarted {
            // A same-direction replacement is a new logical socket pair. The
            // opposite callback from the previous pair is stale as well.
            match side {
                StreamSide::Reader => {
                    if let Some(old) = old {
                        record.retired_reader_endpoints.push(old.endpoint);
                    }
                    if let Some(old) = record.writer.take() {
                        record.retired_writer_endpoints.push(old.endpoint);
                    }
                }
                StreamSide::Writer => {
                    if let Some(old) = old {
                        record.retired_writer_endpoints.push(old.endpoint);
                    }
                    if let Some(old) = record.reader.take() {
                        record.retired_reader_endpoints.push(old.endpoint);
                    }
                }
            }
        }
        Some(AttachResult { token, restarted, link_was_new: false })
    }

    pub(crate) fn is_current(&self, token: StreamToken) -> bool {
        let Some(record) = self.records.get(&token.link) else { return false };
        match token.side {
            StreamSide::Reader => record.reader == Some(token),
            StreamSide::Writer => record.writer == Some(token),
        }
    }

    /// Retire the entire logical link only when this exact endpoint is still
    /// current. A delayed callback from an older generation returns `false`.
    pub(crate) fn close_if_current(&mut self, token: StreamToken) -> bool {
        if !self.is_current(token) {
            return false;
        }
        self.records.remove(&token.link).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{LinkLifecycle, StreamSide};

    #[test]
    fn reader_and_writer_attach_to_one_generation() {
        let mut lifecycle = LinkLifecycle::default();
        let reader = lifecycle.attach(7, StreamSide::Reader, 11).expect("reader accepted");
        let writer = lifecycle.attach(7, StreamSide::Writer, 12).expect("writer accepted");

        assert!(reader.link_was_new);
        assert!(!writer.link_was_new);
        assert!(!writer.restarted);
        assert!(lifecycle.is_current(reader.token));
        assert!(lifecycle.is_current(writer.token));
    }

    #[test]
    fn stale_writer_cannot_close_replacement() {
        let mut lifecycle = LinkLifecycle::default();
        let old = lifecycle.attach(7, StreamSide::Writer, 12).expect("writer accepted").token;
        let replacement = lifecycle.attach(7, StreamSide::Writer, 13).expect("replacement accepted");

        assert!(replacement.restarted);
        assert!(!lifecycle.is_current(old));
        assert!(!lifecycle.close_if_current(old));
        assert!(lifecycle.is_current(replacement.token));
    }

    #[test]
    fn duplicate_endpoint_is_not_attached_twice() {
        let mut lifecycle = LinkLifecycle::default();
        assert!(lifecycle.attach(7, StreamSide::Reader, 15).is_some());
        assert!(lifecycle.attach(7, StreamSide::Reader, 15).is_none());
    }

    #[test]
    fn reconnect_generation_is_not_reused() {
        let mut lifecycle = LinkLifecycle::default();
        let old = lifecycle.attach(7, StreamSide::Reader, 21).expect("reader accepted").token;
        assert!(lifecycle.close_if_current(old));

        let replacement = lifecycle.attach(7, StreamSide::Reader, 21).expect("reader accepted").token;
        assert_ne!(old.generation, replacement.generation);
        assert!(!lifecycle.is_current(old));
        assert!(lifecycle.is_current(replacement));
    }

    #[test]
    fn stale_reader_data_is_rejected_after_replacement() {
        let mut lifecycle = LinkLifecycle::default();
        let old = lifecycle.attach(7, StreamSide::Reader, 31).expect("reader accepted").token;
        let replacement = lifecycle.attach(7, StreamSide::Reader, 32).expect("replacement accepted").token;

        assert!(!lifecycle.is_current(old));
        assert!(lifecycle.is_current(replacement));
        assert!(!lifecycle.close_if_current(old));
        assert!(lifecycle.is_current(replacement));
    }

    #[test]
    fn delayed_opposite_endpoint_is_rejected_after_replacement() {
        let mut lifecycle = LinkLifecycle::default();
        let old_reader = lifecycle.attach(7, StreamSide::Reader, 41).expect("reader accepted");
        let old_writer = lifecycle.attach(7, StreamSide::Writer, 42).expect("writer accepted");
        let replacement = lifecycle.attach(7, StreamSide::Reader, 43).expect("replacement accepted");

        assert!(!lifecycle.is_current(old_reader.token));
        assert!(!lifecycle.is_current(old_writer.token));
        assert!(lifecycle.attach(7, StreamSide::Writer, 42).is_none());
        assert!(lifecycle.is_current(replacement.token));
    }
}
