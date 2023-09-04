#![no_main]

use celestia::shares::Share;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    serde_json::from_slice::<Share>(data).ok();
});
