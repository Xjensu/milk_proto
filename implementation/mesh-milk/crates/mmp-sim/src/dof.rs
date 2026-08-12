//! Simulator-only degree-of-freedom (DoF) transfer model for MMP/Φ.
//!
//! This module models a bounded receiver ledger and a deterministic sequence
//! of contact allocations.  It deliberately sits above the wire and node
//! implementations: it does not mint bundles, change MMP/1 routing, or
//! define a new coding primitive.  Reed--Solomon encoding and decoding are
//! delegated to [`mmp_rs_code`], the existing workspace implementation.
//!
//! The model treats each distinct Reed--Solomon symbol index as one useful
//! degree of freedom.  A receiver stores at most `k` symbols, so duplicate,
//! malformed, and over-cap inputs cannot grow its rank or memory footprint.
//! The planner emits each index at most once and consumes only the configured
//! contact budget.  These are research-model invariants, not production
//! protocol guarantees.

use mmp_rs_code::{MAX_K, MAX_N};

/// Maximum symbol payload accepted by the bounded research ledger.
///
/// The cap keeps malformed experiment fixtures from allocating unbounded
/// memory.  It is intentionally independent of the frozen MMP/1 wire format.
pub const MAX_DOF_SYMBOL_BYTES: usize = 4 * 1024;

/// Result of offering one symbol to a [`DofLedger`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOutcome {
    /// The symbol index was new and increased the receiver rank.
    Useful { received_rank: usize },
    /// The same index and bytes were already retained; rank is unchanged.
    Duplicate,
}

/// Validation or bounded-state failures in the research model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DofError {
    /// `k`, `n`, or symbol length was outside the model's finite bounds.
    InvalidParameters,
    /// A planner budget exceeded the available encoded symbol index space.
    BudgetExceedsSymbolSpace,
    /// A symbol index was not in `0..n`.
    SymbolIndexOutOfRange,
    /// A symbol did not have the configured fixed length.
    SymbolLengthMismatch,
    /// A duplicate index carried bytes different from the retained symbol.
    ConflictingDuplicate,
    /// A new symbol arrived after the ledger had already reached rank `k`.
    RankComplete,
    /// Decoding was requested before `k` useful symbols had been received.
    InsufficientRank,
    /// The delegated Reed--Solomon decoder rejected the validated ledger.
    DecodeFailed,
}

/// Research-only summary of a bounded receiver ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DofLedgerStats {
    /// Data-symbol count required for reconstruction.
    pub k: usize,
    /// Number of distinct symbols retained, capped at `k`.
    pub received_rank: usize,
    /// Number of accepted symbols that contributed a new degree of freedom.
    pub useful_symbols: usize,
    /// Bytes retained by the ledger (`received_rank * symbol_bytes`).
    pub bytes_received: usize,
    /// Whether the ledger has reached rank `k`.
    pub completed: bool,
}

/// A bounded receiver-side degree-of-freedom ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofLedger {
    k: usize,
    n: usize,
    symbol_bytes: usize,
    symbols: Vec<Option<Vec<u8>>>,
    received_rank: usize,
}

impl DofLedger {
    /// Create an empty ledger for an `(k, n)` Reed--Solomon block.
    pub fn new(k: usize, n: usize, symbol_bytes: usize) -> Result<Self, DofError> {
        if !(1..=MAX_K).contains(&k)
            || n < k
            || n > MAX_N
            || symbol_bytes == 0
            || symbol_bytes > MAX_DOF_SYMBOL_BYTES
        {
            return Err(DofError::InvalidParameters);
        }
        Ok(Self {
            k,
            n,
            symbol_bytes,
            symbols: vec![None; n],
            received_rank: 0,
        })
    }

    /// Required data-symbol count.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Total encoded-symbol count accepted by this ledger.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Fixed symbol payload length.
    pub fn symbol_bytes(&self) -> usize {
        self.symbol_bytes
    }

    /// Number of distinct useful symbols retained, never greater than `k`.
    pub fn received_rank(&self) -> usize {
        self.received_rank
    }

    /// Alias used by reports that describe rank as useful symbols.
    pub fn useful_symbols(&self) -> usize {
        self.received_rank
    }

    /// Whether enough distinct symbols are retained to reconstruct the block.
    pub fn is_complete(&self) -> bool {
        self.received_rank == self.k
    }

    /// Return a bounded, copyable research summary.
    pub fn stats(&self) -> DofLedgerStats {
        DofLedgerStats {
            k: self.k,
            received_rank: self.received_rank,
            useful_symbols: self.useful_symbols(),
            bytes_received: self.received_rank * self.symbol_bytes,
            completed: self.is_complete(),
        }
    }

    /// Whether this index is already retained.
    pub fn contains(&self, index: u8) -> bool {
        self.symbols
            .get(index as usize)
            .is_some_and(Option::is_some)
    }

    /// Offer one encoded symbol to the ledger.
    ///
    /// Validation happens before mutation.  Equal duplicates are harmless and
    /// return [`IngestOutcome::Duplicate`]; conflicting bytes for an existing
    /// index fail closed instead of allowing an ambiguous decoder input.  A
    /// new index is rejected after rank `k`, keeping the receiver allocation
    /// bounded even when a peer sends more than the useful amount.
    pub fn ingest(&mut self, index: u8, symbol: &[u8]) -> Result<IngestOutcome, DofError> {
        let slot = self
            .symbols
            .get_mut(index as usize)
            .ok_or(DofError::SymbolIndexOutOfRange)?;
        if symbol.len() != self.symbol_bytes {
            return Err(DofError::SymbolLengthMismatch);
        }
        match slot {
            Some(existing) if existing == symbol => Ok(IngestOutcome::Duplicate),
            Some(_) => Err(DofError::ConflictingDuplicate),
            None if self.received_rank == self.k => Err(DofError::RankComplete),
            None => {
                *slot = Some(symbol.to_vec());
                self.received_rank += 1;
                Ok(IngestOutcome::Useful {
                    received_rank: self.received_rank,
                })
            }
        }
    }

    /// Decode the retained block using the existing workspace RS decoder.
    pub fn decode(&self) -> Result<Vec<Vec<u8>>, DofError> {
        if !self.is_complete() {
            return Err(DofError::InsufficientRank);
        }
        let received = self
            .symbols
            .iter()
            .enumerate()
            .filter_map(|(index, symbol)| symbol.as_ref().map(|bytes| (index as u8, bytes.clone())))
            .collect::<Vec<_>>();
        mmp_rs_code::decode(&received, self.k).ok_or(DofError::DecodeFailed)
    }
}

/// One deterministic contact allocation produced by [`DofContactPlanner`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofContact {
    /// Encoded symbol indices to offer during this contact.
    pub indices: Vec<u8>,
    /// Planner budget before this allocation.
    pub budget_before: usize,
    /// Planner budget after this allocation.
    pub budget_after: usize,
    /// Bytes represented by the allocated symbols.
    pub bytes: usize,
}

/// Research-only summary of planner use and remaining budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DofPlannerStats {
    /// Total symbol budget configured for this plan.
    pub symbol_budget: usize,
    /// Number of unique indices allocated so far.
    pub symbols_planned: usize,
    /// Bytes represented by all allocations so far.
    pub bytes_planned: usize,
    /// Symbols still available to allocate.
    pub remaining_budget: usize,
}

/// Deterministic, bounded planner for symbols sent across multiple contacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofContactPlanner {
    n: usize,
    symbol_bytes: usize,
    symbol_budget: usize,
    remaining_budget: usize,
    next_index: usize,
    symbols_planned: usize,
}

impl DofContactPlanner {
    /// Create a planner that emits each index in ascending order at most once.
    ///
    /// `symbol_budget` is a total budget across all future contacts, not a
    /// per-contact allowance.  Requiring it to fit in `n` makes duplicate
    /// allocation impossible by construction.
    pub fn new(n: usize, symbol_bytes: usize, symbol_budget: usize) -> Result<Self, DofError> {
        if n == 0 || n > MAX_N || symbol_bytes == 0 || symbol_bytes > MAX_DOF_SYMBOL_BYTES {
            return Err(DofError::InvalidParameters);
        }
        if symbol_budget > n {
            return Err(DofError::BudgetExceedsSymbolSpace);
        }
        Ok(Self {
            n,
            symbol_bytes,
            symbol_budget,
            remaining_budget: symbol_budget,
            next_index: 0,
            symbols_planned: 0,
        })
    }

    /// Allocate up to `requested_symbols` from the remaining total budget.
    ///
    /// Requests larger than the remaining budget are clamped, never wrapped or
    /// over-allocated.  A zero-sized request is a valid empty contact.
    pub fn plan_contact(&mut self, requested_symbols: usize) -> DofContact {
        let budget_before = self.remaining_budget;
        let available_indices = self.n.saturating_sub(self.next_index);
        let count = requested_symbols
            .min(self.remaining_budget)
            .min(available_indices);
        let start = self.next_index;
        let indices = (start..start + count)
            .map(|index| index as u8)
            .collect::<Vec<_>>();
        self.next_index += count;
        self.remaining_budget -= count;
        self.symbols_planned += count;
        DofContact {
            indices,
            budget_before,
            budget_after: self.remaining_budget,
            bytes: count * self.symbol_bytes,
        }
    }

    /// Return a bounded summary of planner use.
    pub fn stats(&self) -> DofPlannerStats {
        DofPlannerStats {
            symbol_budget: self.symbol_budget,
            symbols_planned: self.symbols_planned,
            bytes_planned: self.symbols_planned * self.symbol_bytes,
            remaining_budget: self.remaining_budget,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_symbols_do_not_raise_rank_and_conflicts_fail_closed() {
        let mut ledger = DofLedger::new(3, 5, 4).unwrap();
        let symbol = [1, 2, 3, 4];
        assert_eq!(
            ledger.ingest(2, &symbol),
            Ok(IngestOutcome::Useful { received_rank: 1 })
        );
        assert_eq!(ledger.ingest(2, &symbol), Ok(IngestOutcome::Duplicate));
        assert_eq!(ledger.received_rank(), 1);
        assert_eq!(
            ledger.ingest(2, &[1, 2, 3, 5]),
            Err(DofError::ConflictingDuplicate)
        );
    }

    #[test]
    fn rank_is_bounded_at_k_even_when_n_is_larger() {
        let mut ledger = DofLedger::new(2, 5, 1).unwrap();
        assert_eq!(
            ledger.ingest(0, &[0]),
            Ok(IngestOutcome::Useful { received_rank: 1 })
        );
        assert_eq!(
            ledger.ingest(4, &[4]),
            Ok(IngestOutcome::Useful { received_rank: 2 })
        );
        assert!(ledger.is_complete());
        assert_eq!(ledger.ingest(1, &[1]), Err(DofError::RankComplete));
        assert_eq!(ledger.received_rank(), 2);
        assert_eq!(ledger.stats().bytes_received, 2);
    }

    #[test]
    fn invalid_parameters_and_symbols_are_rejected_without_mutation() {
        assert_eq!(DofLedger::new(0, 1, 1), Err(DofError::InvalidParameters));
        assert_eq!(DofLedger::new(3, 2, 1), Err(DofError::InvalidParameters));
        assert_eq!(DofLedger::new(1, 1, 0), Err(DofError::InvalidParameters));
        assert_eq!(
            DofContactPlanner::new(4, 1, 5),
            Err(DofError::BudgetExceedsSymbolSpace)
        );

        let mut ledger = DofLedger::new(2, 4, 2).unwrap();
        assert_eq!(
            ledger.ingest(4, &[1, 2]),
            Err(DofError::SymbolIndexOutOfRange)
        );
        assert_eq!(ledger.ingest(1, &[1]), Err(DofError::SymbolLengthMismatch));
        assert_eq!(ledger.received_rank(), 0);
    }

    #[test]
    fn contact_planning_is_deterministic_and_never_over_allocates() {
        let mut first = DofContactPlanner::new(6, 8, 4).unwrap();
        let mut second = DofContactPlanner::new(6, 8, 4).unwrap();
        assert_eq!(first.plan_contact(2), second.plan_contact(2));
        let first_last = first.plan_contact(99);
        let second_last = second.plan_contact(99);
        assert_eq!(first_last, second_last);
        assert_eq!(first_last.indices, vec![2, 3]);
        assert_eq!(first_last.budget_before, 2);
        assert_eq!(first_last.budget_after, 0);
        assert_eq!(first_last.bytes, 16);
        let empty = first.plan_contact(1);
        assert!(empty.indices.is_empty());
        assert_eq!(first.stats().symbols_planned, 4);
        assert_eq!(first.stats().bytes_planned, 32);
        assert_eq!(first.stats().remaining_budget, 0);
    }

    #[test]
    fn lossy_multi_contact_plan_reconstructs_with_existing_rs_decoder() {
        let k = 4;
        let n = 6;
        let symbol_bytes = 8;
        let data = (0..k)
            .map(|index| vec![index as u8 + 1; symbol_bytes])
            .collect::<Vec<_>>();
        let encoded = mmp_rs_code::encode(&data, n);
        let mut planner = DofContactPlanner::new(n, symbol_bytes, n).unwrap();
        let mut ledger = DofLedger::new(k, n, symbol_bytes).unwrap();

        // Two contacts lose one symbol each; the third carries parity.  Four
        // distinct surviving symbols are enough for RS reconstruction.
        for (contact_number, dropped_index) in [(2, 0u8), (2, 2u8), (2, 255u8)] {
            let contact = planner.plan_contact(contact_number);
            for index in contact.indices {
                if index != dropped_index {
                    ledger.ingest(index, &encoded[index as usize]).unwrap();
                }
            }
        }

        assert_eq!(ledger.received_rank(), k);
        assert_eq!(ledger.stats().useful_symbols, 4);
        assert!(ledger.is_complete());
        assert_eq!(ledger.decode().unwrap(), data);
        assert_eq!(planner.stats().symbols_planned, n);
        assert_eq!(planner.stats().bytes_planned, n * symbol_bytes);
    }
}
