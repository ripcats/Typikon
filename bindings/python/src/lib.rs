use pyo3::prelude::*;

include!(concat!(env!("OUT_DIR"), "/python-bridge.rs"));

#[pyfunction]
fn abi_version() -> u16 {
    typikon::ffi_abi_version()
}

#[pyfunction]
fn negotiate_layer(requested: u16, supported: Vec<u16>) -> PyResult<u16> {
    typikon::LayerSupport::new(supported)
        .negotiate(requested)
        .map_err(|error| {
            pyo3::exceptions::PyValueError::new_err(format!("unsupported Layer {}", error.requested))
        })
}

#[pymodule]
fn typikon_python(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(abi_version, module)?)?;
    module.add_function(wrap_pyfunction!(negotiate_layer, module)?)?;
    register_typikon_python_10(module)?;
    Ok(())
}
