//! Shared helpers for the parser tests.
//!
//! Each test binary includes this module, so some binaries use only one of the
//! two builders. The allow keeps that from tripping the unused warning.
#![allow(dead_code)]

use json_schema_to_zod::{Refs, ZodVersion};

/// Default v4 refs, matching `{ path: [], seen: new Map() }`.
pub fn refs_v4() -> Refs {
    Refs::default_v4()
}

/// Default v3 refs, matching `{ path: [], seen: new Map(), zodVersion: 3 }`.
pub fn refs_v3() -> Refs {
    let mut r = Refs::default_v4();
    r.zod_version = ZodVersion::V3;
    r
}
