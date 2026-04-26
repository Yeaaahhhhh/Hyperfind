// File: crates/platform-windows/src/usn.rs

// Placeholder for USN Journal monitoring — no changes needed from previous version.
// The USN module is for real-time change tracking, not initial scan speed.

#[cfg(not(target_os = "windows"))]
pub struct UsnJournalReader;

#[cfg(not(target_os = "windows"))]
impl UsnJournalReader {
    pub fn new(_volume_letter: char) -> Self { Self }
}