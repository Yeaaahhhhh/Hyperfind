// File: crates/platform-windows/src/mft.rs

#[cfg(target_os = "windows")]
mod inner {
    use hyperfind_common::errors::HyperFindError;
    use hyperfind_common::models::FileDocument;
    use hyperfind_index_engine::document;
    use std::collections::HashMap;
    use std::mem;
    use std::ptr;
    use tracing::{info, warn, debug};

    // Windows API constants
    const FILE_SHARE_READ: u32 = 0x00000001;
    const FILE_SHARE_WRITE: u32 = 0x00000002;
    const OPEN_EXISTING: u32 = 3;
    const GENERIC_READ: u32 = 0x80000000;
    const INVALID_HANDLE_VALUE: isize = -1;
    const FSCTL_ENUM_USN_DATA: u32 = 0x000900b3;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

    extern "system" {
        fn CreateFileW(
            lpFileName: *const u16,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *const u8,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: isize,
        ) -> isize;

        fn DeviceIoControl(
            hDevice: isize,
            dwIoControlCode: u32,
            lpInBuffer: *const u8,
            nInBufferSize: u32,
            lpOutBuffer: *mut u8,
            nOutBufferSize: u32,
            lpBytesReturned: *mut u32,
            lpOverlapped: *mut u8,
        ) -> i32;

        fn CloseHandle(hObject: isize) -> i32;

        fn GetLastError() -> u32;
    }

    /// Raw MFT record parsed from USN enumeration.
    struct MftRecord {
        file_ref: u64,
        parent_ref: u64,
        name: String,
        is_dir: bool,
        file_attributes: u32,
        timestamp: i64,
        file_size: u64,
    }

    /// Input structure for FSCTL_ENUM_USN_DATA.
    #[repr(C)]
    struct MftEnumDataV0 {
        start_file_reference_number: u64,
        low_usn: i64,
        high_usn: i64,
    }

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Opens a volume handle for MFT enumeration.
    fn open_volume(letter: char) -> Result<isize, HyperFindError> {
        let path = format!("\\\\.\\{}:", letter);
        let wide = to_wide(&path);

        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                0,
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            return Err(HyperFindError::PlatformError(format!(
                "Failed to open volume {}:, error code {}. Run as Administrator.", letter, err
            )));
        }

        Ok(handle)
    }

    /// Enumerates all MFT records on a volume using FSCTL_ENUM_USN_DATA.
    fn enumerate_mft(handle: isize) -> Result<Vec<MftRecord>, HyperFindError> {
        let mut records = Vec::with_capacity(500_000);

        let mut input = MftEnumDataV0 {
            start_file_reference_number: 0,
            low_usn: 0,
            high_usn: i64::MAX,
        };

        const BUF_SIZE: usize = 1024 * 1024; // 1 MB buffer
        let mut buffer = vec![0u8; BUF_SIZE];

        loop {
            let mut bytes_returned: u32 = 0;

            let ok = unsafe {
                DeviceIoControl(
                    handle,
                    FSCTL_ENUM_USN_DATA,
                    &input as *const _ as *const u8,
                    mem::size_of::<MftEnumDataV0>() as u32,
                    buffer.as_mut_ptr(),
                    BUF_SIZE as u32,
                    &mut bytes_returned,
                    ptr::null_mut(),
                )
            };

            if ok == 0 {
                let err = unsafe { GetLastError() };
                if err == 38 {
                    break; // ERROR_HANDLE_EOF — enumeration complete
                }
                debug!("FSCTL_ENUM_USN_DATA error: {}", err);
                break;
            }

            if bytes_returned < 8 {
                break;
            }

            let next_ref = u64::from_ne_bytes(buffer[0..8].try_into().unwrap());

            let mut offset = 8usize;
            while offset + 64 <= bytes_returned as usize {
                let record_len = u32::from_ne_bytes(
                    buffer[offset..offset + 4].try_into().unwrap()
                ) as usize;

                if record_len < 64 || offset + record_len > bytes_returned as usize {
                    break;
                }

                let file_ref = u64::from_ne_bytes(
                    buffer[offset + 8..offset + 16].try_into().unwrap()
                );
                let parent_ref = u64::from_ne_bytes(
                    buffer[offset + 16..offset + 24].try_into().unwrap()
                );
                let timestamp = i64::from_ne_bytes(
                    buffer[offset + 32..offset + 40].try_into().unwrap()
                );
                let file_attributes = u32::from_ne_bytes(
                    buffer[offset + 52..offset + 56].try_into().unwrap()
                );
                let name_len = u16::from_ne_bytes(
                    buffer[offset + 56..offset + 58].try_into().unwrap()
                ) as usize;
                let name_offset = u16::from_ne_bytes(
                    buffer[offset + 58..offset + 60].try_into().unwrap()
                ) as usize;

                let abs_name_start = offset + name_offset;
                let abs_name_end = abs_name_start + name_len;

                if abs_name_end <= bytes_returned as usize && name_len > 0 {
                    let name_u16: Vec<u16> = buffer[abs_name_start..abs_name_end]
                        .chunks_exact(2)
                        .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                        .collect();
                    let name = String::from_utf16_lossy(&name_u16);

                    let is_dir = (file_attributes & FILE_ATTRIBUTE_DIRECTORY) != 0;

                    if !name.is_empty() && name != "." && name != ".." {
                        records.push(MftRecord {
                            file_ref: file_ref & 0x0000_FFFF_FFFF_FFFF,
                            parent_ref: parent_ref & 0x0000_FFFF_FFFF_FFFF,
                            name,
                            is_dir,
                            file_attributes,
                            timestamp,
                            file_size: 0,
                        });
                    }
                }

                offset += record_len;
            }

            input.start_file_reference_number = next_ref;
        }

        Ok(records)
    }

    /// Resolves MFT records into full paths by walking parent references.
    fn resolve_paths(
        records: &[MftRecord],
        volume_prefix: &str,
    ) -> Vec<(MftRecord, String)> {
        let mut lookup: HashMap<u64, (&str, u64)> = HashMap::with_capacity(records.len());
        for r in records {
            lookup.insert(r.file_ref, (r.name.as_str(), r.parent_ref));
        }

        let mut resolved = Vec::with_capacity(records.len());

        for r in records {
            let mut components: Vec<&str> = Vec::with_capacity(16);
            let mut current = r.file_ref;
            let mut depth = 0u32;

            loop {
                if let Some(&(name, parent)) = lookup.get(&current) {
                    if current == parent || depth > 512 {
                        break;
                    }
                    components.push(name);
                    current = parent;
                    depth += 1;
                } else {
                    break;
                }
            }

            if components.is_empty() {
                continue;
            }

            components.reverse();

            let mut path = String::with_capacity(
                volume_prefix.len() + components.iter().map(|c| c.len() + 1).sum::<usize>()
            );
            path.push_str(volume_prefix);
            for (i, comp) in components.iter().enumerate() {
                if i > 0 {
                    path.push('\\');
                }
                path.push_str(comp);
            }

            resolved.push((
                MftRecord {
                    file_ref: r.file_ref,
                    parent_ref: r.parent_ref,
                    name: r.name.clone(),
                    is_dir: r.is_dir,
                    file_attributes: r.file_attributes,
                    timestamp: r.timestamp,
                    file_size: r.file_size,
                },
                path,
            ));
        }

        resolved
    }

    /// Converts a Windows FILETIME to chrono DateTime.
    fn filetime_to_datetime(ft: i64) -> chrono::DateTime<chrono::Utc> {
        const EPOCH_DIFF: i64 = 11_644_473_600;
        let secs = (ft / 10_000_000) - EPOCH_DIFF;
        let nanos = ((ft % 10_000_000) * 100) as u32;
        chrono::DateTime::from_timestamp(secs, nanos).unwrap_or_default()
    }

    fn extract_extension(name: &str) -> String {
        if let Some(pos) = name.rfind('.') {
            if pos > 0 && pos < name.len() - 1 {
                return name[pos + 1..].to_lowercase();
            }
        }
        String::new()
    }

    fn extract_parent(path: &str) -> String {
        if let Some(pos) = path.rfind('\\') {
            return path[..pos].to_string();
        }
        String::new()
    }

    fn should_exclude(path: &str, patterns: &[String]) -> bool {
        let path_lower = path.to_lowercase();
        for pattern in patterns {
            let p = pattern.to_lowercase();
            for segment in path_lower.split('\\') {
                if segment == p {
                    return true;
                }
            }
        }
        false
    }

    /// Main entry point: scan a volume using MFT and return FileDocument list.
    pub fn scan_volume_mft(
        volume_letter: char,
        excluded_patterns: &[String],
    ) -> Result<Vec<FileDocument>, HyperFindError> {
        info!("MFT scan starting for volume {}:", volume_letter);
        let start = std::time::Instant::now();

        let handle = open_volume(volume_letter)?;

        let records = enumerate_mft(handle);
        unsafe { CloseHandle(handle); }
        let records = records?;

        info!("MFT enumeration: {} records in {:.2}s",
            records.len(), start.elapsed().as_secs_f64());

        let volume_prefix = format!("{}:\\", volume_letter);
        let resolved = resolve_paths(&records, &volume_prefix);

        info!("Path resolution: {} entries in {:.2}s",
            resolved.len(), start.elapsed().as_secs_f64());

        let documents: Vec<FileDocument> = resolved
            .into_iter()
            .filter(|(_, path)| !should_exclude(path, excluded_patterns))
            .map(|(record, path)| {
                let extension = extract_extension(&record.name);
                let parent = extract_parent(&path);
                let modified = filetime_to_datetime(record.timestamp);

                FileDocument {
                    id: document::next_id(),
                    name: record.name.clone(),
                    name_lower: record.name.to_lowercase(),
                    path,
                    parent,
                    extension,
                    size: record.file_size,
                    modified,
                    is_dir: record.is_dir,
                    content_hash: None,
                }
            })
            .collect();

        info!("MFT scan complete: {} documents in {:.2}s",
            documents.len(), start.elapsed().as_secs_f64());

        Ok(documents)
    }

    /// Scans all NTFS volumes using MFT.
    /// On failure, returns Err so the caller can fall back to walkdir.
    pub fn scan_all_volumes_mft(
        excluded_patterns: &[String],
    ) -> Result<Vec<FileDocument>, HyperFindError> {
        let mut all_docs = Vec::new();

        for letter in b'A'..=b'Z' {
            let letter = letter as char;
            let drive_path = format!("{}:\\", letter);
            let path = std::path::Path::new(&drive_path);

            if !path.exists() {
                continue;
            }

            info!("MFT scanning volume {}:", letter);

            match scan_volume_mft(letter, excluded_patterns) {
                Ok(docs) => {
                    info!("Volume {}: found {} entries", letter, docs.len());
                    all_docs.extend(docs);
                }
                Err(e) => {
                    warn!("Volume {} MFT scan failed: {}", letter, e);
                    return Err(e);
                }
            }
        }

        Ok(all_docs)
    }
}

#[cfg(target_os = "windows")]
pub use inner::*;

// Non-Windows stubs
#[cfg(not(target_os = "windows"))]
pub fn scan_volume_mft(
    _volume_letter: char,
    _excluded_patterns: &[String],
) -> Result<Vec<hyperfind_common::models::FileDocument>, hyperfind_common::errors::HyperFindError> {
    Err(hyperfind_common::errors::HyperFindError::PlatformError(
        "MFT scanning is only available on Windows".into(),
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn scan_all_volumes_mft(
    _excluded_patterns: &[String],
) -> Result<Vec<hyperfind_common::models::FileDocument>, hyperfind_common::errors::HyperFindError> {
    Err(hyperfind_common::errors::HyperFindError::PlatformError(
        "MFT scanning is only available on Windows".into(),
    ))
}