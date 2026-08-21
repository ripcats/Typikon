#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;
use typikon::{CID_BYTES, Decoder, MAX_PACKET_SIZE, WireCodec};

fuzz_target!(|data: &[u8]| {
    for operation in 0..10 {
        let Ok(mut decoder) = Decoder::new(data, MAX_PACKET_SIZE) else {
            return;
        };
        match operation {
            0 => {
                let _ = decoder.varint();
            }
            1 => {
                let _ = decoder.bytes();
            }
            2 => {
                let _ = decoder.bytes_borrowed();
            }
            3 => {
                let _ = decoder.string_borrowed();
            }
            4 => {
                let _ = decoder.read_cid_bytes();
            }
            5 => {
                let _ = decoder.expect_cid_bytes(&[0; CID_BYTES]);
            }
            6 => {
                let _ = Vec::<u16>::decode(&mut decoder);
            }
            7 => {
                let _ = Vec::<String>::decode(&mut decoder);
            }
            8 => {
                let _ = BTreeMap::<u8, u8>::decode(&mut decoder);
            }
            _ => {
                let _ = BTreeMap::<String, Vec<u64>>::decode(&mut decoder);
            }
        }
    }
});
