use crate::persistence::ResolvedFileSource;
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum LaunchError {
    Io(std::io::Error),
    SourceUnavailable(String),
    BoundaryViolation(String),
    Platform(String),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Source I/O error: {error}"),
            Self::SourceUnavailable(message) => write!(f, "{message}"),
            Self::BoundaryViolation(message) => write!(f, "{message}"),
            Self::Platform(message) => write!(f, "{message}"),
        }
    }
}

impl Error for LaunchError {}

impl From<std::io::Error> for LaunchError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn open_source(source: &ResolvedFileSource) -> Result<(), LaunchError> {
    let path = validated_existing_path(source)?;
    platform_open(&path)
}

pub fn reveal_source(source: &ResolvedFileSource) -> Result<(), LaunchError> {
    let path = validated_existing_path(source)?;
    let parent = path.parent().ok_or_else(|| {
        LaunchError::Platform("Source file has no parent directory to reveal.".to_string())
    })?;
    platform_open(parent)
}

pub(crate) fn validated_existing_path(source: &ResolvedFileSource) -> Result<PathBuf, LaunchError> {
    let root = fs::canonicalize(&source.storage_root_path).map_err(|error| {
        LaunchError::SourceUnavailable(format!("Approved source root is unavailable: {error}"))
    })?;

    let joined = source.storage_root_path.join(&source.relative_path);
    let canonical = fs::canonicalize(&joined).map_err(|error| {
        LaunchError::SourceUnavailable(format!(
            "Original source is unavailable at its indexed location: {error}"
        ))
    })?;

    if !canonical.starts_with(&root) {
        return Err(LaunchError::BoundaryViolation(
            "Resolved source escaped its approved storage root.".to_string(),
        ));
    }

    if !canonical.is_file() {
        return Err(LaunchError::SourceUnavailable(
            "Indexed source is no longer a regular file.".to_string(),
        ));
    }

    Ok(canonical)
}

#[cfg(windows)]
fn platform_open(path: &Path) -> Result<(), LaunchError> {
    use std::{ffi::c_void, iter, os::windows::ffi::OsStrExt, ptr};

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_command: i32,
        ) -> isize;
    }

    let operation: Vec<u16> = "open".encode_utf16().chain(iter::once(0)).collect();
    let file: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();

    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            ptr::null(),
            ptr::null(),
            1,
        )
    };

    if result <= 32 {
        return Err(LaunchError::Platform(format!(
            "Windows could not open the source (ShellExecuteW code {result})."
        )));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_open(path: &Path) -> Result<(), LaunchError> {
    let child = std::process::Command::new("open").arg(path).spawn()?;
    drop(child);
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_open(path: &Path) -> Result<(), LaunchError> {
    let child = std::process::Command::new("xdg-open").arg(path).spawn()?;
    drop(child);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn unavailable_source_fails_before_platform_launch() {
        let root =
            std::env::temp_dir().join(format!("professional-docx-launch-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test root should be created");

        let source = ResolvedFileSource {
            storage_root_path: root.clone(),
            relative_path: PathBuf::from("missing.pdf"),
        };

        let error =
            open_source(&source).expect_err("missing file must not reach platform launcher");
        assert!(error.to_string().contains("unavailable"));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected() {
        use std::os::unix::fs::symlink;

        let base =
            std::env::temp_dir().join(format!("professional-docx-boundary-{}", Uuid::new_v4()));
        let root = base.join("root");
        let outside = base.join("outside");
        fs::create_dir_all(&root).expect("root should be created");
        fs::create_dir_all(&outside).expect("outside should be created");
        fs::write(outside.join("secret.txt"), b"secret").expect("outside file should exist");
        symlink(outside.join("secret.txt"), root.join("linked.txt"))
            .expect("test symlink should be created");

        let source = ResolvedFileSource {
            storage_root_path: root,
            relative_path: PathBuf::from("linked.txt"),
        };

        let error = validated_existing_path(&source)
            .expect_err("symlink escaping approved root must be rejected");
        assert!(matches!(error, LaunchError::BoundaryViolation(_)));

        let _ = fs::remove_dir_all(base);
    }
}
