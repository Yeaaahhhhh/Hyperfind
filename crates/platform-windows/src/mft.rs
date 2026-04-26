// File: crates/platform-windows/src/mft.rs

#[cfg(target_os = "windows")]
mod inner {
    use hyperfind_common::errors::HyperFindError;
    use hyperfind_common::models::FileDocument;
    use hyperfind_index_engine::document;
    use rayon::prelude::*;
    use rustc_hash::FxHashMap;
    use std::mem;
    use std::ptr;
    use std::sync::Arc;
    use tracing::{debug, info, warn};

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

    struct MftRecord {
        file_ref: u64,
        parent_ref: u64,
        name: Box<str>,
        is_dir: bool,
        timestamp: i64,
    }

    #[repr(C)]
    struct MftEnumDataV0 {
        start_file_reference_number: u64,
        low_usn: i64,
        high_usn: i64,
    }

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn open_volume(letter: char) -> Result<isize, HyperFindError> {
        let path = format!("\\\\.\\{}:", letter);
        let wide = to_wide(&path);

        let h = unsafe {
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

        if h == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            return Err(HyperFindError::PlatformError(format!(
                "Failed to open volume {}:, error code {}. Run as Administrator.",
                letter, err
            )));
        }

        Ok(h)
    }

    fn enumerate_mft(handle: isize) -> Result<Vec<MftRecord>, HyperFindError> {
        let mut records: Vec<MftRecord> = Vec::with_capacity(800_000);
        let mut input = MftEnumDataV0 {
            start_file_reference_number: 0,
            low_usn: 0,
            high_usn: i64::MAX,
        };

        const BUF_SIZE: usize = 4 * 1024 * 1024;
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
                    break;
                }
                debug!("FSCTL_ENUM_USN_DATA error: {}", err);
                break;
            }

            if bytes_returned < 8 {
                break;
            }

            let next_ref = u64::from_ne_bytes(buffer[0..8].try_into().unwrap());
            let mut offset = 8usize;
            let limit = bytes_returned as usize;

            while offset + 64 <= limit {
                let record_len =
                    u32::from_ne_bytes(buffer[offset..offset + 4].try_into().unwrap()) as usize;
                if record_len < 64 || offset + record_len > limit {
                    break;
                }

                let file_ref =
                    u64::from_ne_bytes(buffer[offset + 8..offset + 16].try_into().unwrap());
                let parent_ref =
                    u64::from_ne_bytes(buffer[offset + 16..offset + 24].try_into().unwrap());
                let timestamp =
                    i64::from_ne_bytes(buffer[offset + 32..offset + 40].try_into().unwrap());
                let attrs =
                    u32::from_ne_bytes(buffer[offset + 52..offset + 56].try_into().unwrap());
                let name_len =
                    u16::from_ne_bytes(buffer[offset + 56..offset + 58].try_into().unwrap())
                        as usize;
                let name_off =
                    u16::from_ne_bytes(buffer[offset + 58..offset + 60].try_into().unwrap())
                        as usize;

                let abs_s = offset + name_off;
                let abs_e = abs_s + name_len;

                if abs_e <= limit && name_len >= 2 {
                    let raw = &buffer[abs_s..abs_e];
                    let units_len = raw.len() / 2;
                    let mut u16buf = Vec::with_capacity(units_len);

                    let mut i = 0usize;
                    while i + 1 < raw.len() {
                        u16buf.push(u16::from_ne_bytes([raw[i], raw[i + 1]]));
                        i += 2;
                    }

                    let name = String::from_utf16_lossy(&u16buf);
                    if !name.is_empty() && name != "." && name != ".." {
                        let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
                        records.push(MftRecord {
                            file_ref: file_ref & 0x0000_FFFF_FFFF_FFFF,
                            parent_ref: parent_ref & 0x0000_FFFF_FFFF_FFFF,
                            name: name.into_boxed_str(),
                            is_dir,
                            timestamp,
                        });
                    }
                }

                offset += record_len;
            }

            input.start_file_reference_number = next_ref;
        }

        Ok(records)
    }

    struct ExcludeMatcher {
        lower: Vec<String>,
    }

    impl ExcludeMatcher {
        fn new(patterns: &[String]) -> Self {
            Self {
                lower: patterns.iter().map(|p| p.to_ascii_lowercase()).collect(),
            }
        }

        fn matches_segment(&self, seg: &str) -> bool {
            if self.lower.is_empty() {
                return false;
            }

            if seg.is_ascii() {
                for p in &self.lower {
                    if p.len() == seg.len() && seg.eq_ignore_ascii_case(p) {
                        return true;
                    }
                }
                false
            } else {
                let s = seg.to_lowercase();
                self.lower.iter().any(|p| *p == s)
            }
        }

        fn excluded_path(&self, path: &str) -> bool {
            for seg in path.split('\\') {
                if self.matches_segment(seg) {
                    return true;
                }
            }
            false
        }
    }

    fn resolve_paths(records: &[MftRecord], volume_prefix: &str) -> Vec<(usize, String)> {
        let mut lookup: FxHashMap<u64, u32> =
            FxHashMap::with_capacity_and_hasher(records.len(), Default::default());

        for (i, r) in records.iter().enumerate() {
            lookup.insert(r.file_ref, i as u32);
        }

        records
            .par_iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let mut comps: Vec<&str> = Vec::with_capacity(16);
                let mut cur = r.file_ref;
                let mut depth = 0u32;

                loop {
                    if let Some(&idx) = lookup.get(&cur) {
                        let rec = &records[idx as usize];
                        if cur == rec.parent_ref || depth > 512 {
                            break;
                        }
                        comps.push(&rec.name);
                        cur = rec.parent_ref;
                        depth += 1;
                    } else {
                        break;
                    }
                }

                if comps.is_empty() {
                    return None;
                }

                comps.reverse();

                let mut total = volume_prefix.len();
                for c in &comps {
                    total += c.len() + 1;
                }

                let mut path = String::with_capacity(total);
                path.push_str(volume_prefix);

                for (k, c) in comps.iter().enumerate() {
                    if k > 0 {
                        path.push('\\');
                    }
                    path.push_str(c);
                }

                Some((i, path))
            })
            .collect()
    }

    fn filetime_to_datetime(ft: i64) -> chrono::DateTime<chrono::Utc> {
        const EPOCH_DIFF: i64 = 11_644_473_600;
        let secs = (ft / 10_000_000) - EPOCH_DIFF;
        let nanos = ((ft % 10_000_000) * 100) as u32;
        chrono::DateTime::from_timestamp(secs, nanos).unwrap_or_default()
    }

    fn extract_extension(name: &str) -> String {
        if let Some(pos) = name.rfind('.') {
            if pos > 0 && pos < name.len() - 1 {
                let ext = &name[pos + 1..];
                if ext.is_ascii() {
                    let mut out = String::with_capacity(ext.len());
                    for b in ext.bytes() {
                        out.push((b as char).to_ascii_lowercase());
                    }
                    return out;
                }
                return ext.to_lowercase();
            }
        }
        String::new()
    }

    pub fn scan_volume_mft(
        volume_letter: char,
        excluded_patterns: &[String],
    ) -> Result<Vec<FileDocument>, HyperFindError> {
        info!("MFT scan starting for volume {}:", volume_letter);
        let start = std::time::Instant::now();

        let handle = open_volume(volume_letter)?;
        let records = enumerate_mft(handle);
        unsafe {
            CloseHandle(handle);
        }
        let records = records?;

        info!(
            "MFT enumeration: {} records in {:.2}s",
            records.len(),
            start.elapsed().as_secs_f64()
        );

        let volume_prefix = format!("{}:\\", volume_letter);
        let resolved = resolve_paths(&records, &volume_prefix);

        info!(
            "Path resolution: {} entries in {:.2}s",
            resolved.len(),
            start.elapsed().as_secs_f64()
        );

        let matcher = Arc::new(ExcludeMatcher::new(excluded_patterns));

        let documents: Vec<FileDocument> = resolved
            .into_par_iter()
            .filter(|(_, p)| !matcher.excluded_path(p))
            .map(|(i, path)| {
                let r = &records[i];
                let ext = extract_extension(&r.name);
                let modified = filetime_to_datetime(r.timestamp);

                FileDocument {
                    id: document::next_id(),
                    name: Arc::from(r.name.as_ref()),
                    path: Arc::from(path.as_str()),
                    extension: Arc::from(ext.as_str()),
                    size: 0,
                    modified,
                    is_dir: r.is_dir,
                    content_hash: None,
                }
            })
            .collect();

        info!(
            "MFT scan complete: {} documents in {:.2}s",
            documents.len(),
            start.elapsed().as_secs_f64()
        );

        Ok(documents)
    }

    pub fn scan_all_volumes_mft(
        excluded_patterns: &[String],
    ) -> Result<Vec<FileDocument>, HyperFindError> {
        let mut letters: Vec<char> = Vec::new();
        for letter in b'A'..=b'Z' {
            let l = letter as char;
            let p = format!("{}:\\", l);
            if std::path::Path::new(&p).exists() {
                letters.push(l);
            }
        }

        if letters.is_empty() {
            return Ok(Vec::new());
        }

        let results: Vec<Result<Vec<FileDocument>, HyperFindError>> = letters
            .par_iter()
            .map(|&l| {
                info!("MFT scanning volume {}: (parallel)", l);
                scan_volume_mft(l, excluded_patterns)
            })
            .collect();

        let mut all = Vec::new();
        for (l, r) in letters.iter().zip(results.into_iter()) {
            match r {
                Ok(d) => {
                    info!("Volume {}: {} entries", l, d.len());
                    all.extend(d);
                }
                Err(e) => {
                    warn!(
                        "Volume {} MFT failed: {} — aborting (caller may fall back)",
                        l, e
                    );
                    return Err(e);
                }
            }
        }

        Ok(all)
    }
}

#[cfg(target_os = "windows")]
pub use inner::*;

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