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
                    DatabaseTask::Save { accelerometer_data, audio_data, audio_metadata, audio_start_timestamp, audio_end_timestamp, session_id, username, scenario } => {
                        handle_save_task(&db_manager, &result_sender, accelerometer_data, audio_data, audio_metadata, audio_start_timestamp, audio_end_timestamp, session_id, username, scenario);
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
                    DatabaseTask::GetUnexportedSessions { response_sender } => {
                        let sessions = db_manager.get_unexported_sessions().unwrap_or_default();
                        if let Err(e) = response_sender.try_send(sessions) {
                            warn!("Database handler: Failed to send unexported sessions: {}", e);
                        }
                    }
                    DatabaseTask::GetUsernames { response_sender } => {
                        let usernames = db_manager.get_all_usernames().unwrap_or_default();
                        if let Err(e) = response_sender.try_send(usernames) {
                            warn!("Database handler: Failed to send usernames: {}", e);
                        }
                    }
                    DatabaseTask::GetSessionsByUsername { username, response_sender } => {
                        let sessions = db_manager.get_sessions_by_username(&username).unwrap_or_default();
                        if let Err(e) = response_sender.try_send(sessions) {
                            warn!("Database handler: Failed to send sessions by username: {}", e);
                        }
                    }
                    DatabaseTask::CheckExported { session_id, response_sender } => {
                        let is_exported = db_manager.is_session_exported(&session_id).unwrap_or(false);
                        if let Err(e) = response_sender.try_send(is_exported) {
                            warn!("Database handler: Failed to send export status: {}", e);
                        }
                    }
                    DatabaseTask::LoadHistoryData { session_id, response_sender } => {
                        let result = handle_load_history_data(&db_manager, &session_id);
                        if let Err(e) = response_sender.try_send(result) {
                            warn!("Database handler: Failed to send history data: {}", e);
                        }
                    }
                    DatabaseTask::LoadAlignedHistoryData { session_id, response_sender } => {
                        let result = handle_load_aligned_history_data(&db_manager, &session_id);
                        if let Err(e) = response_sender.try_send(result) {
                            warn!("Database handler: Failed to send aligned history data: {}", e);
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
    audio_start_timestamp: Option<i64>,
    audio_end_timestamp: Option<i64>,
    session_id: String,
    username: String,
    scenario: String,
) {
    let mut acc_saved = 0;
    let mut audio_saved = 0;
    let mut error_msg = None;

    // 保存加速度数据
    if !accelerometer_data.is_empty() {
        match db_manager.save_accelerometer_data(&accelerometer_data, &session_id, &username, &scenario) {
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
        match db_manager.save_audio_data(&audio_data, audio_metadata.as_ref(), &session_id, audio_start_timestamp, audio_end_timestamp, &username) {
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
                info!("Successfully exported session: {}", session_id);
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
                // 检查是否已导出（通过文件系统检查）
                if !db_manager.is_session_exported(session_id).unwrap_or(false) {
                    match export_session_to_csv_internal(db_manager, session_id) {
                        Ok(()) => {
                            success_count += 1;
                            info!("Successfully exported new session: {}", session_id);
                        }
                        Err(e) => {
                            error_count += 1;
                            error!("Failed to export session {}: {}", session_id, e);
                        }
                    }
                } else {
                    info!("Session {} already exported, skipping", session_id);
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

fn handle_load_history_data(db_manager: &DatabaseManager, session_id: &str) -> (Vec<DataPoint>, Vec<f64>) {
    let mut acc_data = Vec::new();
    let mut audio_data = Vec::new();

    // 加载加速度数据
    match db_manager.get_accelerometer_data_by_session(session_id) {
        Ok(data) => {
            acc_data = data;
            info!("Database handler: Loaded {} accelerometer points for session {}", acc_data.len(), session_id);
        }
        Err(e) => {
            error!("Database handler: Failed to load accelerometer data for session {}: {}", session_id, e);
        }
    }

    // 加载音频数据
    match db_manager.get_audio_data_by_session(session_id) {
        Ok(data) => {
            // 将所有音频片段的样本合并到一个向量中
            for (_, _, samples, _, _, _) in data {
                audio_data.extend(samples);
            }
            info!("Database handler: Loaded {} audio samples for session {}", audio_data.len(), session_id);
        }
        Err(e) => {
            error!("Database handler: Failed to load audio data for session {}: {}", session_id, e);
        }
    }

    (acc_data, audio_data)
}

fn handle_load_aligned_history_data(db_manager: &DatabaseManager, session_id: &str) -> (Vec<DataPoint>, Vec<f64>, i64) {
    let mut acc_data = Vec::new();
    let mut audio_data_raw = Vec::new();

    // 加载原始加速度数据
    match db_manager.get_accelerometer_data_by_session(session_id) {
        Ok(data) => {
            acc_data = data;
            info!("Database handler: Loaded {} raw accelerometer points for session {}", acc_data.len(), session_id);
        }
        Err(e) => {
            error!("Database handler: Failed to load accelerometer data for session {}: {}", session_id, e);
        }
    }

    // 加载原始音频数据
    match db_manager.get_audio_data_by_session(session_id) {
        Ok(data) => {
            audio_data_raw = data;
            info!("Database handler: Loaded {} raw audio blocks for session {}", audio_data_raw.len(), session_id);
        }
        Err(e) => {
            error!("Database handler: Failed to load audio data for session {}: {}", session_id, e);
        }
    }

    // 如果没有数据，返回空结果
    if acc_data.is_empty() && audio_data_raw.is_empty() {
        return (Vec::new(), Vec::new(), 0);
    }

    // 使用对齐算法处理数据
    let (aligned_acc_data, aligned_audio_data, common_time_range_ms) =
        crate::database::tasks::align_session_data_internal(&acc_data, &audio_data_raw);

    // 将对齐后的音频数据合并到一个向量中
    let mut final_audio_data = Vec::new();
    for (_, _, samples, _, _, _) in aligned_audio_data {
        final_audio_data.extend(samples);
    }

    info!("Database handler: Aligned data - {} acc points, {} audio samples, {}ms common range", 
          aligned_acc_data.len(), final_audio_data.len(), common_time_range_ms);

    (aligned_acc_data, final_audio_data, common_time_range_ms)
}
