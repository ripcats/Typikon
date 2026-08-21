use crate::wire::{WireCodec, WireError};

pub const DEFAULT_MAX_PACKET_SIZE: usize = crate::limits::MAX_PACKET_SIZE;

pub trait TypikonCodec: Sized {
    fn encode(&self) -> Result<Vec<u8>, WireError>;
    fn decode(bytes: &[u8]) -> Result<Self, WireError>;
}

pub fn encode_value<T: WireCodec>(value: &T) -> Result<Vec<u8>, WireError> {
    let mut encoder = crate::wire::Encoder::with_capacity(DEFAULT_MAX_PACKET_SIZE, 128);
    value.encode(&mut encoder)?;
    encoder.finish()
}

pub fn decode_value<T: WireCodec>(bytes: &[u8]) -> Result<T, WireError> {
    let mut decoder = crate::wire::Decoder::new(bytes, DEFAULT_MAX_PACKET_SIZE)?;
    let value = T::decode(&mut decoder)?;
    if decoder.is_finished() {
        Ok(value)
    } else {
        Err(WireError::MalformedConstructor)
    }
}
