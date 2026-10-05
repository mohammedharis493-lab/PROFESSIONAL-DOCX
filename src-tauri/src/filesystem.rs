use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

const QUICK_FINGERPRINT_CHUNK_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickFingerprint {
    pub digest: Vec<u8>,
    pub source_stable_during_read: bool,
}

pub fn quick_fingerprint(path: &Path) -> io::Result<QuickFingerprint> {
    let mut file = File::open(path)?;
    let before = file.metadata()?;
    let size = before.len();
    let modified_before = before.modified().ok();

    let mut hasher = Sha256::new();
    hasher.update(b"PROFESSIONAL-DOCX-QUICK-FINGERPRINT-V1");
    hasher.update(size.to_le_bytes());

    let chunk = QUICK_FINGERPRINT_CHUNK_BYTES as u64;
    if size <= chunk.saturating_mul(3) {
        let mut buffer = Vec::with_capacity(size as usize);
        file.read_to_end(&mut buffer)?;
        hasher.update(&buffer);
    } else {
        let middle = size / 2;
        let offsets = [
            0,
            middle.saturating_sub(chunk / 2),
            size.saturating_sub(chunk),
        ];
        let mut buffer = vec![0_u8; QUICK_FINGERPRINT_CHUNK_BYTES];

        for offset in offsets {
            file.seek(SeekFrom::Start(offset))?;
            file.read_exact(&mut buffer)?;
            hasher.update(offset.to_le_bytes());
            hasher.update(&buffer);
        }
    }

    let after = file.metadata()?;
    let source_stable_during_read =
        before.len() == after.len() && modified_before == after.modified().ok();

    Ok(QuickFingerprint {
        digest: hasher.finalize().to_vec(),
        source_stable_during_read,
    })
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct PlatformFileMetadata {
    pub filesystem_identity: Option<Vec<u8>>,
    pub volume_identity: Option<Vec<u8>>,
    pub file_attributes: Option<i64>,
    pub reparse_tag: Option<i64>,
}

#[cfg(windows)]
#[repr(C)]
struct WindowsFileTime {
    low_date_time: u32,
    high_date_time: u32,
}

#[cfg(windows)]
#[repr(C)]
struct WindowsByHandleFileInformation {
    file_attributes: u32,
    creation_time: WindowsFileTime,
    last_access_time: WindowsFileTime,
    last_write_time: WindowsFileTime,
    volume_serial_number: u32,
    file_size_high: u32,
    file_size_low: u32,
    number_of_links: u32,
    file_index_high: u32,
    file_index_low: u32,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    #[link_name = "GetFileInformationByHandle"]
    fn get_file_information_by_handle(
        file: *mut std::ffi::c_void,
        information: *mut WindowsByHandleFileInformation,
    ) -> i32;
}

#[cfg(unix)]
pub fn platform_file_metadata(_path: &Path, metadata: &fs::Metadata) -> PlatformFileMetadata {
    use std::os::unix::fs::MetadataExt;

    PlatformFileMetadata {
        filesystem_identity: Some(metadata.ino().to_le_bytes().to_vec()),
        volume_identity: Some(metadata.dev().to_le_bytes().to_vec()),
        file_attributes: Some(i64::from(metadata.mode())),
        reparse_tag: None,
    }
}

#[cfg(windows)]
pub fn platform_file_metadata(path: &Path, metadata: &fs::Metadata) -> PlatformFileMetadata {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    let fallback = PlatformFileMetadata {
        file_attributes: Some(i64::from(metadata.file_attributes())),
        ..PlatformFileMetadata::default()
    };

    let file = match fs::OpenOptions::new().access_mode(0).open(path) {
        Ok(file) => file,
        Err(_) => return fallback,
    };

    platform_file_metadata_from_open_file(&file, metadata)
}

#[cfg(unix)]
pub fn platform_file_metadata_from_open_file(
    _file: &File,
    metadata: &fs::Metadata,
) -> PlatformFileMetadata {
    platform_file_metadata(Path::new(""), metadata)
}

#[cfg(windows)]
pub fn platform_file_metadata_from_open_file(
    file: &File,
    metadata: &fs::Metadata,
) -> PlatformFileMetadata {
    use std::mem::MaybeUninit;
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::io::AsRawHandle;

    let mut result = PlatformFileMetadata {
        file_attributes: Some(i64::from(metadata.file_attributes())),
        ..PlatformFileMetadata::default()
    };

    let mut information = MaybeUninit::<WindowsByHandleFileInformation>::uninit();
    let succeeded =
        unsafe { get_file_information_by_handle(file.as_raw_handle(), information.as_mut_ptr()) };

    if succeeded == 0 {
        return result;
    }

    let information = unsafe { information.assume_init() };
    let file_index =
        (u64::from(information.file_index_high) << 32) | u64::from(information.file_index_low);

    result.filesystem_identity = Some(file_index.to_le_bytes().to_vec());
    result.volume_identity = Some(information.volume_serial_number.to_le_bytes().to_vec());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn quick_fingerprint_changes_for_same_size_content_changes() {
        let directory = std::env::temp_dir().join(format!(
            "professional-docx-fingerprint-test-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).expect("test directory should exist");
        let path = directory.join("sample.bin");

        let mut first_content = vec![0x41_u8; QUICK_FINGERPRINT_CHUNK_BYTES * 4];
        fs::write(&path, &first_content).expect("first content should be written");
        let first = quick_fingerprint(&path).expect("first fingerprint should succeed");

        let midpoint = first_content.len() / 2;
        first_content[midpoint] = 0x42;
        fs::write(&path, &first_content).expect("second content should be written");
        let second = quick_fingerprint(&path).expect("second fingerprint should succeed");

        assert!(first.source_stable_during_read);
        assert!(second.source_stable_during_read);
        assert_eq!(first.digest.len(), 32);
        assert_eq!(second.digest.len(), 32);
        assert_ne!(first.digest, second.digest);

        let _ = fs::remove_dir_all(directory);
    }
}
