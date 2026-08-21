use crate::ast::{Enum, EnumVariant, Field, Struct, Type};

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
    format!("{guard}{}:{}", field.name, type_canonical(&field.ty))
}

fn type_canonical(ty: &Type) -> String {
    match ty {
        Type::Primitive(name) => name.clone(),
        Type::Vec(item) => format!("Vec<{}>", type_canonical(item)),
        Type::Map(key, value) => format!("Map<{},{}>", type_canonical(key), type_canonical(value)),
    }
}

pub fn constructor_cid(item: &Struct) -> String {
    let hash = blake3::hash(canonical_form(item).as_bytes());
    hash.to_hex()[..16].to_owned()
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
