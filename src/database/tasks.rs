use std::io::Write;
use log::info;

use crate::types::{DataPoint, AudioData};
use super::manager::DatabaseManager;

/// 内部导出函数（在数据库线程中运行）
pub fn export_session_to_csv_internal(db_manager: &DatabaseManager, session_id: &str) -> Result<(), String> {
    // 获取加速度数据
    let acc_data = db_manager.get_accelerometer_data_by_session(session_id)
        .map_err(|e| format!("Failed to get accelerometer data: {}", e))?;

    // 获取音频数据
    let audio_data = db_manager.get_audio_data_by_session(session_id)
        .map_err(|e| format!("Failed to get audio data: {}", e))?;

    if acc_data.is_empty() && audio_data.is_empty() {
        return Err("No data in session".to_string());
    }

    // 执行数据对齐算法
    let (aligned_acc_data, alignment_offset_ms) = align_session_data_internal(&acc_data, &audio_data);

    // 创建CSV文件
    let filename = format!("{}.csv", session_id);
    let mut file = std::fs::File::create(&filename)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    // 写入CSV头部
    writeln!(file, "timestamp_ms,acc_x,acc_y,acc_z,audio_sample")
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // 合并对齐后的加速度和音频数据，按时间戳排序
    let mut combined_data: Vec<(i64, Option<(f64, f64, f64)>, Option<f64>)> = Vec::new();

    // 添加对齐后的加速度数据
    for point in &aligned_acc_data {
        combined_data.push((point.timestamp, Some((point.x, point.y, point.z)), None));
    }

    // 添加音频数据（展开所有样本）
    for (timestamp, samples, sample_rate, _channels, _format) in &audio_data {
        let sample_interval_ms = 1000.0 / (*sample_rate as f64);
        for (i, &sample) in samples.iter().enumerate() {
            let sample_timestamp = timestamp + (i as f64 * sample_interval_ms) as i64;
            combined_data.push((sample_timestamp, None, Some(sample)));
        }
    }

    // 按时间戳排序
    combined_data.sort_by_key(|&(timestamp, _, _)| timestamp);

    // 写入数据行
    let mut row_count = 0;
    for (timestamp, acc_data_opt, audio_sample) in combined_data {
        let (acc_x, acc_y, acc_z) = acc_data_opt.unwrap_or((0.0, 0.0, 0.0));
        let audio = audio_sample.unwrap_or(0.0);
        
        writeln!(file, "{},{},{},{},{}", timestamp, acc_x, acc_y, acc_z, audio)
            .map_err(|e| format!("Failed to write CSV data: {}", e))?;
        row_count += 1;
    }

    info!("Successfully exported session {} to {} ({} rows, alignment offset: {}ms)", 
          session_id, filename, row_count, alignment_offset_ms);
    Ok(())
}

/// 内部对齐算法（在数据库线程中运行）
pub fn align_session_data_internal(
    acc_data: &[DataPoint], 
    audio_data: &[(i64, Vec<f64>, u32, u8, String)]
) -> (Vec<DataPoint>, i64) {
    if acc_data.is_empty() || audio_data.is_empty() {
        return (acc_data.to_vec(), 0);
    }

    // 计算加速度数据的结束时间戳
    let acc_end_timestamp = acc_data.last().map(|p| p.timestamp).unwrap_or(0);
    
    // 计算音频数据的结束时间戳（最后一个音频块的最后一个样本）
    let audio_end_timestamp = if let Some((last_timestamp, last_samples, sample_rate, _, _)) = audio_data.last() {
        let sample_duration_ms = (last_samples.len() as f64 * 1000.0) / (*sample_rate as f64);
        last_timestamp + sample_duration_ms as i64
    } else {
        0
    };

    // 计算时间差
    let time_diff_ms = audio_end_timestamp - acc_end_timestamp;
    
    info!("Data alignment: audio end {} - acc end {} = offset {}ms", 
          audio_end_timestamp, acc_end_timestamp, time_diff_ms);

    // 如果音频数据比加速度数据晚结束，需要调整加速度数据的时间戳
    let aligned_acc_data: Vec<DataPoint> = if time_diff_ms > 0 {
        // 音频晚于加速度，向前调整加速度时间戳
        acc_data.iter()
            .map(|point| DataPoint {
                x: point.x,
                y: point.y,
                z: point.z,
                timestamp: point.timestamp + time_diff_ms,
            })
            .collect()
    } else {
        // 加速度晚于或等于音频，保持加速度时间戳不变
        acc_data.to_vec()
    };

    (aligned_acc_data, time_diff_ms)
}
