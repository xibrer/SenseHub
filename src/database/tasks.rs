use std::io::Write;
use log::info;

use crate::types::{DataPoint, AudioData};
use super::manager::DatabaseManager;

/// 内部导出函数（在数据库线程中运行）
pub fn export_session_to_csv_internal(db_manager: &DatabaseManager, session_id: &str) -> Result<(), String> {
    // 获取session对应的用户名
    let username = db_manager.get_username_for_session(session_id)
        .map_err(|e| format!("Failed to get username for session: {}", e))?;
    
    // 获取session对应的场景
    let scenario = db_manager.get_scenario_for_session(session_id)
        .map_err(|e| format!("Failed to get scenario for session: {}", e))?;
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

    // 确保基础导出目录存在
    let base_export_dir = "data_export";
    if let Err(e) = std::fs::create_dir_all(base_export_dir) {
        return Err(format!("Failed to create base export directory: {}", e));
    }

    // 创建用户名目录（如果用户名为空，则使用 "unknown_user"）
    let user_dir = if username.is_empty() {
        "unknown_user"
    } else {
        &username
    };
    
    // 创建场景目录（如果场景为空，则使用 "standard"）
    let scenario_dir = if scenario.is_empty() {
        "standard"
    } else {
        &scenario
    };
    
    let export_dir = format!("{}/{}/{}", base_export_dir, user_dir, scenario_dir);
    if let Err(e) = std::fs::create_dir_all(&export_dir) {
        return Err(format!("Failed to create user/scenario export directory: {}", e));
    }

    // 创建CSV文件
    let filename = format!("{}/{}.csv", export_dir, session_id);
    let mut file = std::fs::File::create(&filename)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    // 写入CSV头部
    writeln!(file, "acc_x,acc_y,acc_z,gyro_x,gyro_y,gyro_z,audio_sample")
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // 收集所有音频样本到一个向量中
    let mut all_audio_samples: Vec<f64> = Vec::new();
    for (_start_timestamp, _end_timestamp, samples, _sample_rate, _channels, _format) in &trimmed_audio_data {
        all_audio_samples.extend(samples);
    }

    let acc_count = aligned_acc_data.len();
    let audio_count = all_audio_samples.len();
    let min_rows = acc_count.min(audio_count);

    let mut row_count = 0;

    // 前min_rows行：同时写入加速度计和音频数据
    for i in 0..min_rows {
        let point = &aligned_acc_data[i];
        let audio_sample = all_audio_samples[i];
        writeln!(file, "{},{},{},{},{},{},{}", point.x, point.y, point.z, point.gx, point.gy, point.gz, audio_sample)
            .map_err(|e| format!("Failed to write combined data: {}", e))?;
        row_count += 1;
    }

    // 剩余行：只写入数据多的那一种，另一种不补0
    if acc_count > audio_count {
        // 加速度计数据更多，继续写入剩余的加速度计数据
        for i in min_rows..acc_count {
            let point = &aligned_acc_data[i];
            writeln!(file, "{},{},{},{},{},{},", point.x, point.y, point.z, point.gx, point.gy, point.gz)
                .map_err(|e| format!("Failed to write remaining ACC data: {}", e))?;
            row_count += 1;
        }
    } else if audio_count > acc_count {
        // 音频数据更多，继续写入剩余的音频数据
        for i in min_rows..audio_count {
            let audio_sample = all_audio_samples[i];
            writeln!(file, ",,,,,,{}", audio_sample)
                .map_err(|e| format!("Failed to write remaining audio data: {}", e))?;
            row_count += 1;
        }
    }

    info!("Successfully exported session {} for user '{}' in scenario '{}' to {} ({} rows, common time range: {}ms)", 
          session_id, user_dir, scenario_dir, filename, row_count, common_time_range_ms);
    Ok(())
}

/// 内部对齐算法（在数据库线程中运行）
/// 以音频为基准，通过插值和移动来对齐加速度数据
pub fn align_session_data_internal(
    acc_data: &[DataPoint],
    audio_data: &[(i64, i64, Vec<f64>, u32, u8, String)]
) -> (Vec<DataPoint>, Vec<(i64, i64, Vec<f64>, u32, u8, String)>, i64) {
    if acc_data.is_empty() || audio_data.is_empty() {
        info!("Empty data provided, returning original data");
        return (acc_data.to_vec(), audio_data.to_vec(), 0);
    }

    // 获取初始和最后一个数据点的时间戳
    let acc_first_timestamp = acc_data.first().map(|p| p.timestamp).unwrap_or(0);
    let acc_last_timestamp = acc_data.last().map(|p| p.timestamp).unwrap_or(0);
    let acc_duration_ms = acc_last_timestamp - acc_first_timestamp;

    let audio_first_timestamp = audio_data.first().map(|(start, _, _, _, _, _)| *start).unwrap_or(0);
    let audio_last_timestamp = audio_data.last().map(|(_, end, _, _, _, _)| *end).unwrap_or(0);
    let audio_duration_ms = audio_last_timestamp - audio_first_timestamp;

    info!("Timestamp-based alignment:");
    info!("  ACC initial timestamp: {}, final timestamp: {}, duration: {}ms", 
          acc_first_timestamp, acc_last_timestamp, acc_duration_ms);
    info!("  Audio initial timestamp: {}, final timestamp: {}, duration: {}ms", 
          audio_first_timestamp, audio_last_timestamp, audio_duration_ms);

    // 计算时间差（以音频为基准）
    let time_diff_ms = audio_last_timestamp - acc_last_timestamp;
    info!("  Time difference (audio - acc): {}ms", time_diff_ms);

    // 估算加速度采样率
    let acc_sample_rate = if acc_data.len() > 1 {
        let acc_duration_ms = acc_data.last().unwrap().timestamp - acc_data.first().unwrap().timestamp;
        if acc_duration_ms > 0 {
            (acc_data.len() - 1) as f64 * 1000.0 / acc_duration_ms as f64
        } else {
            400.0 // 默认采样率
        }
    } else {
        400.0
    };
    info!("  Estimated ACC sample rate: {:.2} Hz", acc_sample_rate);

    // 计算需要移动的加速度数据点数
    let shift_samples = (time_diff_ms as f64 * acc_sample_rate / 1000.0).round() as i32;
    info!("  ACC data shift: {} samples ({}ms * {:.2}Hz)", shift_samples, time_diff_ms, acc_sample_rate);

    // 创建对齐后的加速度数据
    let aligned_acc_data = if shift_samples == 0 {
        // 不需要移动，直接返回原数据
        acc_data.to_vec()
    } else if shift_samples > 0 {
        // 音频的最后时间戳更大，需要从acc末尾去掉点数，在开头补第一个数据的值
        let shift_count = shift_samples as usize;
        let mut aligned_data = Vec::new();

        if let Some(first_point) = acc_data.first() {
            let sample_interval_ms = 1000.0 / acc_sample_rate;
            // 在开头补去掉个数的acc第一个数据的值
            for i in 0..shift_count {
                let timestamp = first_point.timestamp - ((shift_count - i) as f64 * sample_interval_ms) as i64;
                aligned_data.push(DataPoint {
                    x: first_point.x,  // 使用第一个点的x值
                    y: first_point.y,  // 使用第一个点的y值
                    z: first_point.z,  // 使用第一个点的z值
                    gx: first_point.gx, // 使用第一个点的gx值
                    gy: first_point.gy, // 使用第一个点的gy值
                    gz: first_point.gz, // 使用第一个点的gz值
                    timestamp,
                });
            }
        }

        // 添加原始数据，但去掉末尾的点数
        let end_index = if acc_data.len() > shift_count {
            acc_data.len() - shift_count
        } else {
            0
        };
        aligned_data.extend_from_slice(&acc_data[..end_index]);

        info!("  Removed {} points from end, added {} padding points (using first point values) at the beginning", 
              shift_count.min(acc_data.len()), shift_count);
        aligned_data
    } else {
        // 音频的最后时间戳更小，需要从acc开头去掉点数，在末尾补最后一个数据的值
        let shift_count = (-shift_samples) as usize;
        let mut aligned_data = Vec::new();

        // 去掉开头的点数
        let start_index = shift_count.min(acc_data.len());
        aligned_data.extend_from_slice(&acc_data[start_index..]);

        // 在末尾补最后一个数据的值
        if let Some(last_point) = acc_data.last() {
            let sample_interval_ms = 1000.0 / acc_sample_rate;
            for i in 1..=shift_count {
                let timestamp = last_point.timestamp + (i as f64 * sample_interval_ms) as i64;
                aligned_data.push(DataPoint {
                    x: last_point.x,   // 使用最后一个点的x值
                    y: last_point.y,   // 使用最后一个点的y值
                    z: last_point.z,   // 使用最后一个点的z值
                    gx: last_point.gx,  // 使用最后一个点的gx值
                    gy: last_point.gy,  // 使用最后一个点的gy值
                    gz: last_point.gz,  // 使用最后一个点的gz值
                    timestamp,
                });
            }
        }

        info!("  Removed {} points from beginning, added {} padding points (using last point values) at the end", 
              shift_count.min(acc_data.len()), shift_count);
        aligned_data
    };

    // 合并所有音频数据到一个连续的向量
    let mut all_audio_samples = Vec::new();
    let mut audio_sample_rate = 16000u32;
    let mut audio_channels = 1u8;
    let mut audio_format = "PCM_16".to_string();

    for (_, _, samples, sample_rate, channels, format) in audio_data {
        all_audio_samples.extend(samples);
        audio_sample_rate = *sample_rate;
        audio_channels = *channels;
        audio_format = format.clone();
    }

    // 创建对齐后的音频数据（保持原格式）
    let aligned_audio_data = if all_audio_samples.is_empty() {
        Vec::new()
    } else {
        // 使用音频数据的原始时间范围
        let audio_start = audio_data.first().map(|(start, _, _, _, _, _)| *start).unwrap_or(0);
        let audio_end = audio_data.last().map(|(_, end, _, _, _, _)| *end).unwrap_or(0);

        vec![(
            audio_start,
            audio_end,
            all_audio_samples,
            audio_sample_rate,
            audio_channels,
            audio_format
        )]
    };

    let alignment_info = time_diff_ms.abs();
    info!("Alignment completed: {} ACC points, {} audio samples, alignment offset: {}ms", 
          aligned_acc_data.len(), 
          aligned_audio_data.first().map(|(_, _, samples, _, _, _)| samples.len()).unwrap_or(0),
          alignment_info);

    (aligned_acc_data, aligned_audio_data, alignment_info)
}


