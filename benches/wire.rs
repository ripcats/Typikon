use std::{hint::black_box, time::Instant};

use typikon::{
    CID_BYTES, Decoder, Encoder, WireCodec, WireError, decode_value, encode_value, varint_len,
};

const MESSAGE_CID: [u8; CID_BYTES] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
const MESSAGE_ITERATIONS: usize = 1_000_000;
const BLOB_ITERATIONS: usize = 10_000;

#[derive(Clone)]
struct Message {
    id: u64,
    chat_id: u64,
    author: String,
    text: String,
    sent_at: u64,
}

impl WireCodec for Message {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        encoder.raw(&MESSAGE_CID)?;
        self.id.encode(encoder)?;
        self.chat_id.encode(encoder)?;
        self.author.encode(encoder)?;
        self.text.encode(encoder)?;
        self.sent_at.encode(encoder)
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_cid_bytes(&MESSAGE_CID)?;
        Ok(Self {
            id: decoder.value()?,
            chat_id: decoder.value()?,
            author: decoder.value()?,
            text: decoder.value()?,
            sent_at: decoder.value()?,
        })
    }

    fn encoded_len(&self) -> usize {
        CID_BYTES
            .saturating_add(self.id.encoded_len())
            .saturating_add(self.chat_id.encoded_len())
            .saturating_add(self.author.encoded_len())
            .saturating_add(self.text.encoded_len())
            .saturating_add(self.sent_at.encoded_len())
    }
}

fn decode_borrowed<'a>(wire: &'a [u8]) -> Result<(&'a str, &'a str), WireError> {
    let mut decoder = Decoder::new(wire, typikon::DEFAULT_MAX_PACKET_SIZE)?;
    decoder.expect_cid_bytes(&MESSAGE_CID)?;
    let _: u64 = decoder.value()?;
    let _: u64 = decoder.value()?;
    let author = decoder.string_borrowed()?;
    let text = decoder.string_borrowed()?;
    let _: u64 = decoder.value()?;
    if !decoder.is_finished() {
        return Err(WireError::MalformedConstructor);
    }
    Ok((author, text))
}

fn ns_per_iteration(iterations: usize, mut operation: impl FnMut()) -> f64 {
    let started = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    started.elapsed().as_nanos() as f64 / iterations as f64
}

fn main() {
    let message = Message {
        id: 7,
        chat_id: 42,
        author: "Ada".into(),
        text: "Hello from a binary protocol".into(),
        sent_at: 1_725_000_000_000,
    };
    let wire = encode_value(&message).unwrap();
    assert_eq!(wire.len(), message.encoded_len());

    let encode_ns = ns_per_iteration(MESSAGE_ITERATIONS, || {
        black_box(encode_value(black_box(&message)).unwrap());
    });
    let owned_decode_ns = ns_per_iteration(MESSAGE_ITERATIONS, || {
        black_box(decode_value::<Message>(black_box(&wire)).unwrap());
    });
    let borrowed_decode_ns = ns_per_iteration(MESSAGE_ITERATIONS, || {
        black_box(decode_borrowed(black_box(&wire)).unwrap());
    });

    let blob = vec![0xa5; 64 * 1024];
    let mut blob_encoder = Encoder::with_capacity(
        typikon::DEFAULT_MAX_PACKET_SIZE,
        varint_len(blob.len() as u64) + blob.len(),
    );
    blob_encoder.bytes(&blob).unwrap();
    let blob_wire = blob_encoder.finish().unwrap();
    let blob_encode_ns = ns_per_iteration(BLOB_ITERATIONS, || {
        let mut encoder = Encoder::with_capacity(
            typikon::DEFAULT_MAX_PACKET_SIZE,
            varint_len(blob.len() as u64) + blob.len(),
        );
        encoder.bytes(black_box(&blob)).unwrap();
        black_box(encoder.finish().unwrap());
    });
    let blob_decode_ns = ns_per_iteration(BLOB_ITERATIONS, || {
        let mut decoder =
            Decoder::new(black_box(&blob_wire), typikon::DEFAULT_MAX_PACKET_SIZE).unwrap();
        black_box(decoder.bytes_borrowed().unwrap());
    });

    println!("message_bytes={}", wire.len());
    println!("message_encode_ns={encode_ns:.2}");
    println!("message_owned_decode_ns={owned_decode_ns:.2}");
    println!("message_borrowed_decode_ns={borrowed_decode_ns:.2}");
    println!("blob_bytes={}", blob.len());
    println!("blob_encode_ns={blob_encode_ns:.2}");
    println!("blob_borrowed_decode_ns={blob_decode_ns:.2}");
}
