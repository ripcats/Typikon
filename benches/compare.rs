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

const N: usize = 100_000;
const HEAVY_N: usize = 1_000;
const C: [u8; 8] = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
const A: [u8; 8] = [0x80, 0x70, 0x60, 0x50, 0x40, 0x30, 0x20, 0x10];

struct Counter;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static GLOBAL: Counter = Counter;

struct Data {
    roles: Vec<String>,
    attachments: Vec<(String, String, u64)>,
    metadata: BTreeMap<String, String>,
}
struct AttachmentView<'a> {
    name: &'a str,
    mime: &'a str,
    size: u64,
}
impl<'a> BorrowedWireCodec<'a> for AttachmentView<'a> {
    fn decode_borrowed(d: &mut Decoder<'a>) -> Result<Self, WireError> {
        d.expect_cid_bytes(&A)?;
        Ok(Self {
            name: d.string_borrowed()?,
            mime: d.string_borrowed()?,
            size: d.value()?,
        })
    }

    fn skip_borrowed(d: &mut Decoder<'a>) -> Result<(), WireError> {
        d.expect_cid_bytes(&A)?;
        d.skip_string()?;
        d.skip_string()?;
        d.u64()?;
        Ok(())
    }
}

fn data() -> Data {
    Data {
        roles: ["admin", "moderator", "support"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        attachments: (0..8)
            .map(|i| (format!("photo-{i}"), "image/jpeg".into(), 4096 + i))
            .collect(),
        metadata: BTreeMap::from([
            ("client".into(), "web".into()),
            ("locale".into(), "en".into()),
            ("trace".into(), "benchmark".into()),
        ]),
    }
}
fn heavy_data() -> Data {
    Data {
        roles: (0..64)
            .map(|i| format!("role-{i:03}-messenger-member"))
            .collect(),
        attachments: (0..256)
            .map(|i| {
                (
                    format!("attachment-{i:04}-large-photo-name"),
                    "image/jpeg; charset=binary".into(),
                    1_048_576 + i,
                )
            })
            .collect(),
        metadata: (0..64)
            .map(|i| {
                (
                    format!("metadata-key-{i:03}"),
                    format!("metadata-value-{i:03}-production"),
                )
            })
            .collect(),
    }
}
fn timed(mut f: impl FnMut()) -> f64 {
    timed_n(N, f)
}
fn timed_n(iterations: usize, mut f: impl FnMut()) -> f64 {
    let mut remaining = iterations;
    let start = Instant::now();
    while remaining > 0 {
        f();
        remaining -= 1;
    }
    start.elapsed().as_nanos() as f64 / iterations as f64
}
fn allocs(f: impl FnOnce()) -> usize {
    ALLOCS.store(0, Ordering::Relaxed);
    f();
    ALLOCS.load(Ordering::Relaxed)
}

fn typikon_encode(v: &Data) -> Vec<u8> {
    let mut e = Encoder::with_capacity(typikon::DEFAULT_MAX_PACKET_SIZE, 512);
    e.raw(&C).unwrap();
    e.u64(9).unwrap();
    e.varint(v.roles.len() as u64).unwrap();
    for x in &v.roles {
        e.bytes(x.as_bytes()).unwrap();
    }
    e.varint(v.attachments.len() as u64).unwrap();
    for (name, mime, size) in &v.attachments {
        e.raw(&A).unwrap();
        e.bytes(name.as_bytes()).unwrap();
        e.bytes(mime.as_bytes()).unwrap();
        e.u64(*size).unwrap();
    }
    e.varint(v.metadata.len() as u64).unwrap();
    for (k, v) in &v.metadata {
        e.bytes(k.as_bytes()).unwrap();
        e.bytes(v.as_bytes()).unwrap();
    }
    e.finish().unwrap()
}
fn typikon_owned(bytes: &[u8]) -> Data {
    let mut d = Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE).unwrap();
    d.expect_cid_bytes(&C).unwrap();
    let _: u64 = d.value().unwrap();
    let roles = d.value().unwrap();
    let n = d.varint().unwrap() as usize;
    let attachments = (0..n)
        .map(|_| {
            d.expect_cid_bytes(&A).unwrap();
            (d.string().unwrap(), d.string().unwrap(), d.u64().unwrap())
        })
        .collect();
    let n = d.varint().unwrap() as usize;
    let mut metadata = BTreeMap::new();
    for _ in 0..n {
        metadata.insert(d.string().unwrap(), d.string().unwrap());
    }
    Data {
        roles,
        attachments,
        metadata,
    }
}
fn typikon_view(bytes: &[u8]) -> usize {
    let mut d = Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE).unwrap();
    d.expect_cid_bytes(&C).unwrap();
    let _: u64 = d.value().unwrap();
    let roles: typikon::BorrowedVec<'_, &'_ str> = d.borrowed_vec().unwrap();
    let attachments: typikon::BorrowedVec<'_, AttachmentView<'_>> = d.borrowed_vec().unwrap();
    let metadata: typikon::BorrowedMap<'_, &'_ str, &'_ str> = d.borrowed_map().unwrap();
    let mut total = 0;
    for x in roles.iter() {
        total += x.unwrap().len();
    }
    for x in attachments.iter() {
        let x = x.unwrap();
        total += x.name.len() + x.mime.len() + x.size as usize;
    }
    for x in metadata.iter() {
        let (k, v) = x.unwrap();
        total += k.len() + v.len();
    }
    total
}

fn flat_encode(v: &Data) -> Vec<u8> {
    let mut b = flatbuffers::FlatBufferBuilder::new();
    let roles = v
        .roles
        .iter()
        .map(|x| b.create_string(x))
        .collect::<Vec<_>>();
    let attachments = v
        .attachments
        .iter()
        .map(|(n, m, s)| {
            let n = b.create_string(n);
            let m = b.create_string(m);
            fb::Attachment::create(
                &mut b,
                &fb::AttachmentArgs {
                    name: Some(n),
                    mime: Some(m),
                    size_: *s,
                },
            )
        })
        .collect::<Vec<_>>();
    let keys = v
        .metadata
        .keys()
        .map(|x| b.create_string(x))
        .collect::<Vec<_>>();
    let values = v
        .metadata
        .values()
        .map(|x| b.create_string(x))
        .collect::<Vec<_>>();
    let roles = b.create_vector(&roles);
    let attachments = b.create_vector(&attachments);
    let keys = b.create_vector(&keys);
    let values = b.create_vector(&values);
    let root = fb::CollectionMessage::create(
        &mut b,
        &fb::CollectionMessageArgs {
            id: 9,
            roles: Some(roles),
            attachments: Some(attachments),
            metadata_keys: Some(keys),
            metadata_values: Some(values),
        },
    );
    fb::finish_collection_message_buffer(&mut b, root);
    b.finished_data().to_vec()
}
fn flat_owned(bytes: &[u8]) -> Data {
    let r = fb::root_as_collection_message(bytes).unwrap();
    let roles = r.roles().unwrap().iter().map(str::to_owned).collect();
    let attachments = r
        .attachments()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x.name().unwrap().to_owned(),
                x.mime().unwrap().to_owned(),
                x.size_(),
            )
        })
        .collect();
    let keys = r.metadata_keys().unwrap();
    let values = r.metadata_values().unwrap();
    let metadata = (0..keys.len())
        .map(|i| (keys.get(i).to_owned(), values.get(i).to_owned()))
        .collect();
    Data {
        roles,
        attachments,
        metadata,
    }
}
fn flat_view(bytes: &[u8]) -> usize {
    flat_sum(fb::root_as_collection_message(bytes).unwrap())
}
fn flat_view_unchecked(bytes: &[u8]) -> usize {
    flat_sum(unsafe { fb::root_as_collection_message_unchecked(bytes) })
}
fn flat_sum(r: fb::CollectionMessage<'_>) -> usize {
    let mut total = r.id() as usize;
    for x in r.roles().unwrap().iter() {
        total += x.len();
    }
    for x in r.attachments().unwrap().iter() {
        total += x.name().unwrap().len() + x.mime().unwrap().len() + x.size_() as usize;
    }
    for i in 0..r.metadata_keys().unwrap().len() {
        total +=
            r.metadata_keys().unwrap().get(i).len() + r.metadata_values().unwrap().get(i).len();
    }
    total
}

fn main() {
    let value = data();
    let tw = typikon_encode(&value);
    let fw = flat_encode(&value);
    let te = timed(|| {
        black_box(typikon_encode(&value));
    });
    let fe = timed(|| {
        black_box(flat_encode(&value));
    });
    let to = timed(|| {
        black_box(typikon_owned(&tw));
    });
    let fo = timed(|| {
        black_box(flat_owned(&fw));
    });
    let tv = timed(|| {
        black_box(typikon_view(&tw));
    });
    let fv = timed(|| {
        black_box(flat_view(&fw));
    });
    let fvu = timed(|| {
        black_box(flat_view_unchecked(&fw));
    });
    let toa = allocs(|| {
        black_box(typikon_owned(&tw));
    });
    let foa = allocs(|| {
        black_box(flat_owned(&fw));
    });
    let tva = allocs(|| {
        black_box(typikon_view(&tw));
    });
    let fva = allocs(|| {
        black_box(flat_view(&fw));
    });
    println!(
        "format=typikon bytes={} encode_ns={te:.2} owned_decode_ns={to:.2} borrowed_decode_and_iterate_ns={tv:.2} allocations_owned={toa} allocations_borrowed={tva}",
        tw.len()
    );
    println!(
        "format=flatbuffers bytes={} encode_ns={fe:.2} owned_decode_ns={fo:.2} verified_view_ns={fv:.2} unchecked_view_ns={fvu:.2} allocations_owned={foa} allocations_borrowed={fva}",
        fw.len()
    );

    let heavy = heavy_data();
    let htw = typikon_encode(&heavy);
    let hfw = flat_encode(&heavy);
    let hte = timed_n(HEAVY_N, || {
        black_box(typikon_encode(&heavy));
    });
    let hfe = timed_n(HEAVY_N, || {
        black_box(flat_encode(&heavy));
    });
    let hto = timed_n(HEAVY_N, || {
        black_box(typikon_owned(&htw));
    });
    let hfo = timed_n(HEAVY_N, || {
        black_box(flat_owned(&hfw));
    });
    let htv = timed_n(HEAVY_N, || {
        black_box(typikon_view(&htw));
    });
    let hfv = timed_n(HEAVY_N, || {
        black_box(flat_view(&hfw));
    });
    let hfvu = timed_n(HEAVY_N, || {
        black_box(flat_view_unchecked(&hfw));
    });
    let htoa = allocs(|| {
        black_box(typikon_owned(&htw));
    });
    let hfoa = allocs(|| {
        black_box(flat_owned(&hfw));
    });
    let htva = allocs(|| {
        black_box(typikon_view(&htw));
    });
    let hfva = allocs(|| {
        black_box(flat_view(&hfw));
    });
    println!(
        "case=heavy format=typikon entries=64_roles+256_attachments+64_metadata bytes={} encode_ns={hte:.2} owned_decode_ns={hto:.2} borrowed_decode_and_iterate_ns={htv:.2} allocations_owned={htoa} allocations_borrowed={htva}",
        htw.len()
    );
    println!(
        "case=heavy format=flatbuffers entries=64_roles+256_attachments+64_metadata bytes={} encode_ns={hfe:.2} owned_decode_ns={hfo:.2} verified_view_ns={hfv:.2} unchecked_view_ns={hfvu:.2} allocations_owned={hfoa} allocations_borrowed={hfva}",
        hfw.len()
    );
}
