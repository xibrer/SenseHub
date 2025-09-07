use std::time::Duration;
use eframe::{egui, Frame};
use log::{info, error};

use crate::types::{DataPoint, AudioData, DatabaseTask, SaveResult};
use crate::database::generate_session_id;
use crate::config::ConfigManager;
use super::state::AppState;

pub struct SensorDataApp {
    // 统一的状态管理
    pub state: AppState,
    
    // 配置管理
    pub config: ConfigManager,
}

impl SensorDataApp {
    pub fn new(
        data_receiver: crossbeam_channel::Receiver<DataPoint>, 
        audio_receiver: crossbeam_channel::Receiver<AudioData>,
        db_task_sender: crossbeam_channel::Sender<DatabaseTask>,
        save_result_receiver: crossbeam_channel::Receiver<SaveResult>
    ) -> Self {
        // 创建配置管理器
        let config = ConfigManager::new();
        
        // 创建应用状态
        let mut state = AppState::new(
            data_receiver,
            audio_receiver,
            db_task_sender,
            save_result_receiver,
        );
        
        // 初始化会话ID
        state.collection.current_session_id = generate_session_id();
        
        let app = SensorDataApp {
            state,
            config,
        };
        
        // 打印启动信息
        info!("应用启动，等待数据到达开始校准...");
        
        app
    }
}

impl eframe::App for SensorDataApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 设置明亮模式主题
        ctx.set_visuals(egui::Visuals::light());
        
        // 渲染UI组件
        crate::app::ui::render_status_bar(self, ctx);
        crate::app::ui::render_main_panel(self, ctx);
        crate::app::ui::render_export_dialog(self, ctx);
        
        // 处理各种结果
        self.handle_save_results();
        self.handle_export_results();
        self.handle_sessions_results();
        
        // 处理数据：校准、采集或丢弃
        self.handle_data_processing();
        
        // 处理键盘输入
        self.handle_keyboard_input(ctx);

        ctx.request_repaint_after(Duration::from_millis(120));
    }
}

impl SensorDataApp {
    fn handle_save_results(&mut self) {
        while let Ok(result) = self.state.database.save_result_receiver.try_recv() {
            if let Some(error) = result.error {
                self.state.collection.save_status = error;
            } else if result.acc_saved > 0 || result.audio_saved > 0 {
                self.state.collection.save_status = format!("Saved: {} ACC points, {} audio records", result.acc_saved, result.audio_saved);
                info!("Data saved successfully: {} ACC, {} audio", result.acc_saved, result.audio_saved);
                
                // 生成新的session ID for next save
                self.state.collection.current_session_id = generate_session_id();
            } else {
                self.state.collection.save_status = "No data saved".to_string();
            }
        }
    }
    
    fn handle_export_results(&mut self) {
        if let Some(receiver) = &self.state.export.export_result_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.state.export.export_status = result.message;
                self.state.export.export_result_receiver = None; // 清除接收器
                info!("Export completed: {} succeeded, {} failed", result.success_count, result.error_count);
            }
        }
    }
    
    fn handle_sessions_results(&mut self) {
        if let Some(receiver) = &self.state.export.sessions_result_receiver {
            if let Ok(sessions) = receiver.try_recv() {
                self.state.export.available_sessions = sessions;
                self.state.export.export_status = format!("Found {} sessions", self.state.export.available_sessions.len());
                self.state.export.sessions_result_receiver = None; // 清除接收器
                info!("Refreshed sessions: found {}", self.state.export.available_sessions.len());
            }
        }
    }
    
    fn handle_data_processing(&mut self) {
        if self.state.calibration.is_calibrating {
            crate::app::handlers::CalibrationHandler::handle_calibration(self);
        } else if self.state.collection.is_collecting {
            crate::app::handlers::DataCollectionHandler::handle_collection(self);
        } else {
            // 停止状态：清空接收缓冲区
            while let Ok(_) = self.state.channels.data_receiver.try_recv() {
                // 丢弃数据
            }
            while let Ok(_) = self.state.channels.audio_receiver.try_recv() {
                // 丢弃音频数据
            }
        }
    }
    
    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                if self.state.collection.is_collecting {
                    self.save_current_window_data_async();
                } else {
                    self.state.collection.save_status = "Not collecting data".to_string();
                }
            }
        });
    }
    
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
            .map(|(x, y, z, timestamp)| DataPoint {
                x,
                y,
                z,
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
        };

        // 发送保存任务到后台线程
        match self.state.database.db_task_sender.try_send(save_task) {
            Ok(()) => {
                self.state.collection.save_status = "Saving data...".to_string();
                info!("Save task sent to background thread");
            }
            Err(e) => {
                self.state.collection.save_status = format!("Failed to queue save task: {}", e);
                error!("Failed to send save task: {}", e);
            }
        }
    }
}
