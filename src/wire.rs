//! Small, allocation-aware wire primitives used by generated codecs.

use std::collections::BTreeMap;

const MAX_VARINT_BYTES: usize = 10;

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
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError>;
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError>;
}

impl Encoder {
    pub fn new(max_size: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_size,
        }
    }
    pub fn finish(self) -> Result<Vec<u8>, WireError> {
        Ok(self.bytes)
    }
    pub fn varint(&mut self, mut value: u64) -> Result<(), WireError> {
        loop {
            let byte = if value >= 0x80 {
                let b = (value as u8) | 0x80;
                value >>= 7;
                b
            } else {
                let byte = value as u8;
                value = 0;
                byte
            };
            self.push(byte)?;
            if value == 0 {
                return Ok(());
            }
        }
    }
    pub fn u8(&mut self, value: u8) -> Result<(), WireError> {
        self.push(value)
    }
    pub fn u16(&mut self, value: u16) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
    pub fn u32(&mut self, value: u32) -> Result<(), WireError> {
        self.extend(&value.to_le_bytes())
    }
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
    pub fn bytes(&mut self, value: &[u8]) -> Result<(), WireError> {
        let len = u64::try_from(value.len()).map_err(|_| WireError::IntegerOverflow)?;
        self.varint(len)?;
        self.extend(value)
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
        if self.bytes.len().saturating_add(bytes.len()) > self.max_size {
            return Err(WireError::PacketTooLarge);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
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
    pub fn varint(&mut self) -> Result<u64, WireError> {
        let mut result = 0u64;
        for index in 0..MAX_VARINT_BYTES {
            let byte = self.read_u8()?;
            let shift = index * 7;
            if index == 9 && (byte & 0x7e) != 0 {
                return Err(WireError::IntegerOverflow);
            }
            result |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
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
        let len = usize::try_from(self.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if len > self.max_size {
            return Err(WireError::PacketTooLarge);
        }
        Ok(self.read_slice(len)?.to_vec())
    }
    pub fn string(&mut self) -> Result<String, WireError> {
        String::from_utf8(self.bytes()?).map_err(|_| WireError::InvalidUtf8)
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
        let actual = self.read_raw(crate::constructor::CID_BYTES)?;
        if actual == crate::constructor::cid_bytes(expected)? {
            Ok(())
        } else {
            Err(WireError::UnknownCId)
        }
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
    ($type:ty, $encode:ident, $decode:ident) => {
        impl WireCodec for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
                encoder.$encode(*self)
            }
            fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
                decoder.$decode()
            }
        }
    };
}

primitive_codec!(u8, u8, u8);
primitive_codec!(u16, u16, u16);
primitive_codec!(u32, u32, u32);
primitive_codec!(u64, u64, u64);
primitive_codec!(i32, i32, i32);
primitive_codec!(u128, u128, u128);
primitive_codec!(i8, i8, i8);
primitive_codec!(i16, i16, i16);
primitive_codec!(i64, i64, i64);
primitive_codec!(i128, i128, i128);
primitive_codec!(f32, f32, f32);
primitive_codec!(f64, f64, f64);
primitive_codec!(bool, bool, bool);

impl WireCodec for String {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        encoder.bytes(self.as_bytes())
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.string()
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
        let count = decoder.varint()? as usize;
        if count > decoder.max_size {
            return Err(WireError::PacketTooLarge);
        }
        let mut result = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            result.push(T::decode(decoder)?);
        }
        Ok(result)
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
        let count = decoder.varint()? as usize;
        if count > decoder.max_size {
            return Err(WireError::PacketTooLarge);
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
    }
}
