//! Raster identity shared by tracing and registration.

use super::Source;

impl Source {
    /// Fingerprint for a valid scan page or an unpaginated capture.
    pub fn raster_fingerprint(&self, page: Option<u32>) -> Option<&str> {
        match (page, self.scan_pages) {
            (Some(page), Some(total)) if (1..=total).contains(&page) => self.scan_sha256.as_deref(),
            (None, None) => self.capture.as_ref()?.sha256.as_deref(),
            _ => None,
        }
    }
}
