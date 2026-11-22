//! `cargo fuzz run e57` – see fuzz/README.md and `teamcraft::fuzz_harness::e57`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use teamcraft::fuzz_harness::{e57, guarded, quiet_panics};

fuzz_target!(init: quiet_panics(), |data: &[u8]| guarded(e57, data));
