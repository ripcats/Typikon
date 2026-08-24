use crate::ast::{Enum, EnumVariant, Field, Flags, Item, Schema, Struct, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityError {
    ItemKindChanged { name: String },
    ExistingItemChanged { name: String },
    ExistingEnumVariantChanged { enum_name: String, variant: String },
    ExistingFlagChanged { flags_name: String, bit: String },
}

/// Checks whether `new` can receive packets produced against `old`.
///
/// Existing constructors and fields remain positional and their Constructor IDs
/// are part of the wire contract, so changing them is rejected. New top-level
/// items, enum variants, and flag bits are safe for an old-to-new transition.
///
/// This is an opt-in schema migration audit. It is intentionally separate from
/// Layer negotiation: Layers remain independent artifacts and are never
/// implicitly treated as an inheritance chain.
pub fn is_backward_compatible(old: &Schema, new: &Schema) -> Result<(), CompatibilityError> {
    for old_item in &old.items {
        let name = item_name(old_item);
        let Some(new_item) = new.items.iter().find(|item| item_name(item) == name) else {
            continue;
        };
        match (old_item, new_item) {
            (Item::Struct(old), Item::Struct(new)) => {
                if struct_signature(old) != struct_signature(new) {
                    return Err(CompatibilityError::ExistingItemChanged { name });
                }
            }
            (Item::Enum(old), Item::Enum(new)) => check_enum(old, new)?,
            (Item::Flags(old), Item::Flags(new)) => check_flags(old, new)?,
            (Item::Alias(old), Item::Alias(new)) if old.ty == new.ty => {}
            _ => return Err(CompatibilityError::ItemKindChanged { name }),
        }
    }
    Ok(())
}

fn check_enum(old: &Enum, new: &Enum) -> Result<(), CompatibilityError> {
    for old_variant in &old.variants {
        let Some(new_variant) = new
            .variants
            .iter()
            .find(|variant| variant.name == old_variant.name)
        else {
            return Err(CompatibilityError::ExistingEnumVariantChanged {
                enum_name: old.name.clone(),
                variant: old_variant.name.clone(),
            });
        };
        if variant_signature(old_variant) != variant_signature(new_variant) {
            return Err(CompatibilityError::ExistingEnumVariantChanged {
                enum_name: old.name.clone(),
                variant: old_variant.name.clone(),
            });
        }
    }
    Ok(())
}

fn check_flags(old: &Flags, new: &Flags) -> Result<(), CompatibilityError> {
    if old.underlying != new.underlying {
        return Err(CompatibilityError::ExistingItemChanged {
            name: old.name.clone(),
        });
    }
    for old_bit in &old.bits {
        let Some(new_bit) = new.bits.iter().find(|bit| bit.name == old_bit.name) else {
            return Err(CompatibilityError::ExistingFlagChanged {
                flags_name: old.name.clone(),
                bit: old_bit.name.clone(),
            });
        };
        if old_bit.value != new_bit.value {
            return Err(CompatibilityError::ExistingFlagChanged {
                flags_name: old.name.clone(),
                bit: old_bit.name.clone(),
            });
        }
    }
    Ok(())
}

fn item_name(item: &Item) -> String {
    match item {
        Item::Alias(item) => item.name.clone(),
        Item::Struct(item) => item.name.clone(),
        Item::Enum(item) => item.name.clone(),
        Item::Flags(item) => item.name.clone(),
    }
}

fn struct_signature(item: &Struct) -> Vec<String> {
    item.fields.iter().map(field_signature).collect()
}

fn variant_signature(item: &EnumVariant) -> (Option<u64>, Option<String>, Vec<String>) {
    (
        item.value,
        item.cid.clone(),
        item.fields.iter().map(field_signature).collect(),
    )
}

fn field_signature(field: &Field) -> String {
    format!(
        "{}:{}:{}:{:?}",
        field.name,
        field.guard.as_deref().unwrap_or(""),
        type_signature(&field.ty),
        field.exact_len
    )
}

fn type_signature(ty: &Type) -> String {
    match ty {
        Type::Primitive(name) => name.clone(),
        Type::FixedBytes(length) => format!("bytes[{length}]"),
        Type::Optional(item) => format!("Optional<{}>", type_signature(item)),
        Type::Vec(item) => format!("Vec<{}>", type_signature(item)),
        Type::Map(key, value) => format!("Map<{},{}>", type_signature(key), type_signature(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_schema;

    #[test]
    fn allows_new_items_enum_variants_and_flag_bits() {
        let old = parse_schema(
            "#[version(1)] #[flags(u8)] enum Flags { Ready = 0, } enum Kind { Text { value: u8, }, } struct Message { flags: Flags, kind: Kind, }",
        )
        .unwrap();
        let new = parse_schema(
            "#[version(2)] #[flags(u8)] enum Flags { Ready = 0, Seen = 1, } enum Kind { Text { value: u8, }, Image { value: u8, }, } struct Message { flags: Flags, kind: Kind, } struct Added { id: u64, }",
        )
        .unwrap();
        assert_eq!(is_backward_compatible(&old, &new), Ok(()));
    }

    #[test]
    fn rejects_existing_field_changes() {
        let old = parse_schema("#[version(1)] struct Message { id: u64, }").unwrap();
        let new = parse_schema("#[version(2)] struct Message { id: u32, }").unwrap();
        assert!(matches!(
            is_backward_compatible(&old, &new),
            Err(CompatibilityError::ExistingItemChanged { .. })
        ));
    }

    #[test]
    fn rejects_existing_enum_variant_changes() {
        let old = parse_schema("#[version(1)] enum Kind { Text { value: String, }, }").unwrap();
        let new = parse_schema("#[version(2)] enum Kind { Text { value: Vec<u8>, }, }").unwrap();
        assert!(matches!(
            is_backward_compatible(&old, &new),
            Err(CompatibilityError::ExistingEnumVariantChanged { .. })
        ));
    }
}
