// Inline Python handles — hold a Python object VBR has no type for, and pass it
// back into later blocks. Each block is its own GIL scope. (Slice 2.)

fn main() {
    // Load once: a numpy array. No `As` type — it's held as an opaque PyObject.
    let data: pyo3::Py<pyo3::PyAny> = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
            let ns = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(r#"
import numpy as np
_vbr_result = np.array([3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0])
"#).unwrap(), Some(&ns), Some(&ns))?;
            Ok(ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .unbind())
        })
        .expect("the Python block raised an exception")
    };
    // Query it repeatedly — the handle goes back in via `Python(data)`.
    let mean: f64 = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<f64> {
            let ns = pyo3::types::PyDict::new(py);
            ns.set_item("data", &data)?;
            py.run(&std::ffi::CString::new(r#"
_vbr_result = float(data.mean())
"#).unwrap(), Some(&ns), Some(&ns))?;
            ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .extract()
        })
        .expect("the Python block raised an exception")
    };
    println!("{}", format!("{}{}", "mean = ", mean));
    let biggest: f64 = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<f64> {
            let ns = pyo3::types::PyDict::new(py);
            ns.set_item("data", &data)?;
            py.run(&std::ffi::CString::new(r#"
_vbr_result = float(data.max())
"#).unwrap(), Some(&ns), Some(&ns))?;
            ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .extract()
        })
        .expect("the Python block raised an exception")
    };
    println!("{}", format!("{}{}", "max  = ", biggest));
    // Pass a scalar in alongside the handle: how many exceed a threshold?
    let threshold: f64 = 4.0;
    let above: i32 = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<i32> {
            let ns = pyo3::types::PyDict::new(py);
            ns.set_item("data", &data)?;
            ns.set_item("threshold", &threshold)?;
            py.run(&std::ffi::CString::new(r#"
_vbr_result = int((data > threshold).sum())
"#).unwrap(), Some(&ns), Some(&ns))?;
            ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .extract()
        })
        .expect("the Python block raised an exception")
    };
    println!("{}", format!("{}{}", format!("{}{}", above, " values exceed "), threshold));
}
