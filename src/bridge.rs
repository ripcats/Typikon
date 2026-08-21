use crate::codegen::borrowed_view_name;
use crate::{Item, Schema, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeKind {
    Python,
    Go,
    TypeScript,
}

impl BridgeKind {
    pub const ALL: [Self; 3] = [Self::Python, Self::Go, Self::TypeScript];

    pub fn file_name(self, schema_name: &str, layer: u16) -> String {
        let name = match self {
            Self::Python => "python",
            Self::Go => "golang",
            Self::TypeScript => "typescript",
        };
        format!("{name}.{schema_name}-{layer}.rs")
    }
}

pub fn generate_c_header(schema: &Schema) -> String {
    let mut output = String::from(
        "#ifndef TYPIKON_LAYER_BRIDGE_H\n#define TYPIKON_LAYER_BRIDGE_H\n#include <stddef.h>\n#include <stdint.h>\n\ntypedef struct { int32_t status; uint8_t *data_ptr; size_t data_len; size_t data_capacity; uint8_t *error_ptr; size_t error_len; size_t error_capacity; } TypikonBridgeResult;\n\nvoid typikon_free_bytes(uint8_t *ptr, size_t len, size_t capacity);\n\n",
    );
    for item in &schema.items {
        let name = match item {
            Item::Struct(item) => &item.name,
            Item::Enum(item) => &item.name,
            Item::Flags(item) => &item.name,
        };
        let function_name = snake_case(name);
        output.push_str(&format!(
            "int32_t typikon_{}_{}_validate_borrowed(const uint8_t *input, size_t len);\n",
            schema.version, function_name
        ));
    }
    output.push_str("\n#endif\n");
    output
}

pub fn generate_go_binding(schema: &Schema, header_name: &str) -> String {
    let mut output = format!(
        "package typikon\n\n/*\n#cgo CFLAGS: -I.\n#include \"{header_name}\"\n*/\nimport \"C\"\n\nimport (\n    \"encoding/json\"\n    \"fmt\"\n    \"unsafe\"\n)\n\nfunc bridgeResult(result C.TypikonBridgeResult) ([]byte, error) {{\n    defer C.typikon_free_bytes(result.data_ptr, result.data_len, result.data_capacity)\n    defer C.typikon_free_bytes(result.error_ptr, result.error_len, result.error_capacity)\n    if result.status != 0 {{ return nil, fmt.Errorf(\"native bridge error: %s\", string(C.GoBytes(unsafe.Pointer(result.error_ptr), C.int(result.error_len)))) }}\n    return C.GoBytes(unsafe.Pointer(result.data_ptr), C.int(result.data_len)), nil\n}}\nfunc bridgePtr(data []byte) *C.uint8_t {{ if len(data) == 0 {{ return nil }}; return (*C.uint8_t)(unsafe.Pointer(&data[0])) }}\n\n"
    );
    for item in &schema.items {
        let name = item_name(item);
        let function_name = snake_case(name);
        if let Item::Struct(item) = item {
            output.push_str(&format!("type {name} struct {{\n"));
            for field in &item.fields {
                output.push_str(&format!(
                    "    {} {} `json:\"{}\"`\n",
                    pascal_case(&field.name),
                    if field.guard.is_some() {
                        format!("*{}", go_type(&field.ty, schema))
                    } else {
                        go_type(&field.ty, schema)
                    },
                    field.name
                ));
            }
            output.push_str("}\n\n");
        } else {
            output.push_str(&format!("{}\n\n", go_item_type(item)));
        }
        output.push_str(&format!(
            "func Encode{}(value {}) ([]byte, error) {{ input, err := json.Marshal(value); if err != nil {{ return nil, err }}; result := C.typikon_{}_{}_encode_json(bridgePtr(input), C.size_t(len(input))); return bridgeResult(result) }}\nfunc Decode{}(wire []byte) ({}, error) {{ var value {}; result := C.typikon_{}_{}_decode_json(bridgePtr(wire), C.size_t(len(wire))); data, err := bridgeResult(result); if err != nil {{ return value, err }}; err = json.Unmarshal(data, &value); return value, err }}\n\n",
            name, name, schema.version, function_name, name, name, name, schema.version, function_name
        ));
        output.push_str(&format!(
            "func Validate{}(wire []byte) error {{ if C.typikon_{}_{}_validate_borrowed(bridgePtr(wire), C.size_t(len(wire))) != 0 {{ return fmt.Errorf(\"invalid {} wire\") }}; return nil }}\n\n",
            name, schema.version, function_name, name
        ));
    }
    output
}

pub fn generate_typescript_binding(schema: &Schema) -> String {
    let mut output = String::from(
        "export interface TypikonNative { encodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array; decodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array; validateBinary(layer: number, typeName: string, input: Uint8Array): void; }\n\n",
    );
    for item in &schema.items {
        let name = item_name(item);
        if let Item::Struct(item) = item {
            output.push_str(&format!("export interface {name} {{\n"));
            for field in &item.fields {
                let optional = if field.guard.is_some() { "?" } else { "" };
                output.push_str(&format!(
                    "  {}{}: {};\n",
                    field.name,
                    optional,
                    typescript_type(&field.ty, schema)
                ));
            }
            output.push_str("}\n\n");
        } else {
            output.push_str(&format!("{}\n\n", typescript_item_type(item, schema)));
        }
        let function_name = name.to_ascii_lowercase();
        output.push_str(&format!("export function encodeBinary{name}(native: TypikonNative, wire: Uint8Array): Uint8Array {{ return native.encodeBinary({}, \"{}\", wire); }}\nexport function decodeBinary{name}(native: TypikonNative, wire: Uint8Array): Uint8Array {{ return native.decodeBinary({}, \"{}\", wire); }}\nexport function validateBinary{name}(native: TypikonNative, wire: Uint8Array): void {{ native.validateBinary({}, \"{}\", wire); }}\n\n", schema.version, function_name, schema.version, function_name, schema.version, function_name));
    }
    output
}

pub fn generate_python_binding(schema: &Schema) -> String {
    let mut output = format!(
        "# @generated by typikon; Python facade for Layer {}.\nfrom __future__ import annotations\n\nfrom typing import Any\n\n",
        schema.version
    );
    output.push_str("from typikon_python import ");
    output.push_str(
        &schema
            .items
            .iter()
            .map(|item| {
                let function_name = snake_case(item_name(item));
                format!(
                    "encode_{function_name} as _native_encode_{function_name}, decode_{function_name} as _native_decode_{function_name}, validate_borrowed_{function_name} as _native_validate_borrowed_{function_name}"
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
    );
    output.push_str(&format!("\n\nLAYER = {}\n\n", schema.version));
    for item in &schema.items {
        let function_name = snake_case(item_name(item));
        output.push_str(&format!(
            "def encode_{function_name}(value: Any) -> bytes:\n    return _native_encode_{function_name}(value)\n\ndef decode_{function_name}(wire: bytes) -> Any:\n    return _native_decode_{function_name}(wire)\n\ndef validate_borrowed_{function_name}(wire: bytes) -> None:\n    _native_validate_borrowed_{function_name}(wire)\n\n"
        ));
    }
    output.push_str("__all__ = [\"LAYER\"");
    for item in &schema.items {
        let name = item_name(item);
        let function_name = snake_case(name);
        output.push_str(&format!(
            ", \"{name}\", \"encode_{function_name}\", \"decode_{function_name}\", \"validate_borrowed_{function_name}\""
        ));
    }
    output.push_str("]\n");
    output
}

fn item_name(item: &Item) -> &str {
    match item {
        Item::Struct(item) => &item.name,
        Item::Enum(item) => &item.name,
        Item::Flags(item) => &item.name,
    }
}

fn go_item_type(item: &Item) -> String {
    match item {
        Item::Flags(item) => format!("type {} {}", item.name, go_primitive(&item.underlying)),
        Item::Enum(item)
            if item
                .variants
                .iter()
                .all(|variant| variant.fields.is_empty()) =>
        {
            format!("type {} string", item.name)
        }
        Item::Enum(item)
            if item
                .variants
                .iter()
                .all(|variant| !variant.fields.is_empty()) =>
        {
            format!("type {} map[string]json.RawMessage", item.name)
        }
        Item::Enum(item) => format!("type {} any", item.name),
        Item::Struct(_) => unreachable!("structs are emitted separately"),
    }
}

fn go_primitive(name: &str) -> &str {
    match name {
        "u8" => "uint8",
        "u16" => "uint16",
        "u32" => "uint32",
        "u64" => "uint64",
        "u128" => "string",
        "i8" => "int8",
        "i16" => "int16",
        "i32" => "int32",
        "i64" => "int64",
        "i128" => "string",
        "bool" => "bool",
        "f32" => "float32",
        "f64" => "float64",
        "String" => "string",
        _ => "json.RawMessage",
    }
}

fn typescript_item_type(item: &Item, schema: &Schema) -> String {
    match item {
        Item::Flags(item) => format!("export type {} = number;", item.name),
        Item::Enum(item)
            if item
                .variants
                .iter()
                .all(|variant| variant.fields.is_empty()) =>
        {
            let variants = item
                .variants
                .iter()
                .map(|variant| format!("\"{}\"", variant.name))
                .collect::<Vec<_>>()
                .join(" | ");
            format!("export type {} = {};", item.name, variants)
        }
        Item::Enum(item) => {
            let variants = item
                .variants
                .iter()
                .map(|variant| {
                    if variant.fields.is_empty() {
                        format!("\"{}\"", variant.name)
                    } else {
                        let fields = variant
                            .fields
                            .iter()
                            .map(|field| {
                                format!("{}: {}", field.name, typescript_type(&field.ty, schema))
                            })
                            .collect::<Vec<_>>()
                            .join("; ");
                        format!("{{ {}: {{ {} }} }}", variant.name, fields)
                    }
                })
                .collect::<Vec<_>>()
                .join(" | ");
            format!("export type {} = {};", item.name, variants)
        }
        Item::Struct(_) => unreachable!("structs are emitted separately"),
    }
}

fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn go_type(ty: &Type, schema: &Schema) -> String {
    match ty {
        Type::Primitive(name) => match name.as_str() {
            "String" => "string".into(),
            "bool" => "bool".into(),
            "f32" => "float32".into(),
            "f64" => "float64".into(),
            "u8" => "uint8".into(),
            "u16" => "uint16".into(),
            "u32" => "uint32".into(),
            "u64" => "uint64".into(),
            "u128" => "string".into(),
            "i8" => "int8".into(),
            "i16" => "int16".into(),
            "i32" => "int32".into(),
            "i64" => "int64".into(),
            "i128" => "string".into(),
            _ if schema.items.iter().any(|item| item_name(item) == name) => name.clone(),
            _ => "json.RawMessage".into(),
        },
        Type::Vec(item) => format!("[]{}", go_type(item, schema)),
        Type::Map(_, value) => format!("map[string]{}", go_type(value, schema)),
    }
}

fn typescript_type(ty: &Type, schema: &Schema) -> String {
    match ty {
        Type::Primitive(name) => match name.as_str() {
            "String" => "string".into(),
            "bool" => "boolean".into(),
            n if n.starts_with('f') => "number".into(),
            n if n.starts_with('u') || n.starts_with('i') => "number".into(),
            _ if schema.items.iter().any(|item| item_name(item) == name) => name.clone(),
            _ => "unknown".into(),
        },
        Type::Vec(item) => format!("Array<{}>", typescript_type(item, schema)),
        Type::Map(_, value) => format!("Record<string, {}>", typescript_type(value, schema)),
    }
}

pub fn generate_bridge(schema: &Schema, native_file: &str, kind: BridgeKind) -> String {
    let layer = schema.version;
    let module = native_file.trim_end_matches(".rs").replace(['-', '.'], "_");
    let language = match kind {
        BridgeKind::Python => "Python / PyO3",
        BridgeKind::Go => "Go / cgo",
        BridgeKind::TypeScript => "TypeScript / Node-API",
    };
    if matches!(kind, BridgeKind::Go) {
        return generate_go_validation_bridge(schema, native_file, layer, &module, language);
    }
    let mut output = if matches!(kind, BridgeKind::Python) {
        format!(
            "// @generated by typikon; {language} bridge; do not edit.\n\n#[allow(dead_code)]\n#[path = \"{native_file}\"]\nmod {module};\n\n// Layer {layer} uses the native generated module above for all wire operations.\npub const TYPIKON_LAYER: u16 = {layer};\n"
        )
    } else {
        format!(
            "// @generated by typikon; {language} bridge; do not edit.\n\nuse std::slice;\n\n#[allow(dead_code)]\n#[path = \"{native_file}\"]\nmod {module};\n\n#[repr(C)]\npub struct TypikonBridgeResult {{ pub status: i32, pub data_ptr: *mut u8, pub data_len: usize, pub data_capacity: usize, pub error_ptr: *mut u8, pub error_len: usize, pub error_capacity: usize }}\nimpl TypikonBridgeResult {{ fn success(data: Vec<u8>) -> Self {{ let (data_ptr, data_len, data_capacity) = data.into_raw_parts(); Self {{ status: 0, data_ptr, data_len, data_capacity, error_ptr: std::ptr::null_mut(), error_len: 0, error_capacity: 0 }} }} fn failure(error: String) -> Self {{ let (error_ptr, error_len, error_capacity) = error.into_bytes().into_raw_parts(); Self {{ status: 1, data_ptr: std::ptr::null_mut(), data_len: 0, data_capacity: 0, error_ptr, error_len, error_capacity }} }} }}\n\npub const TYPIKON_LAYER: u16 = {layer};\n"
        )
    };
    if !matches!(kind, BridgeKind::Python) {
        output.push_str("fn input_bytes(input: *const u8, len: usize) -> Result<&'static [u8], String> { if len > 0 && input.is_null() { return Err(\"null input\".into()); } if len == 0 { Ok(&[]) } else { Ok(unsafe { slice::from_raw_parts(input, len) }) } }\n");
    }
    if !matches!(kind, BridgeKind::Python) {
        for item in &schema.items {
            let (name, is_flags) = match item {
                Item::Struct(item) => (&item.name, false),
                Item::Enum(item) => (&item.name, false),
                Item::Flags(item) => (&item.name, true),
            };
            let function_name = snake_case(name);
            let native_name = format!("{module}::{name}");
            if is_flags {
                output.push_str(&format!(
            "pub fn encode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
                output.push_str(&format!(
            "pub fn decode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
            } else {
                output.push_str(&format!(
            "pub fn encode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
                output.push_str(&format!(
            "pub fn decode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
            }
            let borrowed = if let Some(view_name) = borrowed_view_name(item, schema) {
                format!(
                    "let _: {module}::{view_name}<'_> = typikon::decode_borrowed_value(bytes).map_err(|error| format!(\"{{error:?}}\"))?;"
                )
            } else {
                format!(
                    "let mut decoder = typikon::Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }}"
                )
            };
            output.push_str(&format!(
                "pub fn validate_borrowed_{function_name}(input: &[u8]) -> Result<(), String> {{ let bytes = input; {borrowed} Ok(()) }}\n#[unsafe(no_mangle)]\npub extern \"C\" fn typikon_{layer}_{function_name}_validate_borrowed(input: *const u8, len: usize) -> i32 {{ let Ok(bytes) = input_bytes(input, len) else {{ return 1 }}; validate_borrowed_{function_name}(bytes).map(|_| 0).unwrap_or(1) }}\n"
            ));
        }
    }
    if matches!(kind, BridgeKind::Python) {
        output.push_str("use pyo3::exceptions::PyValueError;\n");
        for item in &schema.items {
            let name = item_name(item);
            let function_name = snake_case(name);
            let native_name = format!("{module}::{name}");
            let borrowed_name = borrowed_view_name(item, schema)
                .map(|view| format!("{module}::{view}<'_>"))
                .unwrap_or_else(|| native_name.clone());
            let (encode_body, decode_body) = if matches!(item, Item::Flags(_)) {
                (
                    "let mut encoder = typikon::Encoder::new(typikon::DEFAULT_MAX_PACKET_SIZE); typikon::WireCodec::encode(&value, &mut encoder).map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))?; encoder.finish().map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))".to_owned(),
                    format!("let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?; let value: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?; if !decoder.is_finished() {{ return Err(PyValueError::new_err(\"trailing bytes\")); }}"),
                )
            } else {
                (
                    "typikon::TypikonCodec::encode(&value).map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))".to_owned(),
                    format!("let value: {native_name} = typikon::TypikonCodec::decode(input).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?;"),
                )
            };
            output.push_str(&format!(
                "#[pyfunction]\nfn encode_{function_name}(value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {{ let value: {native_name} = pythonize::depythonize(value).map_err(|error| PyValueError::new_err(error.to_string()))?; {encode_body} }}\n#[pyfunction]\nfn decode_{function_name}(py: Python<'_>, input: &[u8]) -> PyResult<Py<PyAny>> {{ {decode_body} pythonize::pythonize(py, &value).map(|value| value.unbind()).map_err(|error| PyValueError::new_err(error.to_string())) }}\n#[pyfunction]\nfn validate_borrowed_{function_name}(input: &[u8]) -> PyResult<()> {{ typikon::decode_borrowed_value::<{borrowed_name}>(input).map(|_| ()).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\"))) }}\n"
            ));
        }
        output.push_str(&format!("pub fn register_typikon_python_{layer}(module: &Bound<'_, PyModule>) -> PyResult<()> {{\n"));
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!("    module.add_function(wrap_pyfunction!(encode_{function_name}, module)?)?;\n    module.add_function(wrap_pyfunction!(decode_{function_name}, module)?)?;\n    module.add_function(wrap_pyfunction!(validate_borrowed_{function_name}, module)?)?;\n"));
        }
        output.push_str("    Ok(())\n}\n");
    }
    if matches!(kind, BridgeKind::TypeScript) {
        output.push_str(
            "use napi::bindgen_prelude::Buffer;\nuse napi_derive::napi;\n\n#[napi]\npub fn abi_version() -> u16 { typikon::ffi_abi_version() }\n\n#[napi]\npub fn negotiate_layer(requested: u16, supported: Vec<u16>) -> napi::Result<u16> { typikon::LayerSupport::new(supported).negotiate(requested).map_err(|error| napi::Error::from_reason(format!(\"unsupported Layer {}\", error.requested))) }\n\nfn check_layer(layer: u16) -> napi::Result<()> { if layer == TYPIKON_LAYER { Ok(()) } else { Err(napi::Error::from_reason(format!(\"unsupported Layer {}\", layer))) } }\n\n#[napi]\npub fn encode_binary(layer: u16, type_name: String, input: Buffer) -> napi::Result<Buffer> { check_layer(layer)?; match type_name.as_str() {\n",
        );
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!(
                "        \"{}\" => encode_binary_{}(&input).map(Buffer::from).map_err(napi::Error::from_reason),\n",
                function_name, function_name
            ));
        }
        output.push_str("        _ => Err(napi::Error::from_reason(\"unknown schema type\")), } }\n\n#[napi]\npub fn decode_binary(layer: u16, type_name: String, input: Buffer) -> napi::Result<Buffer> { check_layer(layer)?; match type_name.as_str() {\n");
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!(
                "        \"{}\" => decode_binary_{}(&input).map(Buffer::from).map_err(napi::Error::from_reason),\n",
                function_name, function_name
            ));
        }
        output
            .push_str("        _ => Err(napi::Error::from_reason(\"unknown schema type\")), } }\n");
        output.push_str("\n#[napi]\npub fn validate_binary(layer: u16, type_name: String, input: Buffer) -> napi::Result<()> { check_layer(layer)?; match type_name.as_str() {\n");
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!(
                "        \"{}\" => validate_borrowed_{}(&input).map_err(|error| napi::Error::from_reason(format!(\"{{error:?}}\"))),\n",
                function_name, function_name
            ));
        }
        output
            .push_str("        _ => Err(napi::Error::from_reason(\"unknown schema type\")), } }\n");
    }
    output
}

fn generate_go_validation_bridge(
    schema: &Schema,
    native_file: &str,
    layer: u16,
    module: &str,
    language: &str,
) -> String {
    let mut output = format!(
        "// @generated by typikon; {language} bridge; do not edit.\n\nuse std::slice;\n\n#[allow(dead_code)]\n#[path = \"{native_file}\"]\nmod {module};\n\n#[repr(C)]\npub struct TypikonBridgeResult {{ pub status: i32, pub data_ptr: *mut u8, pub data_len: usize, pub data_capacity: usize, pub error_ptr: *mut u8, pub error_len: usize, pub error_capacity: usize }}\n\n pub const TYPIKON_LAYER: u16 = {layer};\nfn input_bytes(input: *const u8, len: usize) -> Result<&'static [u8], String> {{ if len > 0 && input.is_null() {{ return Err(\"null input\".into()); }} if len == 0 {{ Ok(&[]) }} else {{ Ok(unsafe {{ slice::from_raw_parts(input, len) }}) }} }}\n",
    );
    for item in &schema.items {
        let name = item_name(item);
        let function_name = snake_case(name);
        let borrowed = if let Some(view_name) = borrowed_view_name(item, schema) {
            format!(
                "let _: {module}::{view_name}<'_> = typikon::decode_borrowed_value(bytes).map_err(|error| format!(\"{{error:?}}\"))?;"
            )
        } else {
            let native_name = format!("{module}::{name}");
            format!(
                "let mut decoder = typikon::Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }}"
            )
        };
        output.push_str(&format!("pub fn validate_borrowed_{function_name}(input: &[u8]) -> Result<(), String> {{ let bytes = input; {borrowed} Ok(()) }}\n#[unsafe(no_mangle)]\npub extern \"C\" fn typikon_{layer}_{function_name}_validate_borrowed(input: *const u8, len: usize) -> i32 {{ let Ok(bytes) = input_bytes(input, len) else {{ return 1 }}; validate_borrowed_{function_name}(bytes).map(|_| 0).unwrap_or(1) }}\n"));
    }
    output
}

fn snake_case(name: &str) -> String {
    name.chars()
        .enumerate()
        .map(|(index, character)| {
            if character.is_ascii_uppercase() && index > 0 {
                format!("_{}", character.to_ascii_lowercase())
            } else {
                character.to_ascii_lowercase().to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_schema;

    #[test]
    fn bridges_reference_the_same_native_backend() {
        let schema = parse_schema("#[version(10)] struct Message { id: u64, }").unwrap();
        for kind in BridgeKind::ALL {
            let source = generate_bridge(&schema, "chat-10.rs", kind);
            assert!(source.contains("mod chat_10"));
            assert!(source.contains("TYPIKON_LAYER: u16 = 10"));
            if matches!(kind, BridgeKind::Python) {
                assert!(source.contains("fn encode_message"));
            } else {
                assert!(source.contains("validate_borrowed_message"));
                assert!(!source.contains("serde_json"));
            }
        }
        assert_eq!(
            BridgeKind::Python.file_name("chat", 10),
            "python.chat-10.rs"
        );
    }

    #[test]
    fn go_binding_uses_real_go_integer_types() {
        let schema =
            parse_schema("#[version(10)] struct Message { small: u8, id: u64, signed: i32, }")
                .unwrap();
        let source = generate_go_binding(&schema, "typikon-10.h");
        assert!(source.contains("Small uint8"));
        assert!(source.contains("Id uint64"));
        assert!(source.contains("Signed int32"));
        assert!(!source.contains(" Small u8"));
        assert!(!source.contains(" Id u64"));
    }

    #[test]
    fn generated_bridges_expose_owned_error_and_data_buffers() {
        let schema = parse_schema("#[version(10)] struct Message { id: u64, }").unwrap();
        let source = generate_bridge(&schema, "chat-10.rs", BridgeKind::Go);
        assert!(source.contains("data_capacity"));
        assert!(source.contains("error_capacity"));
        assert!(source.contains("typikon_10_message_validate_borrowed"));
    }

    #[test]
    fn typescript_bridge_has_node_api_dispatch_for_every_item() {
        let schema = parse_schema(
            "#[version(10)] struct User { id: u64, } enum Event { Created { user: User }, } #[flags(u16)] enum Flags { Ready = 0, }",
        )
        .unwrap();
        let source = generate_bridge(&schema, "chat-10.rs", BridgeKind::TypeScript);
        assert!(source.contains("#[napi]"));
        assert!(source.contains("\"user\" => encode_binary_user"));
        assert!(source.contains("\"event\" => decode_binary_event"));
        assert!(source.contains("\"flags\" => encode_binary_flags"));
        assert!(!source.contains("pub fn encode_json"));
        assert!(!source.contains("pub fn decode_json"));
    }

    #[test]
    fn generated_language_types_preserve_flags_and_enums() {
        let schema = parse_schema(
            "#[version(10)] #[flags(u16)] enum Flags { Ready = 0, } enum Event { Created { id: u64 }, }",
        )
        .unwrap();
        let go = generate_go_binding(&schema, "chat-10.h");
        assert!(go.contains("type Flags uint16"));
        assert!(go.contains("type Event map[string]json.RawMessage"));
        assert!(!go.contains("type Flags json.RawMessage"));
        let typescript = generate_typescript_binding(&schema);
        assert!(typescript.contains("export type Flags = number;"));
        assert!(typescript.contains("export type Event = { Created:"));
    }

    #[test]
    fn python_facade_wraps_the_pyo3_extension() {
        let schema = parse_schema("#[version(10)] struct User { id: u64, }").unwrap();
        let source = generate_python_binding(&schema);
        assert!(source.contains("from typikon_python import encode_user as _native_encode_user"));
        assert!(source.contains("def encode_user"));
        assert!(source.contains("def decode_user"));
        assert!(source.contains("return _native_encode_user(value)"));
        assert!(source.contains("LAYER = 10"));
    }
}
