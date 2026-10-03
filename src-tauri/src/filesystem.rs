use std::{
    fs::{self, File},
    path::Path,
};

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
