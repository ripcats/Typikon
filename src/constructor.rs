use crate::wire::{Decoder, Encoder, WireCodec, WireError};

pub const CID_BYTES: usize = 8;

pub fn constructor_cid(bytes: &[u8], max_size: usize) -> Result<String, WireError> {
    let decoder = Decoder::new(bytes, max_size)?;
    let raw = decoder.peek_raw(CID_BYTES)?;
    Ok(raw.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn cid_bytes(cid: &str) -> Result<[u8; CID_BYTES], WireError> {
    parse_cid(cid)
}

pub struct ConstructorEncoder {
    inner: Encoder,
}

impl ConstructorEncoder {
    pub fn new(cid: &str, max_size: usize) -> Result<Self, WireError> {
        let mut inner = Encoder::new(max_size);
        inner.raw(&parse_cid(cid)?)?;
        Ok(Self { inner })
    }
    pub fn field<T: WireCodec>(&mut self, value: &T) -> Result<(), WireError> {
        self.inner.value(value)
    }
    pub fn guarded<T: WireCodec>(&mut self, enabled: bool, value: &T) -> Result<(), WireError> {
        if enabled {
            self.field(value)?;
        }
        Ok(())
    }
    pub fn finish(self) -> Result<Vec<u8>, WireError> {
        self.inner.finish()
    }
}

pub struct ConstructorDecoder<'a> {
    inner: Decoder<'a>,
}

impl<'a> ConstructorDecoder<'a> {
    pub fn new(bytes: &'a [u8], expected_cid: &str, max_size: usize) -> Result<Self, WireError> {
        let mut inner = Decoder::new(bytes, max_size)?;
        let actual = inner.read_raw(CID_BYTES)?;
        if actual != parse_cid(expected_cid)? {
            return Err(WireError::UnknownCId);
        }
        Ok(Self { inner })
    }
    pub fn field<T: WireCodec>(&mut self) -> Result<T, WireError> {
        self.inner.value()
    }
    pub fn finish(self) -> Result<(), WireError> {
        if self.inner.is_finished() {
            Ok(())
        } else {
            Err(WireError::MalformedConstructor)
        }
    }
}

fn parse_cid(cid: &str) -> Result<[u8; CID_BYTES], WireError> {
    if cid.len() != CID_BYTES * 2 || !cid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(WireError::InvalidCId);
    }
    let mut bytes = [0; CID_BYTES];
    for (index, pair) in cid.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex(pair[0])? << 4) | hex(pair[1])?;
    }
    Ok(bytes)
}

fn hex(byte: u8) -> Result<u8, WireError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(WireError::InvalidCId),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CID: &str = "a81f2031d72c9f04";

    #[test]
    fn constructor_round_trip_and_guard() {
        let mut encoder = ConstructorEncoder::new(CID, 1024).unwrap();
        encoder.field(&42u64).unwrap();
        encoder.guarded(false, &99u64).unwrap();
        encoder.guarded(true, &7u8).unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(
            &bytes[..8],
            &[0xa8, 0x1f, 0x20, 0x31, 0xd7, 0x2c, 0x9f, 0x04]
        );
        let mut decoder = ConstructorDecoder::new(&bytes, CID, 1024).unwrap();
        assert_eq!(decoder.field::<u64>().unwrap(), 42);
        assert_eq!(decoder.field::<u8>().unwrap(), 7);
        decoder.finish().unwrap();
    }

    #[test]
    fn constructor_rejects_cid_and_trailing_bytes() {
        assert!(matches!(
            ConstructorEncoder::new("bad", 1024),
            Err(WireError::InvalidCId)
        ));
        let mut encoder = ConstructorEncoder::new(CID, 1024).unwrap();
        encoder.field(&1u8).unwrap();
        let mut bytes = encoder.finish().unwrap();
        bytes.push(1);
        let mut decoder = ConstructorDecoder::new(&bytes, CID, 1024).unwrap();
        assert_eq!(decoder.field::<u8>().unwrap(), 1);
        assert_eq!(decoder.finish(), Err(WireError::MalformedConstructor));
        assert!(matches!(
            ConstructorDecoder::new(&bytes, "0000000000000000", 1024),
            Err(WireError::UnknownCId)
        ));
    }
}
