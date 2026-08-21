use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerSupport {
    supported: BTreeSet<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerVersionNotSupported {
    pub requested: u16,
}

impl LayerSupport {
    pub fn new(layers: impl IntoIterator<Item = u16>) -> Self {
        Self {
            supported: layers.into_iter().collect(),
        }
    }
    pub fn supports(&self, layer: u16) -> bool {
        self.supported.contains(&layer)
    }
    pub fn negotiate(&self, requested: u16) -> Result<u16, LayerVersionNotSupported> {
        if self.supports(requested) {
            Ok(requested)
        } else {
            Err(LayerVersionNotSupported { requested })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiates_only_compiled_layers() {
        let support = LayerSupport::new([6, 8, 10]);
        assert_eq!(support.negotiate(8), Ok(8));
        assert_eq!(
            support.negotiate(9),
            Err(LayerVersionNotSupported { requested: 9 })
        );
        assert!(!support.supports(7));
    }

    #[test]
    fn compatibility_fixtures_match_their_layers() {
        for (source, layer) in [
            (include_str!("../fixtures/layers/layer-6.typ"), 6u16),
            (include_str!("../fixtures/layers/layer-8.typ"), 8u16),
            (include_str!("../fixtures/layers/layer-10.typ"), 10u16),
        ] {
            let schema = crate::parse_schema(source).unwrap();
            assert_eq!(schema.version, layer);
        }
    }
}
