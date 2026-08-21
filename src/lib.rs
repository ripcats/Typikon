//! Public API for parsing Typikon schemas.

mod artifacts;
mod ast;
mod bridge;
mod codec;
mod codegen;
mod constructor;
mod error;
mod ffi;
mod fingerprint;
mod layer;
mod limits;
mod parser;
mod validate;
mod wire;

pub use artifacts::{GeneratedArtifacts, compile_schema};
pub use ast::{Enum, EnumVariant, Field, Flags, FlagsBit, Item, Schema, Struct, Type};
pub use bridge::{
    BridgeKind, generate_bridge, generate_c_header, generate_go_binding, generate_python_binding,
    generate_typescript_binding,
};
pub use codec::{
    DEFAULT_MAX_PACKET_SIZE, TypikonCodec, decode_borrowed_value, decode_value, encode_value,
};
pub use codegen::{generate_public_schema, generate_rust};
pub use constructor::CID_BYTES;
pub use constructor::{
    ConstructorDecoder, ConstructorEncoder, cid_bytes, constructor_cid, constructor_cid_bytes,
};
pub use error::ParseError;
pub use fingerprint::{canonical_form, constructor_cid as struct_constructor_cid, variant_cid};
pub use layer::{LayerSupport, LayerVersionNotSupported};
pub use limits::{MAX_NESTING_DEPTH, MAX_PACKET_SIZE};
pub use parser::parse_schema;
pub use validate::validate;
pub use wire::{
    BorrowedMap, BorrowedMapIter, BorrowedVec, BorrowedVecIter, BorrowedWireCodec, Decoder,
    Encoder, WireCodec, WireError, varint_len,
};

/// Stable ABI version exposed to language bindings.
pub fn ffi_abi_version() -> u16 {
    ffi::ABI_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    const PRIMITIVES: [&str; 14] = [
        "bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "f32", "f64",
        "String",
    ];

    #[test]
    fn primitive_types_have_individual_cases() {
        for (index, primitive) in PRIMITIVES.iter().enumerate() {
            let schema = parse_schema(&format!(
                "#[version({index})] struct Value {{ field: {primitive}, }}"
            ))
            .unwrap();
            let Item::Struct(item) = &schema.items[0] else {
                panic!()
            };
            assert_eq!(schema.version, index as u16);
            assert_eq!(item.fields[0].ty, Type::Primitive((*primitive).into()));
        }
    }

    #[test]
    fn parses_enum_variants_and_fields() {
        let schema = parse_schema(
            "#[version(1)] enum Message { Text { text: String, }, Image { data: Vec<u8>, }, }",
        )
        .unwrap();
        let Item::Enum(item) = &schema.items[0] else {
            panic!()
        };
        assert_eq!(item.name, "Message");
        assert_eq!(item.variants.len(), 2);
        assert_eq!(
            item.variants[0].fields[0].ty,
            Type::Primitive("String".into())
        );
        assert_eq!(
            item.variants[1].fields[0].ty,
            Type::Vec(Box::new(Type::Primitive("u8".into())))
        );
    }

    #[test]
    fn parses_unit_enum_with_values() {
        let schema =
            parse_schema("#[version(1)] enum Status { Online = 0, Offline = 1, }").unwrap();
        let Item::Enum(item) = &schema.items[0] else {
            panic!()
        };
        assert_eq!(
            item.variants.iter().map(|v| v.value).collect::<Vec<_>>(),
            vec![Some(0), Some(1)]
        );
    }

    #[test]
    fn parses_flags() {
        let schema = parse_schema(
            "#[version(3)] #[flags(u16)] enum UserFlags { IsBot = 0, IsPremium = 2, }",
        )
        .unwrap();
        let Item::Flags(flags) = &schema.items[0] else {
            panic!()
        };
        assert_eq!(flags.name, "UserFlags");
        assert_eq!(flags.underlying, "u16");
        assert_eq!(flags.bits[0].value, 0);
        assert_eq!(flags.bits[1].name, "IsPremium");
    }

    #[test]
    fn parses_guard_and_cid_and_calculates_fingerprint() {
        let schema = parse_schema("#[version(1)] #[flags(u8)] enum F { Ready = 0, } struct Message { flags: F, #[guard(flags.ready)] value: u64, }").unwrap();
        let Item::Struct(item) = &schema.items[1] else {
            panic!()
        };
        assert_eq!(item.cid, None);
        assert_eq!(item.fields[1].guard.as_deref(), Some("flags.ready"));
        assert_eq!(canonical_form(item), "Message|flags:F|[ready]value:u64");
        assert_eq!(struct_constructor_cid(item).len(), 16);
    }

    #[test]
    fn rejects_invalid_semantics() {
        for source in [
            "#[version(1)] struct A { value: Missing, }",
            "#[version(1)] struct A { x: u8, x: u16, }",
            "#[version(1)] struct A { #[guard(flags.ready)] value: u64, }",
            "#[version(1)] #[flags(String)] enum F { Ready = 0, }",
            "#[version(1)] #[cid(1234)] struct A {}",
            "#[version(1)] struct A {} struct B { values: Map<A, u8>, }",
            "#[version(1)] struct A { values: Map<f32, u8>, }",
            "#[version(1)] #[flags(u8)] enum F { A = 0, B = 0, }",
            "#[version(1)] #[flags(u8)] enum F { A = 8, }",
            "#[version(1)] enum E { A = 0, B = 0, }",
            "#[version(1)] enum E { #[cid(aaaaaaaaaaaaaaaa)] A, }",
            "#[version(1)] #[cid(aaaaaaaaaaaaaaaa)] struct A {}",
            "#[version(1)] struct 1A {}",
            "#[version(18446744073709551616)] struct A {}",
            "#[version(65536)] struct A {}",
        ] {
            assert!(
                parse_schema(source).is_err(),
                "accepted invalid semantics: {source}"
            );
        }
    }

    #[test]
    fn rejects_many_invalid_schemas() {
        let invalid = [
            "",
            "struct A {}",
            "#[version()] struct A {}",
            "#[version(x)] struct A {}",
            "#[version(1)] unknown A {}",
            "#[version(1)] struct {}",
            "#[version(1)] struct A { field }",
            "#[version(1)] struct A { field: }",
            "#[version(1)] struct A { field: Vec<u8, }",
            "#[version(1)] struct A { field: Map<u8>, }",
            "#[version(1)] enum A { X }",
            "#[version(1)] enum A { X = , }",
            "#[version(1)] #[flags()] enum F {}",
            "#[version(1)] #[flags(u8)] enum F { A, }",
        ];
        for source in invalid {
            assert!(parse_schema(source).is_err(), "accepted: {source:?}");
        }
    }

    #[test]
    fn accepts_whitespace_comments_and_deep_nesting() {
        for gap in [" ", "\n", "\t", "\r\n", " // comment\n "] {
            let source =
                format!("#[version(1)]{gap}struct A {{ value: Map<String, Vec<Map<u8, u64>>>, }}");
            assert!(parse_schema(&source).is_ok(), "rejected gap {gap:?}");
        }
        for depth in 1..=20 {
            let mut ty = "u8".to_owned();
            for _ in 0..depth {
                ty = format!("Vec<{ty}>");
            }
            assert!(parse_schema(&format!("#[version(1)] struct Deep {{ value: {ty}, }}")).is_ok());
        }
    }

    #[test]
    fn enforces_type_nesting_limit() {
        for depth in [100, 101] {
            let mut ty = "u8".to_owned();
            for _ in 0..depth {
                ty = format!("Vec<{ty}>");
            }
            let source = format!("#[version(1)] struct Deep {{ value: {ty}, }}");
            assert_eq!(parse_schema(&source).is_ok(), depth == 100);
        }
    }

    #[test]
    fn malformed_schema_corpus_never_panics() {
        let alphabet = b"#[](){}<>,:=._abcdefghijklmnopqrstuvwxyz0123456789 \n";
        let mut state = 0x517cc1b727220a95u64;
        for length in 0..512 {
            let mut source = String::with_capacity(length);
            for _ in 0..length {
                state = state
                    .wrapping_mul(2862933555777941757)
                    .wrapping_add(3037000493);
                source.push(alphabet[(state >> 32) as usize % alphabet.len()] as char);
            }
            let result = std::panic::catch_unwind(|| parse_schema(&source));
            assert!(result.is_ok(), "parser panicked for {length} bytes");
        }
    }

    #[test]
    fn accepts_one_hundred_generated_schemas() {
        for version in 0..100 {
            let source = format!(
                "#[version({version})] struct Item{version} {{ id: u64, values: Vec<String>, }}"
            );
            let schema = parse_schema(&source).unwrap();
            assert_eq!(schema.version, version as u16);
            assert_eq!(schema.items.len(), 1);
        }
    }
}
