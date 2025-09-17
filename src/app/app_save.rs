use std::time::{Duration, Instant};
use log::{info, warn, error};

use crate::types::{DataPoint, DatabaseTask};
use super::app_core::SensorDataApp;

impl SensorDataApp {
    pub fn save_current_window_data_async(&mut self) {
        // 获取当前窗口的加速度数据
        let acc_data = self.state.waveform_plot.get_current_accelerometer_data();
        let audio_data = self.state.waveform_plot.get_current_audio_data();

        if acc_data.is_empty() && audio_data.is_empty() {
            self.state.collection.save_status = "No data to save".to_string();
            return;
        }

        // 转换加速度数据为DataPoint格式，使用发送过来的真实时间戳
        let acc_points: Vec<DataPoint> = acc_data
            .into_iter()
            .map(|(x, y, z, gx, gy, gz, timestamp)| DataPoint {
                x,
                y,
                z,
                gx,
                gy,
                gz,
                timestamp, // 直接使用发送过来的时间戳
            })
            .collect();

        // 获取当前窗口内第一个和最后一个音频数据点的时间戳
        let audio_start_timestamp = self.state.waveform_plot.get_current_audio_first_timestamp();
        let audio_end_timestamp = self.state.waveform_plot.get_current_audio_last_timestamp();

        // 创建保存任务
        let save_task = DatabaseTask::Save {
            accelerometer_data: acc_points,
            audio_data,
            audio_metadata: self.state.database.last_audio_metadata.clone(),
            audio_start_timestamp,
            audio_end_timestamp,
            session_id: self.state.collection.current_session_id.clone(),
            username: self.state.collection.username.clone(),
            scenario: self.state.collection.scenario.clone(),
        };

        // 发送保存任务到后台线程
        match self.state.database.db_task_sender.try_send(save_task) {
            Ok(()) => {
                self.state.collection.save_status = "Saving data...".to_string();
                info!("Save task sent to background thread");
            }
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                self.state.collection.save_status = "Database queue is full, try again later".to_string();
                warn!("Database task queue is full, task not sent");
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                self.state.collection.save_status = "Database connection lost! Please restart the application.".to_string();
                error!("Database task channel disconnected - database thread may have crashed");
            }
        }
    }

    /// 检查是否需要自动保存
    pub fn check_auto_save(&mut self) {
        if !self.state.collection.auto_save_enabled {
            return;
        }

        let now = Instant::now();

        // 初始化自动保存时间
        if self.state.collection.auto_save_last_time.is_none() {
            self.state.collection.auto_save_last_time = Some(now);
            return;
        }

        let last_save_time = self.state.collection.auto_save_last_time.unwrap();
        let elapsed = now.duration_since(last_save_time);
        let interval_duration = Duration::from_millis(self.state.collection.auto_save_interval_ms);

        // 检查是否到了保存时间
        if elapsed >= interval_duration {
            // 执行自动保存
            self.save_current_window_data_async();
            self.state.collection.auto_save_count += 1;
            self.state.collection.auto_save_last_time = Some(now);

            info!("Auto-save triggered (count: {})", self.state.collection.auto_save_count);
        }
    }

    /// 启用/禁用自动保存
    pub fn toggle_auto_save(&mut self) {
        self.state.collection.auto_save_enabled = !self.state.collection.auto_save_enabled;

        if self.state.collection.auto_save_enabled {
            // 启用时重置计时器
            self.state.collection.auto_save_last_time = Some(Instant::now());
            self.state.collection.auto_save_count = 0;
            info!("Auto-save enabled with interval: {}ms", self.state.collection.auto_save_interval_ms);
        } else {
            self.state.collection.auto_save_last_time = None;
            info!("Auto-save disabled");
        }
    }
}