use crate::{
    BridgeKind, ParseError, generate_bridge, generate_c_header, generate_go_binding,
    generate_public_schema, generate_python_binding, generate_rust, generate_typescript_binding,
    parse_schema,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedArtifacts {
    pub layer: u16,
    pub rust_file_name: String,
    pub public_schema_file_name: String,
    pub rust_source: String,
    pub public_schema: String,
    pub bridge_file_names: [String; 3],
    pub bridge_sources: [String; 3],
    pub bridge_header_name: String,
    pub bridge_header: String,
    pub go_file_name: String,
    pub go_source: String,
    pub python_file_name: String,
    pub python_source: String,
    pub typescript_file_name: String,
    pub typescript_source: String,
}

pub fn compile_schema(source: &str, schema_name: &str) -> Result<GeneratedArtifacts, ParseError> {
    let schema = parse_schema(source)?;
    let name = schema_name.strip_suffix(".typ").unwrap_or(schema_name);
    let name = name
        .strip_suffix(&format!("-{}", schema.version))
        .unwrap_or(name);
    let python_name = name.replace('-', "_");
    let rust_file_name = format!("{name}-{}.rs", schema.version);
    let bridge_file_names = BridgeKind::ALL.map(|kind| kind.file_name(name, schema.version));
    let bridge_sources =
        BridgeKind::ALL.map(|kind| generate_bridge(&schema, &rust_file_name, kind));
    Ok(GeneratedArtifacts {
        layer: schema.version,
        rust_file_name,
        public_schema_file_name: format!("{name}-{}.public.typ", schema.version),
        rust_source: generate_rust(&schema),
        public_schema: generate_public_schema(&schema),
        bridge_file_names,
        bridge_sources,
        bridge_header_name: format!("{}-{}.h", name, schema.version),
        bridge_header: generate_c_header(&schema),
        go_file_name: format!("{}-{}.go", name, schema.version),
        go_source: generate_go_binding(&schema, &format!("{}-{}.h", name, schema.version)),
        // Python module filenames cannot contain the '-' used by the other
        // artifact names: `chat-10.py` is not importable as `chat_10`.
        python_file_name: format!("{}_{}.py", python_name, schema.version),
        python_source: generate_python_binding(&schema),
        typescript_file_name: format!("{}-{}.ts", name, schema.version),
        typescript_source: generate_typescript_binding(&schema),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_artifact_names_from_schema_version() {
        let artifacts =
            compile_schema("#[version(18)] struct User { id: u64, }", "valty.typ").unwrap();
        assert_eq!(artifacts.layer, 18);
        assert_eq!(artifacts.rust_file_name, "valty-18.rs");
        assert_eq!(artifacts.public_schema_file_name, "valty-18.public.typ");
        assert!(artifacts.rust_source.contains("pub struct User"));
        assert!(artifacts.public_schema.starts_with("#[version(18)]"));
    }

    #[test]
    fn does_not_duplicate_layer_already_in_schema_filename() {
        let artifacts =
            compile_schema("#[version(8)] struct User { id: u64, }", "layer-8.typ").unwrap();
        assert_eq!(artifacts.rust_file_name, "layer-8.rs");
        assert_eq!(artifacts.public_schema_file_name, "layer-8.public.typ");
        assert_eq!(artifacts.bridge_file_names[0], "python.layer-8.rs");
        assert_eq!(artifacts.go_file_name, "layer-8.go");
        assert_eq!(artifacts.python_file_name, "layer_8.py");
        assert_eq!(artifacts.typescript_file_name, "layer-8.ts");
    }
}
