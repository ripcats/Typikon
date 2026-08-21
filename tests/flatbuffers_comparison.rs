use std::collections::BTreeMap;

use typikon::{BorrowedWireCodec, Decoder, Encoder, WireError};

#[allow(
    dead_code,
    mismatched_lifetime_syntaxes,
    unused_imports,
    unsafe_op_in_unsafe_fn
)]
mod flatbuffers_generated {
    include!("../benches/generated/collection_generated.rs");
}
use flatbuffers_generated::typikon_bench as fb;

const C: [u8; 8] = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
const A: [u8; 8] = [0x80, 0x70, 0x60, 0x50, 0x40, 0x30, 0x20, 0x10];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Data {
    roles: Vec<String>,
    attachments: Vec<(String, String, u64)>,
    metadata: BTreeMap<String, String>,
    payload: Vec<u8>,
}

fn sample() -> Data {
    Data {
        roles: vec!["admin".into(), "moderator".into(), "support".into()],
        attachments: vec![
            ("photo-0".into(), "image/jpeg".into(), 4096),
            ("photo-1".into(), "image/png".into(), 8192),
        ],
        metadata: BTreeMap::from([
            ("client".into(), "web".into()),
            ("locale".into(), "en".into()),
        ]),
        payload: vec![0xa5; 4096],
    }
}

fn typikon_encode(value: &Data) -> Vec<u8> {
    let mut e = Encoder::with_capacity(typikon::DEFAULT_MAX_PACKET_SIZE, 512);
    e.raw(&C).unwrap();
    e.u64(9).unwrap();
    e.varint(value.roles.len() as u64).unwrap();
    for role in &value.roles {
        e.bytes(role.as_bytes()).unwrap();
    }
    e.varint(value.attachments.len() as u64).unwrap();
    for (name, mime, size) in &value.attachments {
        e.raw(&A).unwrap();
        e.bytes(name.as_bytes()).unwrap();
        e.bytes(mime.as_bytes()).unwrap();
        e.u64(*size).unwrap();
    }
    e.varint(value.metadata.len() as u64).unwrap();
    for (key, value) in &value.metadata {
        e.bytes(key.as_bytes()).unwrap();
        e.bytes(value.as_bytes()).unwrap();
    }
    e.bytes(&value.payload).unwrap();
    e.finish().unwrap()
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

fn typikon_decode(bytes: &[u8]) -> Result<Data, WireError> {
    let mut d = Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE)?;
    d.expect_cid_bytes(&C)?;
    let _: u64 = d.value()?;
    let roles: Vec<String> = d.value()?;
    let count = d.varint()? as usize;
    let attachments = (0..count)
        .map(|_| {
            d.expect_cid_bytes(&A)?;
            Ok((d.string()?, d.string()?, d.u64()?))
        })
        .collect::<Result<Vec<_>, WireError>>()?;
    let count = d.varint()? as usize;
    let mut metadata = BTreeMap::new();
    for _ in 0..count {
        metadata.insert(d.string()?, d.string()?);
    }
    let payload = d.bytes()?;
    if d.remaining() != 0 {
        return Err(WireError::MalformedConstructor);
    }
    Ok(Data {
        roles,
        attachments,
        metadata,
        payload,
    })
}

fn typikon_borrowed_checksum(bytes: &[u8]) -> Result<usize, WireError> {
    let mut d = Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE)?;
    d.expect_cid_bytes(&C)?;
    let id: u64 = d.value()?;
    let roles: typikon::BorrowedVec<'_, &'_ str> = d.borrowed_vec()?;
    let attachments: typikon::BorrowedVec<'_, AttachmentView<'_>> = d.borrowed_vec()?;
    let metadata: typikon::BorrowedMap<'_, &'_ str, &'_ str> = d.borrowed_map()?;
    let mut checksum = id as usize;
    for role in roles.iter() {
        checksum += role?.len();
    }
    for attachment in attachments.iter() {
        let attachment = attachment?;
        checksum += attachment.name.len() + attachment.mime.len() + attachment.size as usize;
    }
    for entry in metadata.iter() {
        let (key, value) = entry?;
        checksum += key.len() + value.len();
    }
    checksum += d.bytes_borrowed()?.len();
    if d.remaining() != 0 {
        return Err(WireError::MalformedConstructor);
    }
    Ok(checksum)
}

fn flatbuffers_encode(value: &Data) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let roles = value
        .roles
        .iter()
        .map(|role| builder.create_string(role))
        .collect::<Vec<_>>();
    let attachments = value
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
    let attachments = builder.create_vector(&attachments);
    let keys = builder.create_vector(&keys);
    let values = builder.create_vector(&values);
    let payload = builder.create_vector(&value.payload);
    let root = fb::CollectionMessage::create(
        &mut builder,
        &fb::CollectionMessageArgs {
            id: 9,
            roles: Some(roles),
            attachments: Some(attachments),
            metadata_keys: Some(keys),
            metadata_values: Some(values),
            payload: Some(payload),
        },
    );
    fb::finish_collection_message_buffer(&mut builder, root);
    builder.finished_data().to_vec()
}

fn flatbuffers_decode(bytes: &[u8]) -> Result<Data, flatbuffers::InvalidFlatbuffer> {
    let root = fb::root_as_collection_message(bytes)?;
    let roles = root.roles().unwrap().iter().map(str::to_owned).collect();
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
    Ok(Data {
        roles,
        attachments,
        metadata,
        payload: root.payload().unwrap_or(&[]).to_vec(),
    })
}

fn flatbuffers_view_checksum(bytes: &[u8]) -> Result<usize, flatbuffers::InvalidFlatbuffer> {
    let root = fb::root_as_collection_message(bytes)?;
    let mut checksum = root.id() as usize;
    for role in root.roles().unwrap().iter() {
        checksum += role.len();
    }
    for attachment in root.attachments().unwrap().iter() {
        checksum += attachment.name().unwrap().len()
            + attachment.mime().unwrap().len()
            + attachment.size_() as usize;
    }
    for index in 0..root.metadata_keys().unwrap().len() {
        checksum += root.metadata_keys().unwrap().get(index).len();
        checksum += root.metadata_values().unwrap().get(index).len();
    }
    checksum += root.payload().unwrap_or(&[]).len();
    Ok(checksum)
}

#[test]
fn typikon_and_flatbuffers_round_trip_the_same_model() {
    let value = sample();
    let typikon_wire = typikon_encode(&value);
    let flatbuffers_wire = flatbuffers_encode(&value);

    assert_eq!(typikon_decode(&typikon_wire).unwrap(), value);
    assert_eq!(flatbuffers_decode(&flatbuffers_wire).unwrap(), value);
    assert_ne!(typikon_wire, flatbuffers_wire);
    assert!(typikon_wire.len() > 0 && flatbuffers_wire.len() > 0);
    assert_eq!(
        typikon_borrowed_checksum(&typikon_wire).unwrap(),
        flatbuffers_view_checksum(&flatbuffers_wire).unwrap()
    );
}

#[test]
fn both_formats_reject_truncated_packets() {
    let value = sample();
    let typikon_wire = typikon_encode(&value);
    let flatbuffers_wire = flatbuffers_encode(&value);
    assert!(typikon_decode(&typikon_wire[..typikon_wire.len() / 2]).is_err());
    assert!(flatbuffers_decode(&flatbuffers_wire[..flatbuffers_wire.len() / 2]).is_err());
}
