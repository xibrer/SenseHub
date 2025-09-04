/// Result of a database save operation
#[derive(Debug)]
pub struct SaveResult {
    pub acc_saved: usize,
    pub audio_saved: usize,
    pub error: Option<String>,
}

impl SaveResult {
    pub fn success(acc_saved: usize, audio_saved: usize) -> Self {
        Self {
            acc_saved,
            audio_saved,
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            acc_saved: 0,
            audio_saved: 0,
            error: Some(error),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

/// Result of an export operation
#[derive(Debug)]
pub struct ExportResult {
    pub success_count: usize,
    pub error_count: usize,
    pub message: String,
}

impl ExportResult {
    pub fn new(success_count: usize, error_count: usize, message: String) -> Self {
        Self {
            success_count,
            error_count,
            message,
        }
    }

    pub fn success_only(count: usize) -> Self {
        Self {
            success_count: count,
            error_count: 0,
            message: format!("Successfully exported {} sessions", count),
        }
    }

    pub fn no_data() -> Self {
        Self {
            success_count: 0,
            error_count: 0,
            message: "No new sessions to export".to_string(),
        }
    }
}
