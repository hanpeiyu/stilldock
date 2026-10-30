use std::{
    io,
    path::{Path, PathBuf},
};

#[cfg(windows)]
fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    // Canonicalize the existing parent, preserving long-path/UNC support even
    // when the final file does not exist yet.
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("missing filename"))?;
    Ok(std::fs::canonicalize(parent)?
        .join(name)
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect())
}

#[cfg(windows)]
fn move_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    let source = wide_path(source)?;
    let destination = wide_path(destination)?;
    // MoveFileW, unlike std::fs::rename on Windows, never replaces a target.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileW(source.as_ptr(), destination.as_ptr())
    } != 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(windows))]
fn move_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::hard_link(source, destination)?;
    std::fs::remove_file(source)
}

pub fn publish(
    source: &Path,
    folder: &Path,
    stem: &str,
    extension: &str,
    nonce: &str,
) -> io::Result<PathBuf> {
    for attempt in 0..1000 {
        let suffix = match attempt {
            0 => String::new(),
            1 => format!("_{nonce}"),
            n => format!("_{nonce}_{n}"),
        };
        let destination = folder.join(format!("{stem}{suffix}.{extension}"));
        match move_no_replace(source, &destination) {
            Ok(()) => return Ok(destination),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists || destination.exists() => {
                continue
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("too many filename collisions"))
}

pub fn hide_cache(path: &Path) {
    #[cfg(windows)]
    if let Ok(path) = wide_path(path) {
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, INVALID_FILE_ATTRIBUTES,
        };
        unsafe {
            let flags = GetFileAttributesW(path.as_ptr());
            if flags != INVALID_FILE_ATTRIBUTES {
                let _ = SetFileAttributesW(path.as_ptr(), flags | FILE_ATTRIBUTE_HIDDEN);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_files_are_never_overwritten() {
        let root = std::env::temp_dir().join(format!("stilldock-publish-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("picture.jpg"), b"existing").unwrap();
        std::fs::write(root.join("picture_job.jpg"), b"also existing").unwrap();
        let part = root.join("incoming.part");
        std::fs::write(&part, b"new").unwrap();
        let target = publish(&part, &root, "picture", "jpg", "job").unwrap();
        assert_eq!(target.file_name().unwrap(), "picture_job_2.jpg");
        assert_eq!(
            std::fs::read(root.join("picture.jpg")).unwrap(),
            b"existing"
        );
        assert_eq!(
            std::fs::read(root.join("picture_job.jpg")).unwrap(),
            b"also existing"
        );
        assert_eq!(std::fs::read(target).unwrap(), b"new");
        assert!(!part.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
