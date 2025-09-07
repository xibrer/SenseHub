use super::{DataPoint, AudioData, ExportResult};

/// Database task enumeration for async operations
#[derive(Clone)]
pub enum DatabaseTask {
    Save {
        accelerometer_data: Vec<DataPoint>,
        audio_data: Vec<f64>,
        audio_metadata: Option<AudioData>,
        audio_start_timestamp: Option<i64>,
        audio_end_timestamp: Option<i64>,
        session_id: String,
        username: String,
        scenario: String,
    },
    Export {
        export_type: ExportType,
        response_sender: crossbeam_channel::Sender<ExportResult>,
    },
    GetSessions {
        response_sender: crossbeam_channel::Sender<Vec<String>>,
    },
    GetUnexportedSessions {
        response_sender: crossbeam_channel::Sender<Vec<String>>,
    },
    GetUsernames {
        response_sender: crossbeam_channel::Sender<Vec<String>>,
    },
    GetSessionsByUsername {
        username: String,
        response_sender: crossbeam_channel::Sender<Vec<String>>,
    },
    CheckExported {
        session_id: String,
        response_sender: crossbeam_channel::Sender<bool>,
    },
    LoadHistoryData {
        session_id: String,
        response_sender: crossbeam_channel::Sender<(Vec<DataPoint>, Vec<f64>)>,
    },
    LoadAlignedHistoryData {
        session_id: String,
        response_sender: crossbeam_channel::Sender<(Vec<DataPoint>, Vec<f64>, i64)>,
    },
}

/// Export type specification
#[derive(Clone, Debug)]
pub enum ExportType {
    SelectedSessions(Vec<String>),
    NewSessions,
}

impl ExportType {
    pub fn selected(session_ids: Vec<String>) -> Self {
        Self::SelectedSessions(session_ids)
    }

    pub fn new_only() -> Self {
        Self::NewSessions
    }
}
