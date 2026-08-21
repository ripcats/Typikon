//! Small, allocation-aware wire primitives used by generated codecs.

use std::collections::BTreeMap;

const MAX_VARINT_BYTES: usize = 10;
const DEFAULT_ENCODER_CAPACITY: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    UnexpectedEof,
    InvalidVarInt,
    IntegerOverflow,
    PacketTooLarge,
    InvalidUtf8,
    MalformedConstructor,
    InvalidCId,
    UnknownCId,
    InvalidEnum,
}

pub struct Encoder {
    bytes: Vec<u8>,
    max_size: usize,
}

pub trait WireCodec: Sized {
    const FIXED_ENCODED_LEN: Option<usize> = None;

    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError>;
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError>;
    fn encoded_len(&self) -> usize {
        0
    }
}

pub const fn varint_len(value: u64) -> usize {
    let bits = u64::BITS - value.leading_zeros();
    if bits == 0 {
        1
    } else {
        bits.div_ceil(7) as usize
    }
}

impl Encoder {
    pub fn new(max_size: usize) -> Self {
        Self::with_capacity(max_size, DEFAULT_ENCODER_CAPACITY)
    }
    pub fn with_capacity(max_size: usize, capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity.min(max_size)),
            max_size,
        }
    }
    pub fn with_buffer(max_size: usize, mut bytes: Vec<u8>) -> Result<Self, WireError> {
        if bytes.len() > max_size {
            return Err(WireError::PacketTooLarge);
        }
        bytes.clear();
        Ok(Self { bytes, max_size })
    }
    pub fn finish(self) -> Result<Vec<u8>, WireError> {
        Ok(self.bytes)
    }
    #[inline]
    pub fn varint(&mut self, value: u64) -> Result<(), WireError> {
        if value < 0x80 {
            return self.push(value as u8);
        }
        let mut encoded = [0u8; MAX_VARINT_BYTES];
        let len = encode_varint(value, &mut encoded);
        self.extend(&encoded[..len])
    }
    #[inline]
    pub fn u8(&mut self, value: u8) -> Result<(), WireError> {
        self.push(value)
    }
    #[inline]
    pub fn u16(&mut self, value: u16) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    #[inline]
    pub fn u32(&mut self, value: u32) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    #[inline]
    pub fn u64(&mut self, value: u64) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    pub fn i32(&mut self, value: i32) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    pub fn bool(&mut self, value: bool) -> Result<(), WireError> {
        self.u8(value as u8)
    }
    pub fn u128(&mut self, value: u128) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    pub fn i8(&mut self, value: i8) -> Result<(), WireError> {
        self.u8(value as u8)
    }
    pub fn i16(&mut self, value: i16) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    pub fn i64(&mut self, value: i64) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    pub fn i128(&mut self, value: i128) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    pub fn f32(&mut self, value: f32) -> Result<(), WireError> {
        self.u32(value.to_bits())
    }
    pub fn f64(&mut self, value: f64) -> Result<(), WireError> {
        self.u64(value.to_bits())
    }
    #[inline]
    pub fn bytes(&mut self, value: &[u8]) -> Result<(), WireError> {
        let len = u64::try_from(value.len()).map_err(|_| WireError::IntegerOverflow)?;
        let mut encoded_len = [0u8; MAX_VARINT_BYTES];
        let prefix_len = encode_varint(len, &mut encoded_len);
        let total = prefix_len
            .checked_add(value.len())
            .ok_or(WireError::IntegerOverflow)?;
        self.ensure(total)?;
        self.bytes.extend_from_slice(&encoded_len[..prefix_len]);
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    pub fn value<T: WireCodec>(&mut self, value: &T) -> Result<(), WireError> {
        value.encode(self)
    }
    pub fn raw(&mut self, value: &[u8]) -> Result<(), WireError> {
        self.extend(value)
    }
    fn push(&mut self, byte: u8) -> Result<(), WireError> {
        if self.bytes.len() >= self.max_size {
            return Err(WireError::PacketTooLarge);
        }
        self.bytes.push(byte);
        Ok(())
    }
    fn extend(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        self.ensure(bytes.len())?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn ensure(&self, additional: usize) -> Result<(), WireError> {
        match self.bytes.len().checked_add(additional) {
            Some(end) if end <= self.max_size => Ok(()),
            Some(_) => Err(WireError::PacketTooLarge),
            None => Err(WireError::IntegerOverflow),
        }
    }
}

#[inline]
fn encode_varint(mut value: u64, output: &mut [u8; MAX_VARINT_BYTES]) -> usize {
    let mut len = 0;
    while value >= 0x80 {
        output[len] = (value as u8) | 0x80;
        value >>= 7;
        len += 1;
    }
    output[len] = value as u8;
    len + 1
}

pub struct Decoder<'a> {
    bytes: &'a [u8],
    position: usize,
    max_size: usize,
}

impl<'a> Decoder<'a> {
    pub fn new(bytes: &'a [u8], max_size: usize) -> Result<Self, WireError> {
        if bytes.len() > max_size {
            Err(WireError::PacketTooLarge)
        } else {
            Ok(Self {
                bytes,
                position: 0,
                max_size,
            })
        }
    }
    pub fn is_finished(&self) -> bool {
        self.position == self.bytes.len()
    }
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
    #[inline]
    pub fn varint(&mut self) -> Result<u64, WireError> {
        let first = self.read_u8()?;
        if first < 0x80 {
            return Ok(u64::from(first));
        }
        let mut result = u64::from(first & 0x7f);
        for index in 1..MAX_VARINT_BYTES {
            let byte = self.read_u8()?;
            let shift = index * 7;
            if index == 9 && (byte & 0x7e) != 0 {
                return Err(WireError::IntegerOverflow);
            }
            result |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                if byte == 0 {
                    return Err(WireError::InvalidVarInt);
                }
                return Ok(result);
            }
        }
        Err(WireError::InvalidVarInt)
    }
    pub fn u8(&mut self) -> Result<u8, WireError> {
        self.read_u8()
    }
    pub fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }
    pub fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }
    pub fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }
    pub fn i32(&mut self) -> Result<i32, WireError> {
        Ok(i32::from_le_bytes(self.read_array()?))
    }
    pub fn bool(&mut self) -> Result<bool, WireError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(WireError::MalformedConstructor),
        }
    }
    pub fn u128(&mut self) -> Result<u128, WireError> {
        Ok(u128::from_le_bytes(self.read_array()?))
    }
    pub fn i8(&mut self) -> Result<i8, WireError> {
        Ok(self.u8()? as i8)
    }
    pub fn i16(&mut self) -> Result<i16, WireError> {
        Ok(i16::from_le_bytes(self.read_array()?))
    }
    pub fn i64(&mut self) -> Result<i64, WireError> {
        Ok(i64::from_le_bytes(self.read_array()?))
    }
    pub fn i128(&mut self) -> Result<i128, WireError> {
        Ok(i128::from_le_bytes(self.read_array()?))
    }
    pub fn f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn f64(&mut self) -> Result<f64, WireError> {
        Ok(f64::from_bits(self.u64()?))
    }
    pub fn bytes(&mut self) -> Result<Vec<u8>, WireError> {
        Ok(self.bytes_borrowed()?.to_vec())
    }
    pub fn bytes_borrowed(&mut self) -> Result<&'a [u8], WireError> {
        let len = usize::try_from(self.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if len > self.max_size {
            return Err(WireError::PacketTooLarge);
        }
        self.read_slice(len)
    }
    pub fn string(&mut self) -> Result<String, WireError> {
        Ok(self.string_borrowed()?.to_owned())
    }
    pub fn string_borrowed(&mut self) -> Result<&'a str, WireError> {
        std::str::from_utf8(self.bytes_borrowed()?).map_err(|_| WireError::InvalidUtf8)
    }
    pub fn value<T: WireCodec>(&mut self) -> Result<T, WireError> {
        T::decode(self)
    }
    fn read_u8(&mut self) -> Result<u8, WireError> {
        let value = *self
            .bytes
            .get(self.position)
            .ok_or(WireError::UnexpectedEof)?;
        self.position += 1;
        Ok(value)
    }
    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        self.read_slice(N)?
            .try_into()
            .map_err(|_| WireError::UnexpectedEof)
    }
    fn read_slice(&mut self, len: usize) -> Result<&'a [u8], WireError> {
        let end = self
            .position
            .checked_add(len)
            .ok_or(WireError::IntegerOverflow)?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or(WireError::UnexpectedEof)?;
        self.position = end;
        Ok(slice)
    }
    pub(crate) fn read_raw(&mut self, len: usize) -> Result<&'a [u8], WireError> {
        self.read_slice(len)
    }
    pub(crate) fn peek_raw(&self, len: usize) -> Result<&'a [u8], WireError> {
        self.bytes.get(..len).ok_or(WireError::UnexpectedEof)
    }
    pub fn expect_cid(&mut self, expected: &str) -> Result<(), WireError> {
        let expected = crate::constructor::cid_bytes(expected)?;
        self.expect_cid_bytes(&expected)
    }
    pub fn expect_cid_bytes(
        &mut self,
        expected: &[u8; crate::constructor::CID_BYTES],
    ) -> Result<(), WireError> {
        let actual = self.read_raw(crate::constructor::CID_BYTES)?;
        if actual == expected {
            Ok(())
        } else {
            Err(WireError::UnknownCId)
        }
    }
    pub fn read_cid_bytes(&mut self) -> Result<[u8; crate::constructor::CID_BYTES], WireError> {
        self.read_array()
    }
    pub fn read_cid(&mut self) -> Result<String, WireError> {
        Ok(self
            .read_raw(crate::constructor::CID_BYTES)?
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

macro_rules! primitive_codec {
    ($type:ty, $encode:ident, $decode:ident, $size:expr) => {
        impl WireCodec for $type {
            const FIXED_ENCODED_LEN: Option<usize> = Some($size);

            fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
                encoder.$encode(*self)
            }
            fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
                decoder.$decode()
            }
            fn encoded_len(&self) -> usize {
                $size
            }
        }
    };
}

primitive_codec!(u8, u8, u8, 1);
primitive_codec!(u16, u16, u16, 2);
primitive_codec!(u32, u32, u32, 4);
primitive_codec!(u64, u64, u64, 8);
primitive_codec!(i32, i32, i32, 4);
primitive_codec!(u128, u128, u128, 16);
primitive_codec!(i8, i8, i8, 1);
primitive_codec!(i16, i16, i16, 2);
primitive_codec!(i64, i64, i64, 8);
primitive_codec!(i128, i128, i128, 16);
primitive_codec!(f32, f32, f32, 4);
primitive_codec!(f64, f64, f64, 8);
primitive_codec!(bool, bool, bool, 1);

impl WireCodec for String {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        encoder.bytes(self.as_bytes())
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.string()
    }
    fn encoded_len(&self) -> usize {
        varint_len(self.len() as u64).saturating_add(self.len())
    }
}

impl<T: WireCodec> WireCodec for Vec<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        encoder.varint(self.len() as u64)?;
        for item in self {
            item.encode(encoder)?;
        }
        Ok(())
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = usize::try_from(decoder.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if count > decoder.max_size {
            return Err(WireError::PacketTooLarge);
        }
        let capacity = match T::FIXED_ENCODED_LEN {
            Some(item_size) => {
                let required = count
                    .checked_mul(item_size)
                    .ok_or(WireError::IntegerOverflow)?;
                if required > decoder.remaining() {
                    return Err(WireError::UnexpectedEof);
                }
                count
            }
            None => count.min(1024),
        };
        let mut result = Vec::with_capacity(capacity);
        for _ in 0..count {
            result.push(T::decode(decoder)?);
        }
        Ok(result)
    }
    fn encoded_len(&self) -> usize {
        let prefix = varint_len(self.len() as u64);
        match T::FIXED_ENCODED_LEN {
            Some(item_size) => prefix.saturating_add(self.len().saturating_mul(item_size)),
            None => self
                .iter()
                .fold(prefix, |size, item| size.saturating_add(item.encoded_len())),
        }
    }
}

impl<K: WireCodec + Ord, V: WireCodec> WireCodec for BTreeMap<K, V> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        encoder.varint(self.len() as u64)?;
        for (key, value) in self {
            key.encode(encoder)?;
            value.encode(encoder)?;
        }
        Ok(())
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = usize::try_from(decoder.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if count > decoder.max_size {
            return Err(WireError::PacketTooLarge);
        }
        if let (Some(key_size), Some(value_size)) = (K::FIXED_ENCODED_LEN, V::FIXED_ENCODED_LEN) {
            let pair_size = key_size
                .checked_add(value_size)
                .ok_or(WireError::IntegerOverflow)?;
            let required = count
                .checked_mul(pair_size)
                .ok_or(WireError::IntegerOverflow)?;
            if required > decoder.remaining() {
                return Err(WireError::UnexpectedEof);
            }
        }
        let mut result = BTreeMap::new();
        for _ in 0..count {
            let key = K::decode(decoder)?;
            let value = V::decode(decoder)?;
            if result.insert(key, value).is_some() {
                return Err(WireError::MalformedConstructor);
            }
        }
        Ok(result)
    }
    fn encoded_len(&self) -> usize {
        let prefix = varint_len(self.len() as u64);
        match (K::FIXED_ENCODED_LEN, V::FIXED_ENCODED_LEN) {
            (Some(key_size), Some(value_size)) => prefix.saturating_add(
                self.len()
                    .saturating_mul(key_size.saturating_add(value_size)),
            ),
            _ => self.iter().fold(prefix, |size, (key, value)| {
                size.saturating_add(key.encoded_len())
                    .saturating_add(value.encoded_len())
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    const LIMIT: usize = 1024;

    #[test]
    fn varint_golden_vectors() {
        for (value, expected) in [
            (0, vec![0]),
            (1, vec![1]),
            (127, vec![127]),
            (128, vec![128, 1]),
            (300, vec![172, 2]),
            (
                u64::MAX,
                vec![255, 255, 255, 255, 255, 255, 255, 255, 255, 1],
            ),
        ] {
            let mut encoder = Encoder::new(LIMIT);
            encoder.varint(value).unwrap();
            assert_eq!(encoder.finish().unwrap(), expected);
        }
    }

    #[test]
    fn varint_round_trips_deterministic_fuzz_corpus() {
        let mut state = 0x9e3779b97f4a7c15u64;
        for _ in 0..10_000 {
            state ^= state << 7;
            state ^= state >> 9;
            state ^= state << 8;
            let mut encoder = Encoder::new(LIMIT);
            encoder.varint(state).unwrap();
            let bytes = encoder.finish().unwrap();
            let mut decoder = Decoder::new(&bytes, LIMIT).unwrap();
            assert_eq!(decoder.varint().unwrap(), state);
            assert!(decoder.is_finished());
        }
    }

    #[test]
    fn rejects_non_canonical_varints_without_partial_writes() {
        for bytes in [&[0x80, 0x00][..], &[0x81, 0x00], &[0xff, 0x00]] {
            let mut decoder = Decoder::new(bytes, LIMIT).unwrap();
            assert_eq!(decoder.varint(), Err(WireError::InvalidVarInt));
        }

        let mut encoder = Encoder::new(1);
        assert_eq!(encoder.varint(128), Err(WireError::PacketTooLarge));
        encoder.u8(7).unwrap();
        assert_eq!(encoder.finish().unwrap(), [7]);

        let mut encoder = Encoder::new(1);
        assert_eq!(encoder.bytes(b"x"), Err(WireError::PacketTooLarge));
        encoder.u8(9).unwrap();
        assert_eq!(encoder.finish().unwrap(), [9]);
    }

    #[test]
    fn random_wire_inputs_never_panic() {
        let mut state = 0x123456789abcdef0u64;
        for length in 0..256 {
            let mut bytes = vec![0; length];
            for byte in &mut bytes {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (state >> 56) as u8;
            }
            let result = std::panic::catch_unwind(|| {
                if let Ok(mut decoder) = Decoder::new(&bytes, LIMIT) {
                    let _ = decoder.varint();
                    let _ = decoder.bytes();
                    let _ = Vec::<u16>::decode(&mut decoder);
                    let _ = BTreeMap::<u8, u8>::decode(&mut decoder);
                }
            });
            assert!(result.is_ok(), "wire decoder panicked for {length} bytes");
        }
    }

    #[test]
    fn primitive_round_trip() {
        let mut encoder = Encoder::new(LIMIT);
        encoder.u8(7).unwrap();
        encoder.u16(0x1234).unwrap();
        encoder.u32(0x12345678).unwrap();
        encoder.u64(0x0123456789abcdef).unwrap();
        encoder.i32(-42).unwrap();
        encoder.bytes(b"hello").unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(&bytes[0..7], &[7, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12]);
        let mut decoder = Decoder::new(&bytes, LIMIT).unwrap();
        assert_eq!(decoder.u8().unwrap(), 7);
        assert_eq!(decoder.u16().unwrap(), 0x1234);
        assert_eq!(decoder.u32().unwrap(), 0x12345678);
        assert_eq!(decoder.u64().unwrap(), 0x0123456789abcdef);
        assert_eq!(decoder.i32().unwrap(), -42);
        assert_eq!(decoder.bytes().unwrap(), b"hello");
        assert!(decoder.is_finished());
    }

    #[test]
    fn borrowed_bytes_and_strings_share_input_storage() {
        let mut encoder = Encoder::new(LIMIT);
        encoder.bytes(b"payload").unwrap();
        encoder.bytes(b"text").unwrap();
        let bytes = encoder.finish().unwrap();
        let mut decoder = Decoder::new(&bytes, LIMIT).unwrap();
        let payload = decoder.bytes_borrowed().unwrap();
        let text = decoder.string_borrowed().unwrap();
        assert_eq!(payload, b"payload");
        assert_eq!(text, "text");
        assert!(bytes[payload.as_ptr() as usize - bytes.as_ptr() as usize..].starts_with(payload));
        assert!(decoder.is_finished());
    }

    #[test]
    fn encoder_can_reuse_a_finished_buffer() {
        let mut encoder = Encoder::with_capacity(LIMIT, 32);
        encoder.bytes(b"first").unwrap();
        let buffer = encoder.finish().unwrap();
        let capacity = buffer.capacity();
        let mut encoder = Encoder::with_buffer(LIMIT, buffer).unwrap();
        encoder.bytes(b"second").unwrap();
        let buffer = encoder.finish().unwrap();
        assert_eq!(buffer, [6, b's', b'e', b'c', b'o', b'n', b'd']);
        assert_eq!(buffer.capacity(), capacity);
    }

    #[test]
    fn rejects_bad_input_and_limits() {
        assert!(matches!(
            Decoder::new(&[1, 2], 1),
            Err(WireError::PacketTooLarge)
        ));
        assert_eq!(
            Decoder::new(&[], LIMIT).unwrap().u64(),
            Err(WireError::UnexpectedEof)
        );
        assert_eq!(
            Decoder::new(&[0x80; 10], LIMIT).unwrap().varint(),
            Err(WireError::InvalidVarInt)
        );
        let mut encoder = Encoder::new(1);
        encoder.u16(1).unwrap_err();
        let mut encoder = Encoder::new(LIMIT);
        encoder.bytes(b"abc").unwrap();
        let encoded = encoder.finish().unwrap();
        let mut decoder = Decoder::new(&encoded, LIMIT).unwrap();
        assert_eq!(decoder.bytes().unwrap(), b"abc");
    }

    #[test]
    fn generic_string_vec_and_map_round_trip() {
        let mut map = BTreeMap::new();
        map.insert(2u16, "two".to_owned());
        map.insert(1u16, "one".to_owned());
        let value = ("привет".to_owned(), vec![1u32, 2, 300], map);
        let mut encoder = Encoder::new(LIMIT);
        value.0.encode(&mut encoder).unwrap();
        value.1.encode(&mut encoder).unwrap();
        value.2.encode(&mut encoder).unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(
            bytes.len(),
            value
                .0
                .encoded_len()
                .saturating_add(value.1.encoded_len())
                .saturating_add(value.2.encoded_len())
        );
        assert_eq!(&bytes[0..2], &[12, 208]);
        let mut decoder = Decoder::new(&bytes, LIMIT).unwrap();
        assert_eq!(String::decode(&mut decoder).unwrap(), value.0);
        assert_eq!(Vec::<u32>::decode(&mut decoder).unwrap(), value.1);
        assert_eq!(
            BTreeMap::<u16, String>::decode(&mut decoder).unwrap(),
            value.2
        );
        assert!(decoder.is_finished());
    }

    #[test]
    fn fixed_width_collection_lengths_are_constant_time_metadata() {
        let values = vec![1u64, 2, 3, 4];
        assert_eq!(u64::FIXED_ENCODED_LEN, Some(8));
        assert_eq!(values.encoded_len(), 1 + 4 * 8);

        let map = BTreeMap::from([(1u16, 10u32), (2u16, 20u32)]);
        assert_eq!(map.encoded_len(), 1 + 2 * (2 + 4));
    }

    #[test]
    fn map_is_sorted_and_duplicate_keys_are_rejected() {
        let mut map = BTreeMap::new();
        map.insert(9u8, 9u8);
        map.insert(1u8, 1u8);
        let mut encoder = Encoder::new(LIMIT);
        map.encode(&mut encoder).unwrap();
        assert_eq!(encoder.finish().unwrap(), vec![2, 1, 1, 9, 9]);

        let duplicate = [2, 1, 7, 1, 8];
        let mut decoder = Decoder::new(&duplicate, LIMIT).unwrap();
        assert_eq!(
            BTreeMap::<u8, u8>::decode(&mut decoder),
            Err(WireError::MalformedConstructor)
        );
    }

    #[test]
    fn invalid_utf8_and_truncated_collections_are_rejected() {
        let mut decoder = Decoder::new(&[2, 0xff, 0xff], LIMIT).unwrap();
        assert_eq!(String::decode(&mut decoder), Err(WireError::InvalidUtf8));
        let mut decoder = Decoder::new(&[3, 1, 2], LIMIT).unwrap();
        assert_eq!(
            Vec::<u8>::decode(&mut decoder),
            Err(WireError::UnexpectedEof)
        );
        let mut decoder = Decoder::new(&[100], LIMIT).unwrap();
        assert_eq!(
            Vec::<u64>::decode(&mut decoder),
            Err(WireError::UnexpectedEof)
        );
        let mut decoder = Decoder::new(&[10], LIMIT).unwrap();
        assert_eq!(
            BTreeMap::<u64, u64>::decode(&mut decoder),
            Err(WireError::UnexpectedEof)
        );
    }
}
