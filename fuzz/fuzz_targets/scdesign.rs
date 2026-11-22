//! `cargo fuzz run scdesign` – see fuzz/README.md and `teamcraft::fuzz_harness::scdesign`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use teamcraft::fuzz_harness::{guarded, quiet_panics, scdesign};

fuzz_target!(init: quiet_panics(), |data: &[u8]| guarded(scdesign, data));
