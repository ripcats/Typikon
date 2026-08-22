use crate::ast::{Field, Item, Schema, Type};
use crate::error::ParseError;
use crate::fingerprint::{constructor_cid, variant_cid};
use std::collections::HashSet;

pub fn validate(schema: &Schema) -> Result<(), ParseError> {
    let mut names = HashSet::new();
    for item in &schema.items {
        let name = match item {
            Item::Alias(x) => &x.name,
            Item::Struct(x) => &x.name,
            Item::Enum(x) => &x.name,
            Item::Flags(x) => &x.name,
        };
        if !names.insert(name) {
            return Err(error("duplicate item name"));
        }
    }
    for item in &schema.items {
        match item {
            Item::Alias(x) => validate_type(&x.ty, schema)?,
            Item::Struct(x) => {
                validate_fields(&x.fields, schema)?;
                if let Some(cid) = &x.cid
                    && cid != &constructor_cid(x)
                {
                    return Err(error("struct C-ID does not match its canonical form"));
                }
            }
            Item::Enum(x) => {
                let mut variants = HashSet::new();
                let mut values = HashSet::new();
                let unit_enum = x.variants.iter().all(|variant| variant.fields.is_empty());
                for variant in &x.variants {
                    if !variants.insert(&variant.name) {
                        return Err(error("duplicate enum variant name"));
                    }
                    validate_fields(&variant.fields, schema)?;
                    if unit_enum {
                        if variant.cid.is_some() {
                            return Err(error("unit enum variant cannot have a C-ID"));
                        }
                        let Some(value) = variant.value else {
                            return Err(error("unit enum variant must have an integer value"));
                        };
                        if !values.insert(value) {
                            return Err(error("duplicate enum variant value"));
                        }
                    } else if !variant.fields.is_empty()
                        && let Some(cid) = &variant.cid
                        && cid != &variant_cid(x, variant)
                    {
                        return Err(error("enum variant C-ID does not match its canonical form"));
                    }
                }
            }
            Item::Flags(x) => {
                let width = match x.underlying.as_str() {
                    "u8" => 8,
                    "u16" => 16,
                    "u32" => 32,
                    "u64" => 64,
                    "u128" => 128,
                    _ => 0,
                };
                let mut bits = HashSet::new();
                let mut values = HashSet::new();
                if width == 0
                    || x.bits.iter().any(|b| b.value >= width)
                    || x.bits.iter().any(|b| !bits.insert(normalize_name(&b.name)))
                    || x.bits.iter().any(|b| !values.insert(b.value))
                {
                    return Err(error("invalid flags underlying type or bit"));
                }
            }
        }
    }
    Ok(())
}

fn validate_fields(fields: &[Field], schema: &Schema) -> Result<(), ParseError> {
    let mut names = HashSet::new();
    for (index, field) in fields.iter().enumerate() {
        if !names.insert(&field.name) {
            return Err(error("duplicate field name"));
        }
        validate_type(&field.ty, schema)?;
        if field.exact_len.is_some() && !is_bytes_vec(&field.ty) {
            return Err(error("exact_len is only valid for Vec<u8>"));
        }
        if matches!(field.ty, Type::FixedBytes(_)) && field.exact_len.is_some() {
            return Err(error("exact_len cannot be used with fixed bytes"));
        }
        if let Some(guard) = &field.guard {
            let (owner, bit) = guard
                .split_once('.')
                .ok_or_else(|| error("malformed guard"))?;
            let found = fields[..index].iter().find(|f| f.name == owner);
            let Some(flag_field) = found else {
                return Err(error("guard must reference an earlier field"));
            };
            let Type::Primitive(flag_type) = &flag_field.ty else {
                return Err(error("guard owner must be a flags type"));
            };
            let valid = schema.items.iter().any(|item| matches!(item, Item::Flags(flags) if &flags.name == flag_type && flags.bits.iter().any(|b| normalize_name(&b.name) == normalize_name(bit))));
            if !valid {
                return Err(error("guard references an unknown flag bit"));
            }
        }
    }
    Ok(())
}

fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn validate_type(ty: &Type, schema: &Schema) -> Result<(), ParseError> {
    match ty {
        Type::Primitive(name)
            if [
                "bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
                "f32", "f64", "String",
            ]
            .contains(&name.as_str()) =>
        {
            Ok(())
        }
        Type::Primitive(name)
            if schema.items.iter().any(|item| match item {
                Item::Alias(x) => &x.name == name,
                Item::Struct(x) => &x.name == name,
                Item::Enum(x) => &x.name == name,
                Item::Flags(x) => &x.name == name,
            }) =>
        {
            Ok(())
        }
        Type::Primitive(_) => Err(error("unknown type")),
        Type::FixedBytes(length) if *length > 0 => Ok(()),
        Type::FixedBytes(_) => Err(error("fixed byte length must be greater than zero")),
        Type::Vec(item) => validate_type(item, schema),
        Type::Map(key, value) => {
            if !matches!(key.as_ref(), Type::Primitive(name) if is_map_key_type(name)) {
                return Err(error("map key must be an orderable primitive type"));
            }
            validate_type(key, schema)?;
            validate_type(value, schema)
        }
    }
}

fn is_bytes_vec(ty: &Type) -> bool {
    matches!(ty, Type::Vec(item) if matches!(item.as_ref(), Type::Primitive(name) if name == "u8"))
}

fn is_primitive_name(name: &str) -> bool {
    [
        "bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "f32", "f64",
        "String",
    ]
    .contains(&name)
}

fn is_map_key_type(name: &str) -> bool {
    is_primitive_name(name) && !matches!(name, "f32" | "f64")
}

fn error(message: &str) -> ParseError {
    ParseError {
        message: message.into(),
        position: 0,
    }
}

#[cfg(test)]
mod tests {
    use crate::parse_schema;

    #[test]
    fn rejects_float_map_keys_as_a_schema_error() {
        for key in ["f32", "f64"] {
            let source = format!("#[version(1)] struct Message {{ values: Map<{key}, String>, }}");
            let error = parse_schema(&source).unwrap_err();
            assert_eq!(error.message, "map key must be an orderable primitive type");
        }
    }

    #[test]
    fn rejects_non_primitive_map_keys_without_panicking() {
        let error = parse_schema(
            "#[version(1)] struct Key { id: u64, } struct Message { values: Map<Key, String>, }",
        )
        .unwrap_err();
        assert_eq!(error.message, "map key must be an orderable primitive type");
    }
}
