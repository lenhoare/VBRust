// Tuple return — pull SEVERAL values out of one Python block, in a single GIL
// scope. The natural shape for "a name AND its data" from a model, a dataframe,
// a query result… write the results as a comma-separated tuple on the last line,
// and destructure them into typed VBR bindings. (Slice 3.)

fn main() {
    // One block, three results: a label (String), its data (Vec<Double>), and a
    // summary stat (Double) — extracted together without touching the object twice.
    let (name, weights, total): (String, Vec<f64>, f64) = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<(String, Vec<f64>, f64)> {
            let ns = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(r#"
import numpy as np
w = np.array([0.5, 1.5, 2.0, 3.0])
_vbr_result = "layer.weight", w.tolist(), float(w.sum())
"#).unwrap(), Some(&ns), Some(&ns))?;
            ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .extract()
        })
        .expect("the Python block raised an exception")
    };
    println!("tensor: {}", name);
    println!("sum:    {}", total);
    println!("first:  {}", weights[0]);
    // Works with a handle passed in too: destructure stats out of a held object.
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
    let (lo, hi, mean): (f64, f64, f64) = {
        use pyo3::prelude::*;
        pyo3::Python::with_gil(|py| -> pyo3::PyResult<(f64, f64, f64)> {
            let ns = pyo3::types::PyDict::new(py);
            ns.set_item("data", &data)?;
            py.run(&std::ffi::CString::new(r#"
_vbr_result = float(data.min()), float(data.max()), float(data.mean())
"#).unwrap(), Some(&ns), Some(&ns))?;
            ns.get_item("_vbr_result")?
                .expect("the Python block produced no value on its last line")
                .extract()
        })
        .expect("the Python block raised an exception")
    };
    println!("range:  {} .. {} (mean {})", lo, hi, mean);
}
