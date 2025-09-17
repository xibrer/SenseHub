use log::error;
use crate::app::app_core::SensorDataApp;
use crate::types::{DatabaseTask, ExportType};

pub struct ExportHandler;

impl ExportHandler {
    pub fn refresh_sessions(app: &mut SensorDataApp) {
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
        let task = DatabaseTask::GetAllSessionsWithExportStatus { response_sender };
        
        match app.state.database.db_task_sender.try_send(task) {
            Ok(()) => {
                app.state.export.export_status = "Refreshing sessions list...".to_string();
                app.state.export.sessions_result_receiver = Some(response_receiver);
            }
            Err(e) => {
                app.state.export.export_status = format!("Failed to request sessions list: {}", e);
            }
        }
    }

    pub fn export_selected_sessions(app: &mut SensorDataApp) {
        if app.state.export.selected_sessions.is_empty() {
            app.state.export.export_status = "Please select sessions to export first".to_string();
            return;
        }

        let session_ids: Vec<String> = app.state.export.selected_sessions.iter().cloned().collect();
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
        
        let task = DatabaseTask::Export {
            export_type: ExportType::SelectedSessions(session_ids),
            response_sender,
        };
        
        match app.state.database.db_task_sender.try_send(task) {
            Ok(()) => {
                app.state.export.export_status = "Exporting selected sessions...".to_string();
                app.state.export.selected_sessions.clear();
                app.state.export.export_result_receiver = Some(response_receiver);
            }
            Err(e) => {
                app.state.export.export_status = format!("Failed to start export: {}", e);
            }
        }
    }

    pub fn export_new_sessions_only(app: &mut SensorDataApp) {
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
        
        let task = DatabaseTask::Export {
            export_type: ExportType::NewSessions,
            response_sender,
        };
        
        match app.state.database.db_task_sender.try_send(task) {
            Ok(()) => {
                app.state.export.export_status = "Exporting new sessions...".to_string();
                app.state.export.export_result_receiver = Some(response_receiver);
            }
            Err(e) => {
                app.state.export.export_status = format!("Failed to start export: {}", e);
            }
        }
    }

}
