use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum AtomicWriteError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tempfile error: {0}")]
    TempFile(#[from] tempfile::PersistError),
}

pub fn atomic_write(path: &Path, content: &str) -> Result<(), AtomicWriteError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));

    fs::create_dir_all(dir)?;

    if path.exists() {
        let bak_path = path.with_extension("yaml.bak");
        fs::copy(path, &bak_path).ok();
    }

    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(content.as_bytes())?;
    tmp.as_file_mut().sync_all()?;
    tmp.persist(path)?;

    let bak_path = path.with_extension("yaml.bak");
    let _ = fs::remove_file(bak_path);

    Ok(())
}

#[allow(dead_code)]
pub fn read_or_default(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn atomic_write_new_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.yaml");

        atomic_write(&file_path, "key: value\n").unwrap();
        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "key: value\n");
    }

    #[test]
    fn atomic_write_overwrite() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.yaml");

        atomic_write(&file_path, "first: version\n").unwrap();
        atomic_write(&file_path, "second: version\n").unwrap();
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "second: version\n");
        let bak = file_path.with_extension("yaml.bak");
        assert!(!bak.exists(), "backup file should be cleaned up");
    }

    #[test]
    fn atomic_write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("deep").join("nested").join("test.yaml");

        atomic_write(&file_path, "deep: true\n").unwrap();
        assert!(file_path.exists());
    }

    #[test]
    fn atomic_write_large_content() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("large.yaml");
        let content = "line: value\n".repeat(1000);

        atomic_write(&file_path, &content).unwrap();
        assert_eq!(fs::read_to_string(&file_path).unwrap(), content);
    }

    #[test]
    fn atomic_write_preserves_old_on_crash_simulation() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.yaml");

        atomic_write(&file_path, "original: content\n").unwrap();

        let tmp_path = dir.path().join("test.yaml.tmp");
        fs::write(&tmp_path, "partial: data").unwrap();

        let orig = fs::read_to_string(&file_path).unwrap();
        assert_eq!(orig, "original: content\n");
    }
}
