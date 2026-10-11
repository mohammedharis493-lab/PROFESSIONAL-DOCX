//! Native-only *observational* retention-root platform posture inspection.
//! This check NEVER authorizes capture, declares controlled custody, or creates
//! a principal. A path check cannot lock filesystem objects or attest ACLs,
//! ownership across a later write, mount policy, or power-loss durability.
use crate::persistence::PersistenceError;
use std::path::Path;

fn denied(why: &str) -> PersistenceError {
    PersistenceError::Configuration(format!(
        "Normal Data protected-root posture unavailable: {why}"
    ))
}

/// Observe a private retention root on the current host. This is an early,
/// read-only prerequisite for a FUTURE protected-storage design, never a
/// certificate or transfer token. Windows can observe path/reparse shapes,
/// but it NEVER approves a root without verified owner/ACL and durability.
/// This function is not connected to any Tauri command or promotion workflow.
pub(crate) fn inspect_retention_root_posture(root: &Path) -> Result<(), PersistenceError> {
    #[cfg(unix)]
    {
        inspect_unix_root(root)
    }
    #[cfg(windows)]
    {
        // Observe no-follow path identity and the root's process-user owner SID.
        // Neither observation is a DACL/ACL or durable-custody proof.
        inspect_windows_root_path_shape(root)?;
        Err(denied(
            "Windows DACL permissions and durable protected storage are not verified",
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = root;
        Err(denied(
            "platform-specific protected-directory inspection is unavailable",
        ))
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> u32;
}

#[cfg(unix)]
fn effective_user_id() -> u32 {
    // OS-supplied effective identity, NOT a caller-provided actor/UUID/role.
    unsafe { geteuid() }
}

#[cfg(unix)]
fn check_unix_component(
    metadata: &std::fs::Metadata,
    effective_uid: u32,
    is_final_root: bool,
) -> Result<(), PersistenceError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(denied("a retention-root ancestor is not a real directory"));
    }
    let owner = metadata.uid();
    if is_final_root {
        if owner != effective_uid {
            return Err(denied(
                "private retention root is not owned by current effective user",
            ));
        }
        // Owner-only access is a necessary baseline, not an ACL attestation.
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(denied(
                "private retention root is accessible to other Unix users",
            ));
        }
    } else if owner != effective_uid && owner != 0 {
        return Err(denied("a retention-root ancestor has an unexpected owner"));
    }
    if metadata.permissions().mode() & 0o022 != 0 {
        return Err(denied(
            "a retention-root ancestor is writable by other Unix users",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn inspect_unix_root(root: &Path) -> Result<(), PersistenceError> {
    use std::{fs, path::Component};

    if !root.is_absolute()
        || root
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(denied(
            "retention root must use an absolute, normalized path",
        ));
    }
    let chain: Vec<_> = root.ancestors().collect();
    if chain.is_empty() {
        return Err(denied("retention root is missing"));
    }

    // Resolve type/symlink shape FIRST, so even a privately owned leaf under
    // /tmp or an otherwise unsafe ancestor cannot conceal an intermediate link.
    for directory in chain.iter().rev() {
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(denied(
                "retention root has a symlink or non-directory ancestor",
            ));
        }
    }
    if fs::canonicalize(root)? != root {
        return Err(denied("retention root path is not canonical"));
    }

    let uid = effective_user_id();
    for (position, directory) in chain.iter().rev().enumerate() {
        let metadata = fs::symlink_metadata(directory)?;
        check_unix_component(&metadata, uid, position + 1 == chain.len())?;
    }

    // Deliberately no "approved root" capability returned: hostile writers can
    // race these checks and POSIX mode bits cannot attest all platform ACLs.
    Ok(())
}

/// Windows path-shape inspection is a diagnostic prerequisite ONLY. It does
/// not examine the effective token, DACLs, owner SIDs, inherited permissions,
/// remote share controls, or crash-consistent directory persistence.
/// It checks only the final root owner SID against the current process token
/// user SID; it does not inspect DACLs, ACEs, groups or token impersonation.
///
/// Each ancestor is opened as a directory with OPEN_REPARSE_POINT so symbolic
/// links, junctions and mount-point reparse attributes cannot silently be
/// followed for the *final component*. Held handles are compared to second
/// no-follow opens to detect some replacement races; this is not atomic with
/// path traversal and never grants custody or authorization.
#[cfg(windows)]
fn inspect_windows_root_path_shape(root: &Path) -> Result<(), PersistenceError> {
    use std::path::{Component, Prefix};

    let mut parts = root.components();
    if !root.is_absolute()
        || !matches!(
            parts.next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_))
        )
        || !matches!(parts.next(), Some(Component::RootDir))
        || parts.any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(denied(
            "Windows retention root must be an absolute local drive path",
        ));
    }
    let mut observed = Vec::new();
    let ancestors: Vec<_> = root.ancestors().collect();
    for directory in ancestors.into_iter().rev() {
        let (handle, volume_id, file_id) = open_windows_directory_without_following(directory)?;
        observed.push((directory.to_path_buf(), handle, volume_id, file_id));
    }
    if observed.is_empty() {
        return Err(denied("Windows retention root has no directory components"));
    }
    for (path, _held_handle, volume_id, file_id) in &observed {
        let (_second_handle, second_volume, second_id) =
            open_windows_directory_without_following(path)?;
        if *volume_id != second_volume || *file_id != second_id {
            return Err(denied(
                "Windows directory identity changed during observation",
            ));
        }
    }
    // The owner query needs READ_CONTROL, which the shape handles purposely
    // avoid requesting. A second no-follow handle must match the held root
    // volume/file ID before its owner SID is compared with the process token.
    let (root_path, _, volume_id, file_id) = observed
        .last()
        .ok_or_else(|| denied("Windows retention root was not observed"))?;
    const READ_CONTROL: u32 = 0x0002_0000;
    let (owner_handle, owner_volume, owner_file) =
        open_windows_directory_with_access(root_path, READ_CONTROL)?;
    if *volume_id != owner_volume || *file_id != owner_file {
        return Err(denied("Windows owner handle does not match retained root"));
    }
    crate::normal_data_windows_owner_probe::observe_owner_matches_process_user(&owner_handle)?;
    Ok(())
}

#[cfg(windows)]
fn windows_is_plain_directory(attributes: u32) -> bool {
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    attributes & FILE_ATTRIBUTE_DIRECTORY != 0 && attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
}

#[cfg(windows)]
fn open_windows_directory_without_following(
    path: &Path,
) -> Result<(std::fs::File, Vec<u8>, Vec<u8>), PersistenceError> {
    open_windows_directory_with_access(path, 0)
}

#[cfg(windows)]
fn open_windows_directory_with_access(
    path: &Path,
    requested_access: u32,
) -> Result<(std::fs::File, Vec<u8>, Vec<u8>), PersistenceError> {
    use crate::filesystem;
    use std::fs::{self, OpenOptions};
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    // Rust OpenOptions custom_flags maps to Win32 dwFlagsAndAttributes.
    // BACKUP_SEMANTICS opens directories. OPEN_REPARSE_POINT prevents the
    // terminal junction/symlink from resolving before handle inspection.
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let prior = fs::symlink_metadata(path)?;
    if !prior.file_type().is_dir() || !windows_is_plain_directory(prior.file_attributes()) {
        return Err(denied("Windows path has a reparse point or non-directory"));
    }
    let mut options = OpenOptions::new();
    options
        .access_mode(requested_access)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    let handle = options.open(path)?;
    let opened = handle.metadata()?;
    if !opened.file_type().is_dir() || !windows_is_plain_directory(opened.file_attributes()) {
        return Err(denied("Windows opened directory is a reparse point"));
    }
    let identity = filesystem::platform_file_metadata_from_open_file(&handle, &opened);
    let volume_id = identity
        .volume_identity
        .ok_or_else(|| denied("Windows directory volume identity unavailable"))?;
    let file_id = identity
        .filesystem_identity
        .ok_or_else(|| denied("Windows directory file identity unavailable"))?;
    if volume_id.is_empty() || file_id.is_empty() {
        return Err(denied("Windows directory identity is incomplete"));
    }
    // Catch an obvious path substitution after opening a no-follow handle.
    let latest = fs::symlink_metadata(path)?;
    if !latest.file_type().is_dir() || !windows_is_plain_directory(latest.file_attributes()) {
        return Err(denied(
            "Windows path changed to reparse point or non-directory",
        ));
    }
    Ok((handle, volume_id, file_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};
    use uuid::Uuid;

    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = env::temp_dir().join(format!("pdox-root-posture-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("temporary test root");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn relative_path_and_missing_root_never_pass_as_protected() {
        assert!(inspect_retention_root_posture(Path::new("relative-private-root")).is_err());
        let scratch = Scratch::new();
        assert!(inspect_retention_root_posture(&scratch.0.join("missing")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn unix_root_requires_private_effective_owner_and_no_other_writers() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new();
        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700))
            .expect("owner-only scratch permissions");
        let metadata = fs::symlink_metadata(&scratch.0).expect("metadata");
        let uid = effective_user_id();
        assert!(check_unix_component(&metadata, uid, true).is_ok());
        assert!(check_unix_component(&metadata, uid.wrapping_add(1), true).is_err());

        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o750))
            .expect("group-readable root");
        assert!(
            check_unix_component(
                &fs::symlink_metadata(&scratch.0).expect("metadata"),
                uid,
                true
            )
            .is_err(),
            "private root must reject group read access"
        );

        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o770))
            .expect("group writable root");
        assert!(
            check_unix_component(
                &fs::symlink_metadata(&scratch.0).expect("metadata"),
                uid,
                false
            )
            .is_err(),
            "even an ancestor must reject group write access"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_rejects_symlinked_parent_and_world_writable_ancestor() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let scratch = Scratch::new();
        let parent = scratch.0.join("parent");
        let private = parent.join("private");
        fs::create_dir_all(&private).expect("private candidate root");
        fs::set_permissions(&private, fs::Permissions::from_mode(0o700))
            .expect("private directory");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).expect("insecure ancestor");
        assert!(inspect_retention_root_posture(&private).is_err());

        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).expect("fix ancestor");
        let shortcut = scratch.0.join("shortcut");
        symlink(&parent, &shortcut).expect("link to parent");
        assert!(
            inspect_retention_root_posture(&shortcut.join("private")).is_err(),
            "intermediate symlink must fail closed"
        );
        assert!(
            inspect_retention_root_posture(&shortcut).is_err(),
            "symlink used as root must fail closed"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_root_path_probe_checks_attributes_and_never_authorizes() {
        assert!(windows_is_plain_directory(0x10));
        assert!(!windows_is_plain_directory(0x410));
        assert!(!windows_is_plain_directory(0x400));
        assert!(!windows_is_plain_directory(0));
        let scratch = Scratch::new();
        assert!(
            inspect_windows_root_path_shape(&scratch.0).is_ok(),
            "new local test root must match the actual process token owner SID"
        );
        assert!(inspect_retention_root_posture(&scratch.0).is_err());
        assert!(inspect_windows_root_path_shape(&scratch.0.join("missing")).is_err());
        assert!(inspect_windows_root_path_shape(Path::new("relative-root")).is_err());
        assert!(inspect_windows_root_path_shape(&scratch.0.join("..").join("different")).is_err());
    }

    #[cfg(not(unix))]
    #[test]
    fn unsupported_host_cannot_claim_verified_protected_root() {
        let scratch = Scratch::new();
        assert!(inspect_retention_root_posture(&scratch.0).is_err());
    }
}
