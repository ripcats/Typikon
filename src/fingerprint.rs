use crate::ast::{Enum, EnumVariant, Field, Item, Schema, Struct, Type};

pub fn canonical_form(item: &Struct) -> String {
    let fields = item
        .fields
        .iter()
        .map(field_canonical)
        .collect::<Vec<_>>()
        .join("|");
    format!("{}|{}", item.name, fields)
}

fn field_canonical(field: &Field) -> String {
    let guard = field
        .guard
        .as_deref()
        .map(|g| format!("[{}]", g.rsplit('.').next().unwrap_or(g)))
        .unwrap_or_default();
    let exact_len = field
        .exact_len
        .map(|length| format!("{{exact_len={length}}}"))
        .unwrap_or_default();
    format!(
        "{guard}{}:{}{}",
        field.name,
        type_canonical(&field.ty),
        exact_len
    )
}

fn type_canonical(ty: &Type) -> String {
    match ty {
        Type::Primitive(name) => name.clone(),
        Type::FixedBytes(length) => format!("bytes[{length}]"),
        Type::Optional(item) => format!("Optional<{}>", type_canonical(item)),
        Type::Vec(item) => format!("Vec<{}>", type_canonical(item)),
        Type::Map(key, value) => format!("Map<{},{}>", type_canonical(key), type_canonical(value)),
    }
}

pub fn constructor_cid(item: &Struct) -> String {
    let hash = blake3::hash(canonical_form(item).as_bytes());
    hash.to_hex()[..16].to_owned()
}

pub fn constructor_cid_with_schema(item: &Struct, schema: &Schema) -> String {
    let hash = blake3::hash(canonical_form_with_schema(item, schema).as_bytes());
    hash.to_hex()[..16].to_owned()
}

pub fn canonical_form_with_schema(item: &Struct, schema: &Schema) -> String {
    let fields = item
        .fields
        .iter()
        .map(|field| field_canonical_with_schema(field, schema))
        .collect::<Vec<_>>()
        .join("|");
    format!("{}|{}", item.name, fields)
}

pub fn variant_cid(item: &Enum, variant: &EnumVariant) -> String {
    let fields = variant
        .fields
        .iter()
        .map(field_canonical)
        .collect::<Vec<_>>()
        .join("|");
    let canonical = format!("{}|{}|{}", item.name, variant.name, fields);
    blake3::hash(canonical.as_bytes()).to_hex()[..16].to_owned()
}

pub fn variant_cid_with_schema(item: &Enum, variant: &EnumVariant, schema: &Schema) -> String {
    let fields = variant
        .fields
        .iter()
        .map(|field| field_canonical_with_schema(field, schema))
        .collect::<Vec<_>>()
        .join("|");
    let canonical = format!("{}|{}|{}", item.name, variant.name, fields);
    blake3::hash(canonical.as_bytes()).to_hex()[..16].to_owned()
}

fn field_canonical_with_schema(field: &Field, schema: &Schema) -> String {
    let guard = field
        .guard
        .as_deref()
        .map(|g| format!("[{}]", g.rsplit('.').next().unwrap_or(g)))
        .unwrap_or_default();
    let exact_len = field
        .exact_len
        .map(|length| format!("{{exact_len={length}}}"))
        .unwrap_or_default();
    format!(
        "{guard}{}:{}{}",
        field.name,
        type_canonical_with_schema(&field.ty, schema, &mut Vec::new()),
        exact_len
    )
}

fn type_canonical_with_schema(ty: &Type, schema: &Schema, visiting: &mut Vec<String>) -> String {
    match ty {
        Type::Primitive(name) => {
            let Some(Item::Alias(alias)) = schema
                .items
                .iter()
                .find(|item| matches!(item, Item::Alias(alias) if alias.name == *name))
            else {
                return name.clone();
            };
            if visiting.iter().any(|current| current == name) {
                return name.clone();
            }
            visiting.push(name.clone());
            let constraint = alias
                .exact_len
                .map(|n| format!("{{exact_len={n}}}"))
                .unwrap_or_default();
            let result = format!(
                "{}{}",
                type_canonical_with_schema(&alias.ty, schema, visiting),
                constraint
            );
            visiting.pop();
            result
        }
        Type::FixedBytes(length) => format!("bytes[{length}]"),
        Type::Optional(item) => format!(
            "Optional<{}>",
            type_canonical_with_schema(item, schema, visiting)
        ),
        Type::Vec(item) => format!(
            "Vec<{}>",
            type_canonical_with_schema(item, schema, visiting)
        ),
        Type::Map(key, value) => format!(
            "Map<{},{}>",
            type_canonical_with_schema(key, schema, visiting),
            type_canonical_with_schema(value, schema, visiting)
        ),
    }
}
