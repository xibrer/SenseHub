use std::time::{Duration, UNIX_EPOCH};

/// 将毫秒时间戳格式化为标准时间格式 HH:MM:SS.mmm
pub fn format_timestamp(timestamp_ms: i64) -> String {
    let duration = Duration::from_millis(timestamp_ms as u64);
    
    match UNIX_EPOCH.checked_add(duration) {
        Some(system_time) => {
            match system_time.duration_since(UNIX_EPOCH) {
                Ok(d) => {
                    let total_ms = d.as_millis();
                    let seconds = total_ms / 1000;
                    let ms = total_ms % 1000;
                    
                    // 简化格式：只显示时分秒.毫秒
                    let secs_since_epoch = seconds;
                    let hours = (secs_since_epoch / 3600) % 24;
                    let minutes = (secs_since_epoch / 60) % 60;
                    let secs = secs_since_epoch % 60;
                    
                    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, secs, ms)
                }
                Err(_) => format!("Invalid timestamp: {}", timestamp_ms)
            }
        }
        None => format!("Invalid timestamp: {}", timestamp_ms)
    }
}
