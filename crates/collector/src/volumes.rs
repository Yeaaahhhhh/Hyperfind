// File: crates/collector/src/volumes.rs

//! Volume/drive discovery for automatic full-system indexing.
//! Detects all available drives/mount points on the system.

use hyperfind_common::models::VolumeInfo;
use tracing::info;

/// Discovers all available volumes on the system.
pub fn discover_volumes() -> Vec<VolumeInfo> {
    let mut volumes = Vec::new();

    #[cfg(target_os = "windows")]
    {
        volumes = discover_windows_volumes();
    }

    #[cfg(target_os = "linux")]
    {
        volumes = discover_linux_volumes();
    }

    #[cfg(target_os = "macos")]
    {
        volumes = discover_macos_volumes();
    }

    info!("Discovered {} volumes", volumes.len());
    for v in &volumes {
        info!(
            "  {} ({}) - {:.2} GB total, {:.2} GB free",
            v.mount_point,
            v.label.as_deref().unwrap_or(""),
            v.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
            v.free_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        );
    }

    volumes
}

#[cfg(target_os = "windows")]
fn discover_windows_volumes() -> Vec<VolumeInfo> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let mut volumes = Vec::new();

    // Check drives A-Z
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        let path = std::path::Path::new(&drive);

        if path.exists() {
            let (total, free) = get_disk_space_windows(&drive);
            volumes.push(VolumeInfo {
                mount_point: drive,
                label: None,
                total_bytes: total,
                free_bytes: free,
                fs_type: None,
            });
        }
    }

    volumes
}

#[cfg(target_os = "windows")]
fn get_disk_space_windows(path: &str) -> (u64, u64) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    // Use GetDiskFreeSpaceExW
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailableToCaller: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }

    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
    let mut free_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut free_total: u64 = 0;

    let result = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_caller,
            &mut total,
            &mut free_total,
        )
    };

    if result != 0 {
        (total, free_total)
    } else {
        (0, 0)
    }
}

#[cfg(target_os = "linux")]
fn discover_linux_volumes() -> Vec<VolumeInfo> {
    let mut volumes = Vec::new();

    if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let mount_point = parts[1].to_string();
                let fs_type = parts[2].to_string();

                // Skip virtual filesystems
                if ["proc", "sysfs", "tmpfs", "devpts", "cgroup", "cgroup2",
                    "securityfs", "pstore", "debugfs", "fusectl", "configfs",
                    "hugetlbfs", "mqueue", "binfmt_misc", "tracefs"]
                    .contains(&fs_type.as_str())
                {
                    continue;
                }

                // Skip if not a real path
                if !mount_point.starts_with('/') {
                    continue;
                }

                volumes.push(VolumeInfo {
                    mount_point,
                    label: None,
                    total_bytes: 0,
                    free_bytes: 0,
                    fs_type: Some(fs_type),
                });
            }
        }
    }

    // Fallback: at least include root
    if volumes.is_empty() {
        volumes.push(VolumeInfo {
            mount_point: "/".to_string(),
            label: None,
            total_bytes: 0,
            free_bytes: 0,
            fs_type: None,
        });
    }

    volumes
}

#[cfg(target_os = "macos")]
fn discover_macos_volumes() -> Vec<VolumeInfo> {
    let mut volumes = vec![
        VolumeInfo {
            mount_point: "/".to_string(),
            label: Some("Macintosh HD".to_string()),
            total_bytes: 0,
            free_bytes: 0,
            fs_type: Some("apfs".to_string()),
        },
    ];

    // Also check /Volumes
    if let Ok(entries) = std::fs::read_dir("/Volumes") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let mount = path.to_string_lossy().to_string();
                if mount != "/" {
                    volumes.push(VolumeInfo {
                        mount_point: mount,
                        label: entry.file_name().to_str().map(|s| s.to_string()),
                        total_bytes: 0,
                        free_bytes: 0,
                        fs_type: None,
                    });
                }
            }
        }
    }

    volumes
}