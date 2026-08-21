pub const MAX_PACKET_SIZE: usize = 4 * 1024 * 1024;
pub const MAX_COLLECTION_ITEMS: usize = 1_000_000;
pub const MAX_BYTES_FIELD_SIZE: usize = MAX_PACKET_SIZE;
pub const MAX_NESTING_DEPTH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_packet_size: usize,
    pub max_collection_items: usize,
    pub max_bytes_field_size: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_packet_size: MAX_PACKET_SIZE,
            max_collection_items: MAX_COLLECTION_ITEMS,
            max_bytes_field_size: MAX_BYTES_FIELD_SIZE,
        }
    }
}
