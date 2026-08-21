use crate::limits::DecodeLimits;
use crate::wire::{BorrowedWireCodec, WireCodec, WireError};

pub const DEFAULT_MAX_PACKET_SIZE: usize = crate::limits::MAX_PACKET_SIZE;

pub trait TypikonCodec: Sized {
    fn encode(&self) -> Result<Vec<u8>, WireError>;
    fn decode(bytes: &[u8]) -> Result<Self, WireError>;
    fn decode_with_limits(bytes: &[u8], limits: DecodeLimits) -> Result<Self, WireError>
    where
        Self: WireCodec,
    {
        let mut decoder = crate::wire::Decoder::with_limits(bytes, limits)?;
        let value = <Self as WireCodec>::decode(&mut decoder)?;
        if decoder.is_finished() {
            Ok(value)
        } else {
            Err(WireError::MalformedConstructor)
        }
    }
}

pub fn encode_value<T: WireCodec>(value: &T) -> Result<Vec<u8>, WireError> {
    let encoded_len = value.encoded_len();
    if encoded_len > DEFAULT_MAX_PACKET_SIZE {
        return Err(WireError::PacketTooLarge);
    }
    let capacity = if encoded_len == 0 { 128 } else { encoded_len };
    let mut encoder = crate::wire::Encoder::with_capacity(DEFAULT_MAX_PACKET_SIZE, capacity);
    value.encode(&mut encoder)?;
    encoder.finish()
}

pub fn decode_value<T: WireCodec>(bytes: &[u8]) -> Result<T, WireError> {
    let mut decoder = crate::wire::Decoder::with_limits(bytes, DecodeLimits::default())?;
    let value = T::decode(&mut decoder)?;
    if decoder.is_finished() {
        Ok(value)
    } else {
        Err(WireError::MalformedConstructor)
    }
}

pub fn decode_borrowed_value<'a, T: BorrowedWireCodec<'a>>(
    bytes: &'a [u8],
) -> Result<T, WireError> {
    decode_borrowed_value_with_limits(bytes, DecodeLimits::default())
}

pub fn decode_borrowed_value_with_limits<'a, T: BorrowedWireCodec<'a>>(
    bytes: &'a [u8],
    limits: DecodeLimits,
) -> Result<T, WireError> {
    let mut decoder = crate::wire::Decoder::with_limits(bytes, limits)?;
    let value = T::decode_borrowed(&mut decoder)?;
    if decoder.is_finished() {
        Ok(value)
    } else {
        Err(WireError::MalformedConstructor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{Decoder, Encoder};

    #[derive(Debug, PartialEq)]
    struct BorrowedMessage<'a> {
        text: &'a str,
        data: &'a [u8],
    }

    impl<'a> BorrowedWireCodec<'a> for BorrowedMessage<'a> {
        fn decode_borrowed(decoder: &mut Decoder<'a>) -> Result<Self, WireError> {
            Ok(Self {
                text: decoder.string_borrowed()?,
                data: decoder.bytes_borrowed()?,
            })
        }
    }

    #[test]
    fn top_level_borrowed_decode_shares_packet_storage() {
        let mut encoder = Encoder::new(DEFAULT_MAX_PACKET_SIZE);
        encoder.bytes(b"hello").unwrap();
        encoder.bytes(b"payload").unwrap();
        let packet = encoder.finish().unwrap();
        let decoded = decode_borrowed_value::<BorrowedMessage<'_>>(&packet).unwrap();
        assert_eq!(decoded.text, "hello");
        assert_eq!(decoded.data, b"payload");
        let start = packet.as_ptr() as usize;
        let end = start + packet.len();
        assert!((start..end).contains(&(decoded.text.as_ptr() as usize)));
        assert!((start..end).contains(&(decoded.data.as_ptr() as usize)));
    }

    #[test]
    fn borrowed_decode_accepts_custom_limits() {
        let mut encoder = Encoder::new(DEFAULT_MAX_PACKET_SIZE);
        encoder.bytes(b"payload").unwrap();
        let packet = encoder.finish().unwrap();
        let limits = DecodeLimits {
            max_packet_size: DEFAULT_MAX_PACKET_SIZE,
            max_collection_items: 1,
            max_bytes_field_size: 3,
        };
        assert_eq!(
            decode_borrowed_value_with_limits::<&[u8]>(&packet, limits),
            Err(WireError::PacketTooLarge)
        );
    }
}
