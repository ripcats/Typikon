//! Minimal stable C ABI used by future cgo and Node-API bindings.

use std::slice;

pub const ABI_VERSION: u16 = 1;
pub const LAYER_UNSUPPORTED: i32 = -1;
pub const INVALID_ARGUMENT: i32 = -2;

#[unsafe(no_mangle)]
pub extern "C" fn typikon_abi_version() -> u16 {
    ABI_VERSION
}

/// Returns the requested Layer, `-1` when unsupported, or `-2` for a null list.
#[unsafe(no_mangle)]
pub extern "C" fn typikon_negotiate_layer(
    requested: u16,
    supported: *const u16,
    count: usize,
) -> i32 {
    if count > 0 && supported.is_null() {
        return INVALID_ARGUMENT;
    }
    let layers = if count == 0 {
        &[]
    } else {
        // SAFETY: the caller owns a readable array of `count` u16 values for this call.
        unsafe { slice::from_raw_parts(supported, count) }
    };
    if layers.contains(&requested) {
        requested as i32
    } else {
        LAYER_UNSUPPORTED
    }
}

/// Frees a byte buffer allocated by the Rust ABI.
#[unsafe(no_mangle)]
pub extern "C" fn typikon_free_bytes(ptr: *mut u8, len: usize, capacity: usize) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: `ptr`, `len`, and `capacity` must be the values returned by Rust allocation API.
    unsafe {
        drop(Vec::from_raw_parts(ptr, len, capacity));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_abi_layer_negotiation_is_deterministic() {
        let layers = [6, 8, 10];
        assert_eq!(typikon_abi_version(), 1);
        assert_eq!(typikon_negotiate_layer(8, layers.as_ptr(), layers.len()), 8);
        assert_eq!(
            typikon_negotiate_layer(9, layers.as_ptr(), layers.len()),
            LAYER_UNSUPPORTED
        );
        assert_eq!(
            typikon_negotiate_layer(8, std::ptr::null(), 1),
            INVALID_ARGUMENT
        );
    }
}
