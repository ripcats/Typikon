//! Small, allocation-aware wire primitives used by generated codecs.

use std::collections::BTreeMap;
use std::io::{self, IoSlice, Write};
use std::marker::PhantomData;

use crate::limits::DecodeLimits;

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

impl<const N: usize> WireCodec for [u8; N] {
    const FIXED_ENCODED_LEN: Option<usize> = Some(N);

    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        encoder.fixed_bytes(self)
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.fixed_bytes()
    }

    fn encoded_len(&self) -> usize {
        N
    }
}

pub trait BorrowedWireCodec<'a>: Sized {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError>;

    fn skip_borrowed(decoder: &mut Decoder<'a>) -> Result<(), WireError> {
        Self::decode_borrowed(decoder).map(|_| ())
    }
}

impl<'a> BorrowedWireCodec<'a> for &'a str {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        decoder.string_borrowed()
    }

    fn skip_borrowed(decoder: &mut Decoder<'a>) -> Result<(), WireError> {
        decoder.skip_string()
    }
}

impl<'a> BorrowedWireCodec<'a> for &'a [u8] {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        decoder.bytes_borrowed()
    }

    fn skip_borrowed(decoder: &mut Decoder<'a>) -> Result<(), WireError> {
        decoder.skip_bytes()
    }
}

impl<'a, T: WireCodec> BorrowedWireCodec<'a> for T {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        T::decode(decoder)
    }
}

pub struct BorrowedVec<'a, T> {
    bytes: &'a [u8],
    count: usize,
    max_size: usize,
    marker: PhantomData<T>,
}

impl<'a, T> std::fmt::Debug for BorrowedVec<'a, T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BorrowedVec")
            .field("count", &self.count)
            .finish()
    }
}

impl<'a, T> Clone for BorrowedVec<'a, T> {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes,
            count: self.count,
            max_size: self.max_size,
            marker: PhantomData,
        }
    }
}

impl<'a, T> PartialEq for BorrowedVec<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes && self.count == other.count
    }
}

impl<'a, T> BorrowedVec<'a, T> {
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl<'a, T: BorrowedWireCodec<'a>> BorrowedVec<'a, T> {
    pub fn iter(&self) -> BorrowedVecIter<'a, T> {
        BorrowedVecIter {
            decoder: Decoder::new(self.bytes, self.max_size).expect("validated view range"),
            remaining: self.count,
            marker: PhantomData,
        }
    }
}

impl<'a, T: BorrowedWireCodec<'a>> BorrowedWireCodec<'a> for BorrowedVec<'a, T> {
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        decoder.borrowed_vec()
    }

    fn skip_borrowed(decoder: &mut Decoder<'a>) -> Result<(), WireError> {
        decoder.skip_vec::<T>()
    }
}

pub struct BorrowedVecIter<'a, T> {
    decoder: Decoder<'a>,
    remaining: usize,
    marker: PhantomData<T>,
}

impl<'a, T: BorrowedWireCodec<'a>> Iterator for BorrowedVecIter<'a, T> {
    type Item = Result<T, WireError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some(T::decode_borrowed(&mut self.decoder))
    }
}

pub struct BorrowedMap<'a, K, V> {
    bytes: &'a [u8],
    count: usize,
    max_size: usize,
    marker: PhantomData<(K, V)>,
}

impl<'a, K, V> std::fmt::Debug for BorrowedMap<'a, K, V> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BorrowedMap")
            .field("count", &self.count)
            .finish()
    }
}

impl<'a, K, V> Clone for BorrowedMap<'a, K, V> {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes,
            count: self.count,
            max_size: self.max_size,
            marker: PhantomData,
        }
    }
}

impl<'a, K, V> PartialEq for BorrowedMap<'a, K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes && self.count == other.count
    }
}

impl<'a, K, V> BorrowedMap<'a, K, V> {
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl<'a, K: BorrowedWireCodec<'a> + Ord + Clone, V: BorrowedWireCodec<'a>> BorrowedMap<'a, K, V> {
    pub fn iter(&self) -> BorrowedMapIter<'a, K, V> {
        BorrowedMapIter {
            decoder: Decoder::new(self.bytes, self.max_size).expect("validated view range"),
            remaining: self.count,
            previous: None,
            marker: PhantomData,
        }
    }
}

impl<'a, K: BorrowedWireCodec<'a>, V: BorrowedWireCodec<'a>> BorrowedWireCodec<'a>
    for BorrowedMap<'a, K, V>
{
    fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
        decoder.borrowed_map()
    }

    fn skip_borrowed(decoder: &mut Decoder<'a>) -> Result<(), WireError> {
        decoder.skip_map::<K, V>()
    }
}

pub struct BorrowedMapIter<'a, K, V> {
    decoder: Decoder<'a>,
    remaining: usize,
    previous: Option<K>,
    marker: PhantomData<(K, V)>,
}

impl<'a, K: BorrowedWireCodec<'a> + Ord + Clone, V: BorrowedWireCodec<'a>> Iterator
    for BorrowedMapIter<'a, K, V>
{
    type Item = Result<(K, V), WireError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some((|| {
            let key = K::decode_borrowed(&mut self.decoder)?;
            if self
                .previous
                .as_ref()
                .is_some_and(|previous| key.cmp(previous) != std::cmp::Ordering::Greater)
            {
                return Err(WireError::MalformedConstructor);
            }
            self.previous = Some(key.clone());
            let value = V::decode_borrowed(&mut self.decoder)?;
            Ok((key, value))
        })())
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
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Clears the encoded packet while retaining the allocation for reuse.
    pub fn reset(&mut self) {
        self.bytes.clear();
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub fn write_vectored<W: Write>(
        &self,
        writer: &mut W,
        prefix: &[u8],
        suffix: &[u8],
    ) -> io::Result<usize> {
        let segments = [prefix, self.bytes.as_slice(), suffix];
        let mut segment = 0;
        let mut offset = 0;
        let mut written = 0;
        while segment < segments.len() {
            while segment < segments.len() && offset == segments[segment].len() {
                segment += 1;
                offset = 0;
            }
            if segment == segments.len() {
                break;
            }
            let mut slices = [IoSlice::new(&[]), IoSlice::new(&[]), IoSlice::new(&[])];
            let mut count = 0;
            for (index, value) in segments.iter().enumerate().skip(segment) {
                let start = if index == segment { offset } else { 0 };
                if start < value.len() {
                    slices[count] = IoSlice::new(&value[start..]);
                    count += 1;
                }
            }
            let progress = writer.write_vectored(&slices[..count])?;
            if progress == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "vectored write made no progress",
                ));
            }
            written += progress;
            let mut remaining = progress;
            while remaining > 0 {
                let available = segments[segment].len() - offset;
                if remaining < available {
                    offset += remaining;
                    remaining = 0;
                } else {
                    remaining -= available;
                    segment += 1;
                    offset = 0;
                    while segment < segments.len() && segments[segment].is_empty() {
                        segment += 1;
                    }
                }
            }
        }
        Ok(written)
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

    pub fn bytes_exact(&mut self, value: &[u8], expected: usize) -> Result<(), WireError> {
        if value.len() != expected {
            return Err(WireError::MalformedConstructor);
        }
        self.bytes(value)
    }
    pub fn value<T: WireCodec>(&mut self, value: &T) -> Result<(), WireError> {
        value.encode(self)
    }
    pub fn raw(&mut self, value: &[u8]) -> Result<(), WireError> {
        self.extend(value)
    }

    pub fn fixed_bytes<const N: usize>(&mut self, value: &[u8; N]) -> Result<(), WireError> {
        self.raw(value)
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
    limits: DecodeLimits,
}

impl<'a> Decoder<'a> {
    pub fn new(bytes: &'a [u8], max_size: usize) -> Result<Self, WireError> {
        let limits = DecodeLimits {
            max_packet_size: max_size,
            max_bytes_field_size: max_size,
            ..DecodeLimits::default()
        };
        Self::with_limits(bytes, limits)
    }
    pub fn with_limits(bytes: &'a [u8], limits: DecodeLimits) -> Result<Self, WireError> {
        if bytes.len() > limits.max_packet_size {
            Err(WireError::PacketTooLarge)
        } else {
            Ok(Self {
                bytes,
                position: 0,
                limits,
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

    pub fn bytes_exact(&mut self, expected: usize) -> Result<Vec<u8>, WireError> {
        let value = self.bytes()?;
        if value.len() != expected {
            return Err(WireError::MalformedConstructor);
        }
        Ok(value)
    }
    pub fn bytes_borrowed(&mut self) -> Result<&'a [u8], WireError> {
        let len = usize::try_from(self.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if len > self.limits.max_bytes_field_size {
            return Err(WireError::PacketTooLarge);
        }
        self.read_slice(len)
    }

    pub fn fixed_bytes<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        self.read_array()
    }

    pub fn fixed_bytes_borrowed<const N: usize>(&mut self) -> Result<&'a [u8], WireError> {
        self.read_slice(N)
    }

    pub fn skip_fixed_bytes(&mut self, len: usize) -> Result<(), WireError> {
        self.read_slice(len).map(|_| ())
    }
    pub fn skip_bytes(&mut self) -> Result<(), WireError> {
        let len = usize::try_from(self.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if len > self.limits.max_bytes_field_size {
            return Err(WireError::PacketTooLarge);
        }
        self.read_slice(len).map(|_| ())
    }
    pub fn string(&mut self) -> Result<String, WireError> {
        Ok(self.string_borrowed()?.to_owned())
    }
    pub fn string_borrowed(&mut self) -> Result<&'a str, WireError> {
        std::str::from_utf8(self.bytes_borrowed()?).map_err(|_| WireError::InvalidUtf8)
    }
    pub fn skip_string(&mut self) -> Result<(), WireError> {
        self.skip_bytes()
    }
    pub fn borrowed_vec<T: BorrowedWireCodec<'a>>(
        &mut self,
    ) -> Result<BorrowedVec<'a, T>, WireError> {
        let count = usize::try_from(self.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if count > self.limits.max_collection_items {
            return Err(WireError::PacketTooLarge);
        }
        let start = self.position;
        for _ in 0..count {
            T::skip_borrowed(self)?;
        }
        let bytes = &self.bytes[start..self.position];
        Ok(BorrowedVec {
            bytes,
            count,
            max_size: self.limits.max_packet_size,
            marker: PhantomData,
        })
    }
    pub fn borrowed_map<K: BorrowedWireCodec<'a>, V: BorrowedWireCodec<'a>>(
        &mut self,
    ) -> Result<BorrowedMap<'a, K, V>, WireError> {
        let count = usize::try_from(self.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if count > self.limits.max_collection_items {
            return Err(WireError::PacketTooLarge);
        }
        let start = self.position;
        for _ in 0..count {
            K::skip_borrowed(self)?;
            V::skip_borrowed(self)?;
        }
        let bytes = &self.bytes[start..self.position];
        Ok(BorrowedMap {
            bytes,
            count,
            max_size: self.limits.max_packet_size,
            marker: PhantomData,
        })
    }
    pub fn skip_vec<T: BorrowedWireCodec<'a>>(&mut self) -> Result<(), WireError> {
        let count = usize::try_from(self.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if count > self.limits.max_collection_items {
            return Err(WireError::PacketTooLarge);
        }
        for _ in 0..count {
            T::skip_borrowed(self)?;
        }
        Ok(())
    }
    pub fn skip_map<K: BorrowedWireCodec<'a>, V: BorrowedWireCodec<'a>>(
        &mut self,
    ) -> Result<(), WireError> {
        let count = usize::try_from(self.varint()?).map_err(|_| WireError::IntegerOverflow)?;
        if count > self.limits.max_collection_items {
            return Err(WireError::PacketTooLarge);
        }
        for _ in 0..count {
            K::skip_borrowed(self)?;
            V::skip_borrowed(self)?;
        }
        Ok(())
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

/// Optional values are encoded locally: `0` for None, `1` followed by T for Some.
/// This keeps Optional composable inside collections without synthetic schema fields.
impl<T: WireCodec> WireCodec for Option<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), WireError> {
        match self {
            Some(value) => {
                encoder.u8(1)?;
                value.encode(encoder)
            }
            None => encoder.u8(0),
        }
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.u8()? {
            0 => Ok(None),
            1 => Ok(Some(T::decode(decoder)?)),
            _ => Err(WireError::MalformedConstructor),
        }
    }

    fn encoded_len(&self) -> usize {
        1 + self.as_ref().map_or(0, WireCodec::encoded_len)
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
        if count > decoder.limits.max_collection_items {
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
        if count > decoder.limits.max_collection_items {
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

    #[test]
    fn fixed_byte_arrays_round_trip_without_length_prefix() {
        let value = [7u8; 16];
        let encoded = crate::encode_value(&value).unwrap();
        assert_eq!(encoded, vec![7; 16]);
        assert_eq!(crate::decode_value::<[u8; 16]>(&encoded).unwrap(), value);
        assert!(crate::decode_value::<[u8; 16]>(&encoded[..15]).is_err());
    }
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
    fn optional_values_use_local_presence_markers_and_compose_in_vectors() {
        let value = vec![Some("yes".to_owned()), None, Some("no".to_owned())];
        let encoded = crate::encode_value(&value).unwrap();
        assert_eq!(encoded[0], 3);
        assert_eq!(encoded[1], 1);
        assert_eq!(
            crate::decode_value::<Vec<Option<String>>>(&encoded).unwrap(),
            value
        );
        assert_eq!(
            crate::decode_value::<Option<String>>(&[2]),
            Err(WireError::MalformedConstructor)
        );
    }

    #[test]
    fn exact_byte_helpers_keep_vec_length_prefix_and_reject_wrong_lengths() {
        let mut encoder = Encoder::new(crate::DEFAULT_MAX_PACKET_SIZE);
        encoder.bytes_exact(b"abcd", 4).unwrap();
        assert_eq!(encoder.finish().unwrap(), vec![4, b'a', b'b', b'c', b'd']);
        let mut decoder =
            Decoder::new(&[4, b'a', b'b', b'c', b'd'], crate::DEFAULT_MAX_PACKET_SIZE).unwrap();
        assert_eq!(decoder.bytes_exact(4).unwrap(), b"abcd");
        let mut decoder =
            Decoder::new(&[3, b'a', b'b', b'c'], crate::DEFAULT_MAX_PACKET_SIZE).unwrap();
        assert_eq!(decoder.bytes_exact(4), Err(WireError::MalformedConstructor));
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
    fn borrowed_collections_iterate_without_copying_elements() {
        let mut encoder = Encoder::new(LIMIT);
        encoder.varint(2).unwrap();
        encoder.bytes(b"first").unwrap();
        encoder.bytes(b"second").unwrap();
        let packet = encoder.finish().unwrap();
        let mut decoder = Decoder::new(&packet, LIMIT).unwrap();
        let values: BorrowedVec<'_, &'_ [u8]> = decoder.borrowed_vec().unwrap();
        let first = values.iter().next().unwrap().unwrap();
        let second = values.iter().nth(1).unwrap().unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(first, b"first");
        assert_eq!(second, b"second");
        let start = packet.as_ptr() as usize;
        let end = start + packet.len();
        assert!((start..end).contains(&(first.as_ptr() as usize)));
        assert!((start..end).contains(&(second.as_ptr() as usize)));
        assert!(decoder.is_finished());
    }

    #[test]
    fn borrowed_collection_scan_rejects_truncated_items() {
        let packet = [1, 3, b'x'];
        let mut decoder = Decoder::new(&packet, LIMIT).unwrap();
        assert!(matches!(
            decoder.borrowed_vec::<&str>(),
            Err(WireError::UnexpectedEof)
        ));
    }

    #[test]
    fn structural_skip_advances_without_utf8_decoding() {
        let packet = [2, 2, 0xff, 1, 1, b'x'];
        let mut decoder = Decoder::new(&packet, LIMIT).unwrap();
        decoder.skip_vec::<&str>().unwrap();
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
    fn encoder_reset_retains_buffer_for_hot_path_reuse() {
        let mut encoder = Encoder::with_capacity(LIMIT, 64);
        encoder.bytes(b"first packet").unwrap();
        let capacity = encoder.as_bytes().as_ptr();
        encoder.reset();
        encoder.bytes(b"second packet").unwrap();
        assert_eq!(
            encoder.as_bytes(),
            &[
                13, b's', b'e', b'c', b'o', b'n', b'd', b' ', b'p', b'a', b'c', b'k', b'e', b't'
            ]
        );
        assert_eq!(encoder.as_bytes().as_ptr(), capacity);
    }

    #[test]
    fn decoder_applies_independent_collection_and_bytes_limits() {
        let limits = crate::DecodeLimits {
            max_packet_size: LIMIT,
            max_collection_items: 2,
            max_bytes_field_size: 3,
        };
        let mut bytes = Encoder::new(LIMIT);
        bytes.bytes(b"four").unwrap();
        let packet = bytes.finish().unwrap();
        let mut decoder = Decoder::with_limits(&packet, limits).unwrap();
        assert_eq!(decoder.bytes_borrowed(), Err(WireError::PacketTooLarge));

        let mut values = Encoder::new(LIMIT);
        values.varint(3).unwrap();
        values.u8(1).unwrap();
        values.u8(2).unwrap();
        values.u8(3).unwrap();
        let packet = values.finish().unwrap();
        let mut decoder = Decoder::with_limits(&packet, limits).unwrap();
        assert_eq!(decoder.skip_vec::<u8>(), Err(WireError::PacketTooLarge));
    }

    #[test]
    fn vectored_write_keeps_framing_segments_separate() {
        let mut encoder = Encoder::new(LIMIT);
        encoder.bytes(b"payload").unwrap();
        let mut output = Vec::new();
        let written = encoder
            .write_vectored(&mut output, b"header", b"trailer")
            .unwrap();
        assert_eq!(written, 6 + encoder.len() + 7);
        assert_eq!(
            output,
            [b"header".as_slice(), encoder.as_bytes(), b"trailer"].concat()
        );
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
