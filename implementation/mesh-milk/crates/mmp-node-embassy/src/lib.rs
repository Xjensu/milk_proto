//! mmp-node-embassy — embedded orchestration on the Embassy executor.
//!
//! Status: out of MVP scope (HANDOFF §0, with `mmp-tal-embedded`). The core
//! stack is sans-IO with an injected monotonic clock, so the Embassy port is
//! an executor shell around the same engines — no protocol code lands here.

#![no_std]
