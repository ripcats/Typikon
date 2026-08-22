use std::{collections::BTreeMap, hint::black_box, time::Instant};

use typikon::{
    BorrowedWireCodec, CID_BYTES, Decoder, Encoder, WireCodec, WireError, decode_value,
    encode_value, varint_len,
};

const MESSAGE_CID: [u8; CID_BYTES] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
const MESSAGE_ITERATIONS: usize = 1_000_000;
const BLOB_ITERATIONS: usize = 10_000;
const COLLECTION_ITERATIONS: usize = 100_000;

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

fn decode_borrowed(wire: &[u8]) -> Result<(&str, &str), WireError> {
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

const COLLECTION_CID: [u8; CID_BYTES] = [0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04];
const ATTACHMENT_CID: [u8; CID_BYTES] = [0xca, 0xfe, 0xba, 0xbe, 0x05, 0x06, 0x07, 0x08];

#[derive(Clone)]
struct Attachment {
    name: String,
    mime: String,
    size: u64,
}

impl WireCodec for Attachment {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        encoder.raw(&ATTACHMENT_CID)?;
        self.name.encode(encoder)?;
        self.mime.encode(encoder)?;
        self.size.encode(encoder)
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_cid_bytes(&ATTACHMENT_CID)?;
        Ok(Self {
            name: decoder.value()?,
            mime: decoder.value()?,
            size: decoder.value()?,
        })
    }

    fn encoded_len(&self) -> usize {
        CID_BYTES + self.name.encoded_len() + self.mime.encoded_len() + self.size.encoded_len()
    }
}

struct BorrowedAttachment<'a> {
    name: &'a str,
    mime: &'a str,
    size: u64,
}

impl<'a> BorrowedWireCodec<'a> for BorrowedAttachment<'a> {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        decoder.expect_cid_bytes(&ATTACHMENT_CID)?;
        Ok(Self {
            name: decoder.string_borrowed()?,
            mime: decoder.string_borrowed()?,
            size: decoder.value()?,
        })
    }
}

struct CollectionMessage {
    id: u64,
    roles: Vec<String>,
    attachments: Vec<Attachment>,
    metadata: BTreeMap<String, String>,
}

impl WireCodec for CollectionMessage {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        encoder.raw(&COLLECTION_CID)?;
        self.id.encode(encoder)?;
        self.roles.encode(encoder)?;
        self.attachments.encode(encoder)?;
        self.metadata.encode(encoder)
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_cid_bytes(&COLLECTION_CID)?;
        Ok(Self {
            id: decoder.value()?,
            roles: decoder.value()?,
            attachments: decoder.value()?,
            metadata: decoder.value()?,
        })
    }

    fn encoded_len(&self) -> usize {
        CID_BYTES
            + self.id.encoded_len()
            + self.roles.encoded_len()
            + self.attachments.encoded_len()
            + self.metadata.encoded_len()
    }
}

struct BorrowedCollectionMessage<'a> {
    id: u64,
    roles: typikon::BorrowedVec<'a, &'a str>,
    attachments: typikon::BorrowedVec<'a, BorrowedAttachment<'a>>,
    metadata: typikon::BorrowedMap<'a, &'a str, &'a str>,
}

impl<'a> BorrowedWireCodec<'a> for BorrowedCollectionMessage<'a> {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        decoder.expect_cid_bytes(&COLLECTION_CID)?;
        Ok(Self {
            id: decoder.value()?,
            roles: decoder.borrowed_vec()?,
            attachments: decoder.borrowed_vec()?,
            metadata: decoder.borrowed_map()?,
        })
    }
}

fn decode_borrowed_collection<'a>(
    wire: &'a [u8],
) -> Result<BorrowedCollectionMessage<'a>, WireError> {
    typikon::decode_borrowed_value(wire)
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

    let collection = CollectionMessage {
        id: 9,
        roles: vec!["admin".into(), "moderator".into(), "support".into()],
        attachments: (0..8)
            .map(|index| Attachment {
                name: format!("photo-{index}"),
                mime: "image/jpeg".into(),
                size: 4096 + index,
            })
            .collect(),
        metadata: BTreeMap::from([
            ("client".into(), "web".into()),
            ("locale".into(), "en".into()),
            ("trace".into(), "benchmark".into()),
        ]),
    };
    let collection_wire = encode_value(&collection).unwrap();
    let collection_owned_decode_ns = ns_per_iteration(COLLECTION_ITERATIONS, || {
        black_box(decode_value::<CollectionMessage>(black_box(&collection_wire)).unwrap());
    });
    let collection_borrowed_decode_ns = ns_per_iteration(COLLECTION_ITERATIONS, || {
        black_box(decode_borrowed_collection(black_box(&collection_wire)).unwrap());
    });
    let collection_borrowed_iterate_ns = ns_per_iteration(COLLECTION_ITERATIONS, || {
        let view = decode_borrowed_collection(black_box(&collection_wire)).unwrap();
        let mut total = view.id as usize;
        for role in view.roles.iter() {
            total += role.unwrap().len();
        }
        for attachment in view.attachments.iter() {
            let attachment = attachment.unwrap();
            total += attachment.name.len() + attachment.mime.len() + attachment.size as usize;
        }
        for entry in view.metadata.iter() {
            let (key, value) = entry.unwrap();
            total += key.len() + value.len();
        }
        black_box(total);
    });

    println!("message_bytes={}", wire.len());
    println!("message_encode_ns={encode_ns:.2}");
    println!("message_owned_decode_ns={owned_decode_ns:.2}");
    println!("message_borrowed_decode_ns={borrowed_decode_ns:.2}");
    println!("blob_bytes={}", blob.len());
    println!("blob_encode_ns={blob_encode_ns:.2}");
    println!("blob_borrowed_decode_ns={blob_decode_ns:.2}");
    println!("collection_bytes={}", collection_wire.len());
    println!("collection_owned_decode_ns={collection_owned_decode_ns:.2}");
    println!("collection_borrowed_decode_ns={collection_borrowed_decode_ns:.2}");
    println!("collection_borrowed_iterate_ns={collection_borrowed_iterate_ns:.2}");
}
