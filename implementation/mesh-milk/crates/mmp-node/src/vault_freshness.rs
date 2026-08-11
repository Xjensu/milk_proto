//! Test-only model of the provider freshness contract.
//!
//! This module deliberately does not implement a platform authority.  It is a
//! small reference model for the ordering and rejection rules that a future
//! provider must satisfy before it can be wired into [`super::SnapshotVault`].

use super::VaultError;
use mmp_types::ids::{Id16, Key32};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FreshnessScope {
    kind: u8,
    identity: Key32,
    node_id: Id16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FreshnessState {
    Uninitialized,
    Initialized { floor: u64, record_digest: [u8; 32] },
}

trait FreshnessAuthority {
    fn inspect(&mut self, scope: &FreshnessScope) -> Result<FreshnessState, VaultError>;

    fn advance_before_publish(
        &mut self,
        scope: &FreshnessScope,
        expected: u64,
        next: u64,
        record_digest: [u8; 32],
    ) -> Result<(), VaultError>;
}

/// Minimal in-memory authority used only to make the contract executable in
/// unit tests.  A real implementation must keep the equivalent state in a
/// platform-owned, independently durable freshness boundary.
#[derive(Debug, Clone)]
struct InMemoryFreshnessAuthority {
    scope: FreshnessScope,
    state: FreshnessState,
    available: bool,
    reset: bool,
}

impl InMemoryFreshnessAuthority {
    fn new(scope: FreshnessScope) -> Self {
        Self {
            scope,
            state: FreshnessState::Uninitialized,
            available: true,
            reset: false,
        }
    }

    fn state_for_test(&self) -> FreshnessState {
        self.state
    }

    fn set_available_for_test(&mut self, available: bool) {
        self.available = available;
    }

    fn mark_reset_for_test(&mut self) {
        self.reset = true;
    }

    fn set_state_for_test(&mut self, state: FreshnessState) {
        self.state = state;
    }

    fn check_access(&self, scope: &FreshnessScope) -> Result<(), VaultError> {
        if !self.available || self.reset {
            return Err(VaultError::Unavailable);
        }
        if scope != &self.scope {
            return Err(VaultError::Invalid);
        }
        Ok(())
    }
}

impl FreshnessAuthority for InMemoryFreshnessAuthority {
    fn inspect(&mut self, scope: &FreshnessScope) -> Result<FreshnessState, VaultError> {
        self.check_access(scope)?;
        Ok(self.state)
    }

    fn advance_before_publish(
        &mut self,
        scope: &FreshnessScope,
        expected: u64,
        next: u64,
        record_digest: [u8; 32],
    ) -> Result<(), VaultError> {
        self.check_access(scope)?;

        let expected_next = match self.state {
            FreshnessState::Uninitialized => {
                if expected != 0 {
                    return Err(VaultError::Generation);
                }
                1
            }
            FreshnessState::Initialized { floor, .. } => {
                floor.checked_add(1).ok_or(VaultError::Generation)?
            }
        };
        if next != expected_next || expected != next - 1 {
            return Err(VaultError::Generation);
        }

        // Commit only after every validation above has succeeded.  In
        // particular, a failed candidate must never move the authority floor.
        self.state = FreshnessState::Initialized {
            floor: next,
            record_digest,
        };
        Ok(())
    }
}

/// Validate that the journal image is exactly the record authenticated by the
/// authority.  This is a model-boundary check, not a production restore path.
fn validate_journal_match<A: FreshnessAuthority>(
    authority: &mut A,
    scope: &FreshnessScope,
    journal: Option<(u64, [u8; 32])>,
) -> Result<(), VaultError> {
    let authority_state = authority.inspect(scope)?;
    match (authority_state, journal) {
        (FreshnessState::Uninitialized, None) => Ok(()),
        (
            FreshnessState::Initialized {
                floor,
                record_digest,
            },
            Some((journal_floor, journal_digest)),
        ) if floor == journal_floor && record_digest == journal_digest => Ok(()),
        (FreshnessState::Initialized { .. }, None) => Err(VaultError::Generation),
        (FreshnessState::Initialized { floor, .. }, Some((journal_floor, _)))
            if floor != journal_floor =>
        {
            Err(VaultError::Generation)
        }
        (FreshnessState::Uninitialized, Some(_)) => Err(VaultError::Generation),
        // Equal generations with different digests are a fork, not a valid
        // successor.  Reject before any journal or in-memory state mutation.
        (FreshnessState::Initialized { .. }, Some(_)) => Err(VaultError::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(kind: u8, identity_byte: u8, node_byte: u8) -> FreshnessScope {
        FreshnessScope {
            kind,
            identity: Key32([identity_byte; 32]),
            node_id: Id16([node_byte; 16]),
        }
    }

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn explicit_scope_binding_rejects_wrong_scope_without_mutation() {
        let expected_scope = scope(1, 0x11, 0x22);
        let wrong_scope = scope(1, 0x33, 0x44);
        let mut authority = InMemoryFreshnessAuthority::new(expected_scope);

        assert_eq!(authority.inspect(&wrong_scope), Err(VaultError::Invalid));
        assert_eq!(
            authority.advance_before_publish(&wrong_scope, 0, 1, digest(1)),
            Err(VaultError::Invalid)
        );
        assert_eq!(authority.state_for_test(), FreshnessState::Uninitialized);
        assert_eq!(
            authority.inspect(&expected_scope),
            Ok(FreshnessState::Uninitialized)
        );
    }

    #[test]
    fn only_initialization_and_contiguous_successors_are_accepted() {
        let bound_scope = scope(7, 0xa1, 0xb2);
        let mut authority = InMemoryFreshnessAuthority::new(bound_scope);

        assert_eq!(
            authority.advance_before_publish(&bound_scope, 0, 1, digest(1)),
            Ok(())
        );
        assert_eq!(
            authority.advance_before_publish(&bound_scope, 1, 2, digest(2)),
            Ok(())
        );

        let accepted = authority.state_for_test();
        for (expected, next, candidate_digest) in [
            (0, 1, digest(9)), // repeated initialization/equal generation
            (2, 2, digest(3)), // equal generation
            (2, 4, digest(4)), // gap
            (3, 2, digest(5)), // decrease
        ] {
            assert_eq!(
                authority.advance_before_publish(&bound_scope, expected, next, candidate_digest),
                Err(VaultError::Generation)
            );
            assert_eq!(authority.state_for_test(), accepted);
        }
    }

    #[test]
    fn overflow_and_forked_generation_fail_without_mutation() {
        let bound_scope = scope(2, 0xc1, 0xd2);
        let mut authority = InMemoryFreshnessAuthority::new(bound_scope);
        authority.set_state_for_test(FreshnessState::Initialized {
            floor: u64::MAX,
            record_digest: digest(1),
        });
        let before = authority.state_for_test();

        assert_eq!(
            authority.advance_before_publish(&bound_scope, u64::MAX, 0, digest(2)),
            Err(VaultError::Generation)
        );
        assert_eq!(authority.state_for_test(), before);

        authority.set_state_for_test(FreshnessState::Initialized {
            floor: 4,
            record_digest: digest(4),
        });
        let before_fork = authority.state_for_test();
        assert_eq!(
            validate_journal_match(&mut authority, &bound_scope, Some((4, digest(5)))),
            Err(VaultError::Invalid)
        );
        assert_eq!(authority.state_for_test(), before_fork);
    }

    #[test]
    fn unavailable_and_reset_authority_fail_closed_without_mutation() {
        let bound_scope = scope(3, 0xe1, 0xf2);
        let mut authority = InMemoryFreshnessAuthority::new(bound_scope);
        authority
            .advance_before_publish(&bound_scope, 0, 1, digest(1))
            .expect("initialization should succeed");
        let before = authority.state_for_test();

        authority.set_available_for_test(false);
        assert_eq!(
            authority.inspect(&bound_scope),
            Err(VaultError::Unavailable)
        );
        assert_eq!(
            authority.advance_before_publish(&bound_scope, 1, 2, digest(2)),
            Err(VaultError::Unavailable)
        );
        assert_eq!(authority.state_for_test(), before);

        authority.set_available_for_test(true);
        authority.mark_reset_for_test();
        assert_eq!(
            validate_journal_match(&mut authority, &bound_scope, Some((1, digest(1)))),
            Err(VaultError::Unavailable)
        );
        assert_eq!(
            authority.advance_before_publish(&bound_scope, 1, 2, digest(2)),
            Err(VaultError::Unavailable)
        );
        assert_eq!(authority.state_for_test(), before);
    }

    #[test]
    fn journal_boundary_requires_exact_authority_floor_and_digest() {
        let bound_scope = scope(4, 0x12, 0x34);
        let mut authority = InMemoryFreshnessAuthority::new(bound_scope);
        assert_eq!(
            validate_journal_match(&mut authority, &bound_scope, None),
            Ok(())
        );
        assert_eq!(
            validate_journal_match(&mut authority, &bound_scope, Some((1, digest(1)))),
            Err(VaultError::Generation)
        );

        authority
            .advance_before_publish(&bound_scope, 0, 1, digest(1))
            .expect("initialization should succeed");
        authority
            .advance_before_publish(&bound_scope, 1, 2, digest(2))
            .expect("successor should succeed");

        // The authority is newer than the journal: never lower the authority
        // or fall back to the older, otherwise-valid image.
        assert_eq!(
            validate_journal_match(&mut authority, &bound_scope, Some((1, digest(1)))),
            Err(VaultError::Generation)
        );
        // A same-generation record with a different digest is a fork.
        assert_eq!(
            validate_journal_match(&mut authority, &bound_scope, Some((2, digest(9)))),
            Err(VaultError::Invalid)
        );
        assert_eq!(
            authority.inspect(&bound_scope),
            Ok(FreshnessState::Initialized {
                floor: 2,
                record_digest: digest(2),
            })
        );
    }
}
