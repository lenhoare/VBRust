// Inline Python — drop into real CPython for a bit, get a plain value back.
// The block runs via pyo3; its last line is the value, extracted into the
// type you annotate with `As`. (Slice 1: scalars in, scalars out.)

fn main() {
    // A one-liner reaching a library VBR doesn't have.
    let mean: f64 = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<f64> {
            let ns = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(r#"
import numpy as np
_vbr_result = np.array([1, 2, 3, 4]).mean()
"#).unwrap(), Some(&ns), Some(&ns))?;
            ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .extract()
        })
        .expect("the Python block raised an exception")
    };
    println!("{}", format!("{}{}", "mean is ", mean));
    // A multi-line block — real Python, sealed inside; last line is the value.
    let greeting: String = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<String> {
            let ns = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(r#"
name = "world"
_vbr_result = f"hello, {name}"
"#).unwrap(), Some(&ns), Some(&ns))?;
            ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .extract()
        })
        .expect("the Python block raised an exception")
    };
    println!("{}", greeting);
    let answer: i32 = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<i32> {
            let ns = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(r#"
_vbr_result = 6 * 7
"#).unwrap(), Some(&ns), Some(&ns))?;
            ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .extract()
        })
        .expect("the Python block raised an exception")
    };
    println!("{}", format!("{}{}", "the answer is ", answer));
}
