//! Native-only *observational* retention-root Unix posture inspection.
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
/// certificate or transfer token. Windows is deliberately unsupported until
/// handle-based reparse/ACL/owner inspection and durable storage are designed.
/// This function is not connected to any Tauri command or promotion workflow.
pub(crate) fn inspect_retention_root_posture(root: &Path) -> Result<(), PersistenceError> {
    #[cfg(unix)]
    {
        inspect_unix_root(root)
    }
    #[cfg(not(unix))]
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

    #[cfg(not(unix))]
    #[test]
    fn unsupported_host_cannot_claim_verified_protected_root() {
        let scratch = Scratch::new();
        assert!(inspect_retention_root_posture(&scratch.0).is_err());
    }
}
