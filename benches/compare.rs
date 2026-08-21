use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::BTreeMap,
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use typikon::{BorrowedWireCodec, Decoder, Encoder, WireError};

#[allow(
    dead_code,
    mismatched_lifetime_syntaxes,
    unused_imports,
    unsafe_op_in_unsafe_fn
)]
mod flatbuffers_generated {
    include!("generated/collection_generated.rs");
}

use flatbuffers_generated::typikon_bench as fb;

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const ITERATIONS: usize = 100_000;
const TL_COLLECTION: u32 = 0x1020_3040;
const TL_ATTACHMENT: u32 = 0x5060_7080;
const TL_VECTOR: u32 = 0x1cb5c415;
const TYPIKON_COLLECTION: [u8; 8] = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
const TYPIKON_ATTACHMENT: [u8; 8] = [0x80, 0x70, 0x60, 0x50, 0x40, 0x30, 0x20, 0x10];

struct Data {
    roles: Vec<String>,
    attachments: Vec<(String, String, u64)>,
    metadata: BTreeMap<String, String>,
}

struct TypikonAttachmentView<'a> {
    name: &'a str,
    mime: &'a str,
    size: u64,
}

impl<'a> BorrowedWireCodec<'a> for TypikonAttachmentView<'a> {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        decoder.expect_cid_bytes(&TYPIKON_ATTACHMENT)?;
        Ok(Self {
            name: decoder.string_borrowed()?,
            mime: decoder.string_borrowed()?,
            size: decoder.value()?,
        })
    }
}

struct TypikonView<'a> {
    roles: typikon::BorrowedVec<'a, &'a str>,
    attachments: typikon::BorrowedVec<'a, TypikonAttachmentView<'a>>,
    metadata: typikon::BorrowedMap<'a, &'a str, &'a str>,
}

impl<'a> BorrowedWireCodec<'a> for TypikonView<'a> {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        decoder.expect_cid_bytes(&TYPIKON_COLLECTION)?;
        let _: u64 = decoder.value()?;
        Ok(Self {
            roles: decoder.borrowed_vec()?,
            attachments: decoder.borrowed_vec()?,
            metadata: decoder.borrowed_map()?,
        })
    }
}

fn data() -> Data {
    Data {
        roles: ["admin", "moderator", "support"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        attachments: (0..8)
            .map(|index| (format!("photo-{index}"), "image/jpeg".into(), 4096 + index))
            .collect(),
        metadata: BTreeMap::from([
            ("client".into(), "web".into()),
            ("locale".into(), "en".into()),
            ("trace".into(), "benchmark".into()),
        ]),
    }
}

fn ns_per_iteration(iterations: usize, mut operation: impl FnMut()) -> f64 {
    let started = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    started.elapsed().as_nanos() as f64 / iterations as f64
}

fn allocation_count(operation: impl FnOnce()) -> usize {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    operation();
    ALLOCATIONS.load(Ordering::Relaxed)
}

fn build_flatbuffers(value: &Data) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let roles = value
        .roles
        .iter()
        .map(|role| builder.create_string(role))
        .collect::<Vec<_>>();
    let attachment_offsets = value
        .attachments
        .iter()
        .map(|(name, mime, size)| {
            let name = builder.create_string(name);
            let mime = builder.create_string(mime);
            fb::Attachment::create(
                &mut builder,
                &fb::AttachmentArgs {
                    name: Some(name),
                    mime: Some(mime),
                    size_: *size,
                },
            )
        })
        .collect::<Vec<_>>();
    let keys = value
        .metadata
        .keys()
        .map(|key| builder.create_string(key))
        .collect::<Vec<_>>();
    let values = value
        .metadata
        .values()
        .map(|value| builder.create_string(value))
        .collect::<Vec<_>>();
    let roles = builder.create_vector(&roles);
    let attachments = builder.create_vector(&attachment_offsets);
    let keys = builder.create_vector(&keys);
    let values = builder.create_vector(&values);
    let root = fb::CollectionMessage::create(
        &mut builder,
        &fb::CollectionMessageArgs {
            id: 9,
            roles: Some(roles),
            attachments: Some(attachments),
            metadata_keys: Some(keys),
            metadata_values: Some(values),
        },
    );
    fb::finish_collection_message_buffer(&mut builder, root);
    builder.finished_data().to_vec()
}

fn build_typikon(value: &Data) -> Vec<u8> {
    let mut encoder = Encoder::with_capacity(typikon::DEFAULT_MAX_PACKET_SIZE, 512);
    encoder.raw(&TYPIKON_COLLECTION).unwrap();
    encoder.u64(9).unwrap();
    encoder.varint(value.roles.len() as u64).unwrap();
    for role in &value.roles {
        encoder.bytes(role.as_bytes()).unwrap();
    }
    encoder.varint(value.attachments.len() as u64).unwrap();
    for (name, mime, size) in &value.attachments {
        encoder.raw(&TYPIKON_ATTACHMENT).unwrap();
        encoder.bytes(name.as_bytes()).unwrap();
        encoder.bytes(mime.as_bytes()).unwrap();
        encoder.u64(*size).unwrap();
    }
    encoder.varint(value.metadata.len() as u64).unwrap();
    for (key, value) in &value.metadata {
        encoder.bytes(key.as_bytes()).unwrap();
        encoder.bytes(value.as_bytes()).unwrap();
    }
    encoder.finish().unwrap()
}

fn decode_typikon_owned(bytes: &[u8]) -> Data {
    let mut decoder = Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE).unwrap();
    decoder.expect_cid_bytes(&TYPIKON_COLLECTION).unwrap();
    black_box(decoder.u64().unwrap());
    let roles = decoder.value().unwrap();
    let count = decoder.varint().unwrap() as usize;
    let attachments = (0..count)
        .map(|_| {
            decoder.expect_cid_bytes(&TYPIKON_ATTACHMENT).unwrap();
            (
                decoder.string().unwrap(),
                decoder.string().unwrap(),
                decoder.u64().unwrap(),
            )
        })
        .collect();
    let count = decoder.varint().unwrap() as usize;
    let mut metadata = BTreeMap::new();
    for _ in 0..count {
        let key = decoder.string().unwrap();
        let value = decoder.string().unwrap();
        metadata.insert(key, value);
    }
    Data {
        roles,
        attachments,
        metadata,
    }
}

fn iterate_typikon(view: &TypikonView<'_>) -> usize {
    let mut total = 0;
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
    total
}

fn decode_flat_owned(bytes: &[u8]) -> Data {
    let root = fb::root_as_collection_message(bytes).unwrap();
    let roles = root
        .roles()
        .unwrap()
        .iter()
        .map(|role| role.to_owned())
        .collect();
    let attachments = root
        .attachments()
        .unwrap()
        .iter()
        .map(|attachment| {
            (
                attachment.name().unwrap().to_owned(),
                attachment.mime().unwrap().to_owned(),
                attachment.size_(),
            )
        })
        .collect();
    let keys = root.metadata_keys().unwrap();
    let values = root.metadata_values().unwrap();
    let metadata = (0..keys.len())
        .map(|index| (keys.get(index).to_owned(), values.get(index).to_owned()))
        .collect();
    black_box(root.id());
    Data {
        roles,
        attachments,
        metadata,
    }
}

fn iterate_flat(bytes: &[u8]) -> usize {
    let root = fb::root_as_collection_message(bytes).unwrap();
    let mut total = root.id() as usize;
    for role in root.roles().unwrap().iter() {
        total += role.len();
    }
    for attachment in root.attachments().unwrap().iter() {
        total += attachment.name().unwrap().len()
            + attachment.mime().unwrap().len()
            + attachment.size_() as usize;
    }
    for index in 0..root.metadata_keys().unwrap().len() {
        total += root.metadata_keys().unwrap().get(index).len();
        total += root.metadata_values().unwrap().get(index).len();
    }
    total
}

fn put_tl_string(output: &mut Vec<u8>, value: &str) {
    assert!(value.len() < 254);
    output.push(value.len() as u8);
    output.extend_from_slice(value.as_bytes());
    while !output.len().is_multiple_of(4) {
        output.push(0);
    }
}

fn put_tl_vector<T>(output: &mut Vec<u8>, values: &[T], mut encode: impl FnMut(&mut Vec<u8>, &T)) {
    output.extend_from_slice(&TL_VECTOR.to_le_bytes());
    output.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        encode(output, value);
    }
}

fn build_tl(value: &Data) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&TL_COLLECTION.to_le_bytes());
    output.extend_from_slice(&9u64.to_le_bytes());
    put_tl_vector(&mut output, &value.roles, |output, role| {
        put_tl_string(output, role)
    });
    put_tl_vector(&mut output, &value.attachments, |output, attachment| {
        output.extend_from_slice(&TL_ATTACHMENT.to_le_bytes());
        put_tl_string(output, &attachment.0);
        put_tl_string(output, &attachment.1);
        output.extend_from_slice(&attachment.2.to_le_bytes());
    });
    let entries = value.metadata.iter().collect::<Vec<_>>();
    put_tl_vector(&mut output, &entries, |output, (key, value)| {
        put_tl_string(output, key);
        put_tl_string(output, value);
    });
    output
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    fn take(&mut self, length: usize) -> &'a [u8] {
        let end = self.position + length;
        let result = &self.bytes[self.position..end];
        self.position = end;
        result
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn string(&mut self) -> &'a str {
        let length = self.take(1)[0] as usize;
        let value = std::str::from_utf8(self.take(length)).unwrap();
        while !self.position.is_multiple_of(4) {
            self.position += 1;
        }
        value
    }
    fn vector(&mut self) -> usize {
        assert_eq!(self.u32(), TL_VECTOR);
        self.u32() as usize
    }
}

fn decode_tl_owned(bytes: &[u8]) -> Data {
    let mut cursor = Cursor::new(bytes);
    assert_eq!(cursor.u32(), TL_COLLECTION);
    black_box(cursor.u64());
    let role_count = cursor.vector();
    let roles = (0..role_count)
        .map(|_| cursor.string().to_owned())
        .collect();
    let attachment_count = cursor.vector();
    let attachments = (0..attachment_count)
        .map(|_| {
            assert_eq!(cursor.u32(), TL_ATTACHMENT);
            (
                cursor.string().to_owned(),
                cursor.string().to_owned(),
                cursor.u64(),
            )
        })
        .collect();
    let metadata_count = cursor.vector();
    let metadata = (0..metadata_count)
        .map(|_| (cursor.string().to_owned(), cursor.string().to_owned()))
        .collect();
    Data {
        roles,
        attachments,
        metadata,
    }
}

struct TlView<'a> {
    roles: &'a [u8],
    attachments: &'a [u8],
    metadata: &'a [u8],
}

fn skip_tl_vector<'a>(cursor: &mut Cursor<'a>, mut item: impl FnMut(&mut Cursor<'a>)) -> &'a [u8] {
    let start = cursor.position;
    let count = cursor.vector();
    for _ in 0..count {
        item(cursor);
    }
    &cursor.bytes[start..cursor.position]
}

fn decode_tl_view(bytes: &[u8]) -> TlView<'_> {
    let mut cursor = Cursor::new(bytes);
    assert_eq!(cursor.u32(), TL_COLLECTION);
    black_box(cursor.u64());
    let roles = skip_tl_vector(&mut cursor, |cursor| {
        cursor.string();
    });
    let attachments = skip_tl_vector(&mut cursor, |cursor| {
        assert_eq!(cursor.u32(), TL_ATTACHMENT);
        cursor.string();
        cursor.string();
        cursor.u64();
    });
    let metadata = skip_tl_vector(&mut cursor, |cursor| {
        cursor.string();
        cursor.string();
    });
    TlView {
        roles,
        attachments,
        metadata,
    }
}

fn iterate_tl(view: &TlView<'_>) -> usize {
    let mut roles = Cursor::new(view.roles);
    let mut total = 0;
    for _ in 0..roles.vector() {
        total += roles.string().len();
    }
    let mut attachments = Cursor::new(view.attachments);
    for _ in 0..attachments.vector() {
        assert_eq!(attachments.u32(), TL_ATTACHMENT);
        total +=
            attachments.string().len() + attachments.string().len() + attachments.u64() as usize;
    }
    let mut metadata = Cursor::new(view.metadata);
    for _ in 0..metadata.vector() {
        total += metadata.string().len() + metadata.string().len();
    }
    total
}

fn main() {
    let value = data();
    let typikon_wire = build_typikon(&value);
    let flat_wire = build_flatbuffers(&value);
    let tl_wire = build_tl(&value);
    let typikon_encode = ns_per_iteration(ITERATIONS, || {
        black_box(build_typikon(&value));
    });
    let flat_encode = ns_per_iteration(ITERATIONS, || {
        black_box(build_flatbuffers(&value));
    });
    let tl_encode = ns_per_iteration(ITERATIONS, || {
        black_box(build_tl(&value));
    });
    let typikon_owned = ns_per_iteration(ITERATIONS, || {
        black_box(decode_typikon_owned(&typikon_wire));
    });
    let typikon_view = ns_per_iteration(ITERATIONS, || {
        let view: TypikonView<'_> = typikon::decode_borrowed_value(&typikon_wire).unwrap();
        black_box(iterate_typikon(&view));
    });
    let flat_owned = ns_per_iteration(ITERATIONS, || {
        black_box(decode_flat_owned(&flat_wire));
    });
    let flat_view = ns_per_iteration(ITERATIONS, || {
        black_box(iterate_flat(&flat_wire));
    });
    let tl_owned = ns_per_iteration(ITERATIONS, || {
        black_box(decode_tl_owned(&tl_wire));
    });
    let tl_view = ns_per_iteration(ITERATIONS, || {
        let view = decode_tl_view(&tl_wire);
        black_box(iterate_tl(&view));
    });
    let typikon_owned_allocs = allocation_count(|| {
        black_box(decode_typikon_owned(&typikon_wire));
    });
    let typikon_view_allocs = allocation_count(|| {
        let view: TypikonView<'_> = typikon::decode_borrowed_value(&typikon_wire).unwrap();
        black_box(iterate_typikon(&view));
    });
    let flat_owned_allocs = allocation_count(|| {
        black_box(decode_flat_owned(&flat_wire));
    });
    let flat_view_allocs = allocation_count(|| {
        black_box(iterate_flat(&flat_wire));
    });
    let tl_owned_allocs = allocation_count(|| {
        black_box(decode_tl_owned(&tl_wire));
    });
    let tl_view_allocs = allocation_count(|| {
        let view = decode_tl_view(&tl_wire);
        black_box(iterate_tl(&view));
    });
    println!(
        "format=typikon bytes={} encode_ns={typikon_encode:.2} owned_decode_ns={typikon_owned:.2} borrowed_decode_and_iterate_ns={typikon_view:.2}",
        typikon_wire.len()
    );
    println!(
        "format=typikon allocations_owned={} allocations_borrowed={}",
        typikon_owned_allocs, typikon_view_allocs
    );
    println!(
        "format=flatbuffers bytes={} encode_ns={flat_encode:.2} owned_decode_ns={flat_owned:.2} borrowed_decode_and_iterate_ns={flat_view:.2}",
        flat_wire.len()
    );
    println!(
        "format=flatbuffers allocations_owned={} allocations_borrowed={}",
        flat_owned_allocs, flat_view_allocs
    );
    println!(
        "format=tl_style bytes={} encode_ns={tl_encode:.2} owned_decode_ns={tl_owned:.2} borrowed_decode_and_iterate_ns={tl_view:.2}",
        tl_wire.len()
    );
    println!(
        "format=tl allocations_owned={} allocations_borrowed={}",
        tl_owned_allocs, tl_view_allocs
    );
}
