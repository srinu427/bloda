use std::path::PathBuf;

use pyo3::{exceptions::PyException, prelude::*};

#[pyclass]
struct ArchiveReader{
    inner: bloda_sys::ArchiveReader
}

#[pymethods]
impl ArchiveReader{
    fn entry_type(&self, name: String) -> PyResult<Option<String>>{
        PyResult::Ok(self.inner.entry_type(&name))
    }

    fn list_all_entries(&self) -> PyResult<Vec<String>>{
        Ok(self.inner.list_all_entries())
    }

    fn list_entries_re(&self, re_pattern: String) -> PyResult<Vec<String>>{
        self.inner.list_entries_re(&re_pattern).map_err(PyException::new_err)
    }

    fn list_dir(&self, dir_name: String) -> PyResult<Vec<(String, String)>>{
        self.inner.list_dir(&dir_name).map_err(PyException::new_err)
    }

    fn extract_file(&self, name: String, output: PathBuf) -> PyResult<()>{
        self.inner.extract_file(&name, &output).map_err(PyException::new_err)
    }

    fn extract_files(&self, re_pattern: String, output_dir: PathBuf) -> PyResult<()>{
        self.inner.extract_files(&re_pattern, &output_dir, false).map_err(PyException::new_err)
    }
}

#[pyfunction]
fn open_archive(archive_path: PathBuf) -> PyResult<ArchiveReader> {
    bloda_sys::ArchiveReader::new(&archive_path, None)
        .map(|x| ArchiveReader {inner: x})
        .map_err(PyException::new_err)
}

#[pyfunction]
#[pyo3(signature = (input_dir, output_file_name, /, compression_type="ZSTD".to_string(), threads=1, block_size=None))]
fn create_archive(
    input_dir: PathBuf,
    output_file_name: PathBuf,
    compression_type: String,
    threads: u32,
    block_size: Option<u64>
) -> PyResult<()> {
    bloda_sys::create_archive(
        &input_dir,
        &output_file_name,
        &compression_type,
        threads as _,
        block_size
    )
        .map_err(PyException::new_err)
}

#[pyfunction]
fn decompress_archive(
    archive_path: PathBuf,
    output_dir: PathBuf,
) -> PyResult<()> {
    bloda_sys::decompress_archive(&archive_path, &output_dir)
        .map_err(PyException::new_err)
}

#[pymodule]
fn bloda_pyo3(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(open_archive, m)?)?;
    m.add_function(wrap_pyfunction!(create_archive, m)?)?;
    m.add_function(wrap_pyfunction!(decompress_archive, m)?)?;
    Ok(())
}
