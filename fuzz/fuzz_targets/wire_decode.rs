#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;
use typikon::{Decoder, WireCodec, MAX_PACKET_SIZE};

fuzz_target!(|data: &[u8]| {
    if let Ok(mut decoder) = Decoder::new(data, MAX_PACKET_SIZE) {
        let _ = decoder.varint();
        let _ = decoder.bytes();
        let _ = Vec::<u16>::decode(&mut decoder);
        let _ = BTreeMap::<u8, u8>::decode(&mut decoder);
    }
});
