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

    // 执行数据对齐算法（同时处理加速度计和音频数据）
    let (aligned_acc_data, trimmed_audio_data, common_time_range_ms) = align_session_data_internal(&acc_data, &audio_data);

    // 确保导出目录存在
    let export_dir = "data_export";
    if let Err(e) = std::fs::create_dir_all(export_dir) {
        return Err(format!("Failed to create export directory: {}", e));
    }

    // 创建CSV文件
    let filename = format!("{}/{}.csv", export_dir, session_id);
    let mut file = std::fs::File::create(&filename)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    // 写入CSV头部
    writeln!(file, "acc_x,acc_y,acc_z,audio_sample")
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // 收集所有音频样本到一个向量中
    let mut all_audio_samples: Vec<f64> = Vec::new();
    for (_start_timestamp, _end_timestamp, samples, _sample_rate, _channels, _format) in &trimmed_audio_data {
        all_audio_samples.extend(samples);
    }
    
    let acc_count = aligned_acc_data.len();
    let audio_count = all_audio_samples.len();
    let max_rows = acc_count.max(audio_count);
    let min_rows = acc_count.min(audio_count);
    
    let mut row_count = 0;
    
    // 前min_rows行：同时写入加速度计和音频数据
    for i in 0..min_rows {
        let point = &aligned_acc_data[i];
        let audio_sample = all_audio_samples[i];
        writeln!(file, "{},{},{},{}", point.x, point.y, point.z, audio_sample)
            .map_err(|e| format!("Failed to write combined data: {}", e))?;
        row_count += 1;
    }
    
    // 剩余行：只写入数据多的那一种，另一种不补0
    if acc_count > audio_count {
        // 加速度计数据更多，继续写入剩余的加速度计数据
        for i in min_rows..acc_count {
            let point = &aligned_acc_data[i];
            writeln!(file, "{},{},{},", point.x, point.y, point.z)
                .map_err(|e| format!("Failed to write remaining ACC data: {}", e))?;
            row_count += 1;
        }
    } else if audio_count > acc_count {
        // 音频数据更多，继续写入剩余的音频数据
        for i in min_rows..audio_count {
            let audio_sample = all_audio_samples[i];
            writeln!(file, ",,{}", audio_sample)
                .map_err(|e| format!("Failed to write remaining audio data: {}", e))?;
            row_count += 1;
        }
    }

    info!("Successfully exported session {} to {} ({} rows, common time range: {}ms)", 
          session_id, filename, row_count, common_time_range_ms);
    Ok(())
}

/// 内部对齐算法（在数据库线程中运行）
/// 基于共同时间戳范围进行对齐，只保留重叠时间范围内的数据
/// 通过采样率和时间戳差值去除多余的数据点
pub fn align_session_data_internal(
    acc_data: &[DataPoint], 
    audio_data: &[(i64, i64, Vec<f64>, u32, u8, String)]
) -> (Vec<DataPoint>, Vec<(i64, i64, Vec<f64>, u32, u8, String)>, i64) {
    if acc_data.is_empty() || audio_data.is_empty() {
        return (acc_data.to_vec(), audio_data.to_vec(), 0);
    }

    // 计算加速度数据的时间范围
    let acc_start_timestamp = acc_data.first().map(|p| p.timestamp).unwrap_or(0);
    let acc_end_timestamp = acc_data.last().map(|p| p.timestamp).unwrap_or(0);
    
    // 计算音频数据的时间范围
    let audio_start_timestamp = audio_data.first().map(|(start, _, _, _, _, _)| *start).unwrap_or(0);
    let audio_end_timestamp = audio_data.last().map(|(_, end, _, _, _, _)| *end).unwrap_or(0);
    
    // 计算共同时间范围
    let common_start = acc_start_timestamp.max(audio_start_timestamp);
    let common_end = acc_end_timestamp.min(audio_end_timestamp);
    
    info!("Time range analysis:");
    info!("  ACC: {} to {} (duration: {}ms)", acc_start_timestamp, acc_end_timestamp, acc_end_timestamp - acc_start_timestamp);
    info!("  Audio: {} to {} (duration: {}ms)", audio_start_timestamp, audio_end_timestamp, audio_end_timestamp - audio_start_timestamp);
    info!("  Common: {} to {} (duration: {}ms)", common_start, common_end, common_end - common_start);
    
    if common_start >= common_end {
        info!("No overlapping time range found, returning original data");
        return (acc_data.to_vec(), audio_data.to_vec(), 0);
    }
    
    // 计算需要裁剪的数据量
    let acc_excess_start_ms = common_start - acc_start_timestamp;
    let acc_excess_end_ms = acc_end_timestamp - common_end;
    let audio_excess_start_ms = common_start - audio_start_timestamp;
    let audio_excess_end_ms = audio_end_timestamp - common_end;
    
    info!("Excess data to trim:");
    info!("  ACC: start={}ms, end={}ms", acc_excess_start_ms, acc_excess_end_ms);
    info!("  Audio: start={}ms, end={}ms", audio_excess_start_ms, audio_excess_end_ms);
    
    // 估算加速度计采样率（基于现有数据）
    let acc_sample_rate = if acc_data.len() > 1 {
        let total_time_ms = acc_end_timestamp - acc_start_timestamp;
        if total_time_ms > 0 {
            (acc_data.len() - 1) as f64 * 1000.0 / total_time_ms as f64
        } else {
            400.0 // 默认采样率
        }
    } else {
        400.0
    };
    
    info!("Estimated ACC sample rate: {:.2} Hz", acc_sample_rate);
    
    // 根据时间范围裁剪加速度数据
    let mut trimmed_acc_data: Vec<DataPoint> = acc_data.iter()
        .filter(|point| point.timestamp >= common_start && point.timestamp <= common_end)
        .cloned()
        .collect();
        
    // 如果需要更精确的裁剪（根据采样率计算需要去除的点数）
    if acc_excess_end_ms > 0 && !trimmed_acc_data.is_empty() {
        let points_to_remove = ((acc_excess_end_ms as f64 * acc_sample_rate) / 1000.0).round() as usize;
        let points_to_keep = trimmed_acc_data.len().saturating_sub(points_to_remove);
        trimmed_acc_data.truncate(points_to_keep);
        info!("Removed {} ACC points from end ({}ms * {:.2}Hz)", points_to_remove, acc_excess_end_ms, acc_sample_rate);
    }
    
    if acc_excess_start_ms > 0 && !trimmed_acc_data.is_empty() {
        let points_to_remove = ((acc_excess_start_ms as f64 * acc_sample_rate) / 1000.0).round() as usize;
        let points_to_remove = points_to_remove.min(trimmed_acc_data.len());
        trimmed_acc_data.drain(0..points_to_remove);
        info!("Removed {} ACC points from start ({}ms * {:.2}Hz)", points_to_remove, acc_excess_start_ms, acc_sample_rate);
    }
    
    // ===== 音频数据处理 =====
    info!("Processing audio data alignment...");
    
    // 将所有音频数据合并成一个连续的样本数组进行裁剪
    let mut all_samples = Vec::new();
    let mut audio_sample_rate = 16000u32; // 默认采样率
    let mut audio_channels = 1u8;
    let mut audio_format = "PCM_16".to_string();
    
    // 合并所有音频块的样本
    for (_, _, samples, sample_rate, channels, format) in audio_data {
        all_samples.extend(samples);
        audio_sample_rate = *sample_rate;
        audio_channels = *channels;
        audio_format = format.clone();
    }
    
    info!("Total audio samples before trimming: {}, sample rate: {}Hz", 
          all_samples.len(), audio_sample_rate);
    
    let mut trimmed_audio_samples = all_samples;
    
    // 从开始处删除多余的样本点（如果开始时间早于共同开始时间）
    if audio_excess_start_ms > 0 {
        let samples_to_remove_start = ((audio_excess_start_ms as f64 * audio_sample_rate as f64) / 1000.0).round() as usize;
        let samples_to_remove = samples_to_remove_start.min(trimmed_audio_samples.len());
        trimmed_audio_samples.drain(0..samples_to_remove);
        info!("Removed {} audio samples from start ({}ms * {}Hz)", 
              samples_to_remove, audio_excess_start_ms, audio_sample_rate);
    }
    
    // 从结束处删除多余的样本点（如果结束时间晚于共同结束时间）
    if audio_excess_end_ms > 0 && !trimmed_audio_samples.is_empty() {
        let samples_to_remove_end = ((audio_excess_end_ms as f64 * audio_sample_rate as f64) / 1000.0).round() as usize;
        let samples_to_keep = trimmed_audio_samples.len().saturating_sub(samples_to_remove_end);
        trimmed_audio_samples.truncate(samples_to_keep);
        info!("Removed {} audio samples from end ({}ms * {}Hz)", 
              samples_to_remove_end, audio_excess_end_ms, audio_sample_rate);
    }
    
    info!("Audio samples after trimming: {}", trimmed_audio_samples.len());
    
    // 创建裁剪后的音频数据
    let trimmed_audio_data = if trimmed_audio_samples.is_empty() {
        Vec::new()
    } else {
        vec![(
            common_start,
            common_end,
            trimmed_audio_samples,
            audio_sample_rate,
            audio_channels,
            audio_format
        )]
    };
    
    let alignment_info = common_end - common_start;
    info!("Data alignment completed: common time range {}ms, {} ACC points retained, {} audio blocks created", 
          alignment_info, trimmed_acc_data.len(), trimmed_audio_data.len());
    
    (trimmed_acc_data, trimmed_audio_data, alignment_info)
}

