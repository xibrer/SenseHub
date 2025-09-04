use crate::database::export_session_to_csv_internal;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use crossbeam_channel::{Receiver, Sender};
use log::{info, error, warn};
use std::io::Write;

use crate::types::{DatabaseTask, ExportType, ExportResult, SaveResult, DataPoint, AudioData};
use super::manager::DatabaseManager;

pub fn run_database_handler(
    task_receiver: Receiver<DatabaseTask>,
    result_sender: Sender<SaveResult>,
    shutdown_signal: Arc<AtomicBool>
) -> Result<(), Box<dyn std::error::Error>> {
    // 在保存线程中创建数据库连接
    let db_manager = match DatabaseManager::new() {
        Ok(db) => {
            info!("Database handler thread: DuckDB initialized successfully");
            db
        }
        Err(e) => {
            error!("Database handler thread: Failed to initialize DuckDB: {}", e);
            return Err(e.into());
        }
    };

    info!("Database handler thread started");

    while !shutdown_signal.load(Ordering::Relaxed) {
        match task_receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(task) => {
                match task {
                    DatabaseTask::Save { accelerometer_data, audio_data, audio_metadata, session_id } => {
                        handle_save_task(&db_manager, &result_sender, accelerometer_data, audio_data, audio_metadata, session_id);
                    }
                    DatabaseTask::Export { export_type, response_sender } => {
                        let result = handle_export_request(&db_manager, export_type);
                        if let Err(e) = response_sender.try_send(result) {
                            warn!("Database handler: Failed to send export result: {}", e);
                        }
                    }
                    DatabaseTask::GetSessions { response_sender } => {
                        let sessions = db_manager.get_all_sessions().unwrap_or_default();
                        if let Err(e) = response_sender.try_send(sessions) {
                            warn!("Database handler: Failed to send sessions: {}", e);
                        }
                    }
                    DatabaseTask::CheckExported { session_id, response_sender } => {
                        let is_exported = db_manager.is_session_exported(&session_id).unwrap_or(false);
                        if let Err(e) = response_sender.try_send(is_exported) {
                            warn!("Database handler: Failed to send export status: {}", e);
                        }
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // 超时，继续循环检查关闭信号
                continue;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                // 通道断开，退出循环
                info!("Database handler: Task channel disconnected, exiting");
                break;
            }
        }
    }

    info!("Database handler thread exiting gracefully");
    Ok(())
}

fn handle_save_task(
    db_manager: &DatabaseManager,
    result_sender: &Sender<SaveResult>,
    accelerometer_data: Vec<DataPoint>,
    audio_data: Vec<f64>,
    audio_metadata: Option<AudioData>,
    session_id: String,
) {
    let mut acc_saved = 0;
    let mut audio_saved = 0;
    let mut error_msg = None;

    // 保存加速度数据
    if !accelerometer_data.is_empty() {
        match db_manager.save_accelerometer_data(&accelerometer_data, &session_id) {
            Ok(count) => {
                acc_saved = count;
                info!("Database handler: Saved {} accelerometer data points", count);
            }
            Err(e) => {
                error!("Database handler: Failed to save accelerometer data: {}", e);
                error_msg = Some(format!("Error saving acc data: {}", e));
            }
        }
    }

    // 保存音频数据
    if !audio_data.is_empty() && error_msg.is_none() {
        match db_manager.save_audio_data(&audio_data, audio_metadata.as_ref(), &session_id) {
            Ok(count) => {
                audio_saved = count;
                info!("Database handler: Saved {} audio records", count);
            }
            Err(e) => {
                error!("Database handler: Failed to save audio data: {}", e);
                error_msg = Some(format!("Error saving audio data: {}", e));
            }
        }
    }

    // 发送保存结果
    let result = SaveResult {
        acc_saved,
        audio_saved,
        error: error_msg,
    };

    if let Err(_) = result_sender.send(result) {
        // GUI已关闭，退出循环
        info!("Database handler: Result channel disconnected, exiting");
    }
}

pub fn handle_export_request(db_manager: &DatabaseManager, export_type: ExportType) -> ExportResult {
    match export_type {
        ExportType::SelectedSessions(session_ids) => {
            handle_selected_sessions_export(db_manager, session_ids)
        }
        ExportType::NewSessions => {
            handle_new_sessions_export(db_manager)
        }
    }
}

fn handle_selected_sessions_export(db_manager: &DatabaseManager, session_ids: Vec<String>) -> ExportResult {
    let mut success_count = 0;
    let mut error_count = 0;

    for session_id in &session_ids {
        match export_session_to_csv_internal(db_manager, session_id) {
            Ok(()) => {
                success_count += 1;
                // 标记为已导出
                if let Err(e) = db_manager.mark_session_exported(session_id) {
                    warn!("Failed to mark session as exported: {}", e);
                }
            }
            Err(e) => {
                error_count += 1;
                error!("Failed to export session {}: {}", session_id, e);
            }
        }
    }

    ExportResult {
        success_count,
        error_count,
        message: format!("Export completed: {} succeeded, {} failed", success_count, error_count),
    }
}

fn handle_new_sessions_export(db_manager: &DatabaseManager) -> ExportResult {
    let mut success_count = 0;
    let mut error_count = 0;

    match db_manager.get_all_sessions() {
        Ok(sessions) => {
            for session_id in &sessions {
                // 检查是否已导出
                if !db_manager.is_session_exported(session_id).unwrap_or(false) {
                    match export_session_to_csv_internal(db_manager, session_id) {
                        Ok(()) => {
                            success_count += 1;
                            // 标记为已导出
                            if let Err(e) = db_manager.mark_session_exported(session_id) {
                                warn!("Failed to mark session as exported: {}", e);
                            }
                        }
                        Err(e) => {
                            error_count += 1;
                            error!("Failed to export session {}: {}", session_id, e);
                        }
                    }
                }
            }

            if success_count == 0 && error_count == 0 {
                ExportResult {
                    success_count: 0,
                    error_count: 0,
                    message: "No new sessions to export".to_string(),
                }
            } else {
                ExportResult {
                    success_count,
                    error_count,
                    message: format!("New sessions export completed: {} succeeded, {} failed", success_count, error_count),
                }
            }
        }
        Err(e) => ExportResult {
            success_count: 0,
            error_count: 1,
            message: format!("Failed to get sessions: {}", e),
        }
    }
}
