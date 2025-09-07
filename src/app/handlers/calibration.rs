use log::info;
use crate::app::sensor_app::SensorDataApp;
use crate::types::DataPoint;

pub struct CalibrationHandler;

impl CalibrationHandler {
    pub fn handle_calibration(app: &mut SensorDataApp) {
        // 校准模式：收集timestamp数据
        while let Ok(data) = app.state.channels.data_receiver.try_recv() {
            Self::process_calibration_data(app, data);
        }
        
        // 检查是否达到5秒
        if let Some(start_time) = app.state.calibration.calibration_start_time {
            let elapsed = start_time.elapsed();
            if elapsed.as_secs_f64() >= 5.0 && !app.state.calibration.calibration_data.is_empty() {
                Self::calculate_sample_rate_from_timestamps(app);
            }
        }
        
        // 校准期间丢弃音频数据
        while let Ok(_) = app.state.channels.audio_receiver.try_recv() {
            // 丢弃音频数据
        }
    }
    
    fn process_calibration_data(app: &mut SensorDataApp, data: DataPoint) {
        // 如果这是第一个样本，开始计时
        if app.state.calibration.calibration_start_time.is_none() {
            app.state.calibration.calibration_start_time = Some(std::time::Instant::now());
            info!("收到第一个样本，开始校准计时");
        }
        
        app.state.calibration.calibration_data.push(data);
    }
    
    fn calculate_sample_rate_from_timestamps(app: &mut SensorDataApp) {
        if app.state.calibration.calibration_data.len() < 2 {
            app.state.calibration.is_calibrating = false;
            return;
        }

        // 使用时间戳计算采样率
        let first_timestamp = app.state.calibration.calibration_data.first().unwrap().timestamp;
        let last_timestamp = app.state.calibration.calibration_data.last().unwrap().timestamp;
        let time_diff_ms = last_timestamp - first_timestamp;
        let sample_count = app.state.calibration.calibration_data.len() as f64;
        
        if time_diff_ms > 0 {
            let sample_rate = (sample_count - 1.0) * 1000.0 / time_diff_ms as f64;
            
            info!("校准完成: {} 个样本, 时间差 {}ms, 计算采样率: {:.2} Hz", 
                  sample_count, time_diff_ms, sample_rate);
            
            // 使用新的状态管理方法完成校准
            app.state.complete_calibration(sample_rate);
            
            info!("开始正常数据采集模式");
        } else {
            info!("校准失败：时间戳差值为0或负数");
            app.state.calibration.is_calibrating = false;
        }
    }
}
