use std::time::{Duration, Instant};
use eframe::{egui, Frame};
use log::{info, error, warn};

use crate::types::{DataPoint, AudioData, DatabaseTask, SaveResult};
use crate::database::generate_session_id;
use crate::config::ConfigManager;
use crate::audio::AudioPlayer;
use super::state::AppState;

pub struct SensorDataApp {
    // 统一的状态管理
    pub state: AppState,

    // 配置管理
    pub config: ConfigManager,

    // 音频播放器
    pub audio_player: Option<AudioPlayer>,
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
            config.get_config(),
        );

        // 初始化会话ID
        state.collection.current_session_id = generate_session_id();
        
        // 初始化自动保存间隔为窗口长度
        let plot_config = config.get_config();
        state.collection.auto_save_interval_ms = (plot_config.plot.window_duration_seconds * 1000.0) as u64;

        // 初始化音频播放器
        let audio_player = match AudioPlayer::new() {
            Ok(player) => {
                info!("Audio player initialized successfully");
                Some(player)
            }
            Err(e) => {
                warn!("Failed to initialize audio player: {}", e);
                None
            }
        };

        let mut app = SensorDataApp {
            state,
            config,
            audio_player,
        };

        // 加载文本文件
        if let Err(e) = app.state.load_text_file("documents/chinese.txt") {
            warn!("Failed to load text file: {}", e);
        } else {
            info!("Text file loaded successfully");
        }

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
        crate::app::ui::render_bottom_status_bar(self, ctx);
        crate::app::ui::render_history_panel(self, ctx);
        crate::app::ui::render_main_panel(self, ctx);
        crate::app::ui::render_export_dialog(self, ctx);

        // 处理各种结果
        self.handle_save_results();
        self.handle_export_results();
        self.handle_sessions_results();
        self.handle_history_results();

        // 处理数据：校准、采集或丢弃
        self.handle_data_processing();

        // 处理键盘输入
        self.handle_keyboard_input(ctx);

        // 更新音频播放状态
        self.update_audio_playback_state();

        ctx.request_repaint_after(Duration::from_millis(150));
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
            if let Ok(sessions_with_status) = receiver.try_recv() {
                self.state.export.sessions_with_export_status = sessions_with_status.clone();
                
                // 提取所有session ID
                self.state.export.available_sessions = sessions_with_status.iter()
                    .map(|(session_id, _)| session_id.clone())
                    .collect();
                
                // 统计已导出和未导出的session数量
                let exported_count = sessions_with_status.iter().filter(|(_, is_exported)| *is_exported).count();
                let unexported_count = sessions_with_status.len() - exported_count;
                
                self.state.export.export_status = format!(
                    "Found {} sessions ({} exported, {} unexported)", 
                    sessions_with_status.len(), exported_count, unexported_count
                );
                self.state.export.sessions_result_receiver = None; // 清除接收器
                info!("Refreshed sessions: found {} total ({} exported, {} unexported)", 
                      sessions_with_status.len(), exported_count, unexported_count);
            }
        }
    }

    fn handle_history_results(&mut self) {
        // Handle username list results
        if let Some(receiver) = &self.state.history.usernames_result_receiver {
            if let Ok(usernames) = receiver.try_recv() {
                self.state.history.available_usernames = usernames;
                self.state.history.loading_status = format!("Found {} users", self.state.history.available_usernames.len());
                self.state.history.usernames_result_receiver = None; // Clear receiver
                
                // 自动选择第一个用户（如果列表不为空且当前没有选择）
                if !self.state.history.available_usernames.is_empty() && self.state.history.selected_username.is_none() {
                    let first_username = self.state.history.available_usernames[0].clone();
                    self.state.history.selected_username = Some(first_username.clone());
                    info!("Auto-selected first username: {}", first_username);
                    
                    // 如果scenario也已经选择，则加载sessions
                    if self.state.history.selected_scenario.is_some() {
                        crate::app::ui::history_panel::load_sessions_for_username_from_main(self, &first_username);
                    }
                }
                
                info!("Refreshed usernames: found {}", self.state.history.available_usernames.len());
            }
        }

        // Handle scenarios list results
        if let Some(receiver) = &self.state.history.scenarios_result_receiver {
            if let Ok(scenarios) = receiver.try_recv() {
                self.state.history.available_scenarios = scenarios;
                self.state.history.scenarios_result_receiver = None; // Clear receiver
                
                // 自动选择第一个scenario（如果列表不为空且当前没有选择）
                if !self.state.history.available_scenarios.is_empty() && self.state.history.selected_scenario.is_none() {
                    let first_scenario = self.state.history.available_scenarios[0].clone();
                    self.state.history.selected_scenario = Some(first_scenario.clone());
                    info!("Auto-selected first scenario: {}", first_scenario);
                    
                    // 如果用户名也已经选择，则加载sessions
                    if let Some(username) = self.state.history.selected_username.clone() {
                        crate::app::ui::history_panel::load_sessions_for_username_from_main(self, &username);
                    }
                }
                
                info!("Refreshed scenarios: found {}", self.state.history.available_scenarios.len());
            }
        }

        // Handle session list results
        if let Some(receiver) = &self.state.history.sessions_result_receiver {
            if let Ok(sessions) = receiver.try_recv() {
                self.state.history.history_sessions = sessions;
                self.state.history.loading_status = format!("Found {} history sessions for selected user", self.state.history.history_sessions.len());
                self.state.history.sessions_result_receiver = None; // Clear receiver
                
                // 自动选择第一个session（如果列表不为空且当前没有选择）
                if !self.state.history.history_sessions.is_empty() && self.state.history.selected_session.is_none() {
                    let first_session = self.state.history.history_sessions[0].clone();
                    self.state.history.selected_session = Some(first_session.clone());
                    self.state.history.current_session_index = 0;
                    info!("Auto-selected first session: {}", first_session);
                    
                    // 自动加载第一个session的数据
                    crate::app::ui::history_panel::load_both_data_types_from_main(self, &first_session);
                }
                
                info!("Refreshed history sessions for user: found {} sessions", self.state.history.history_sessions.len());
            }
        }

        // Handle history data loading results (original data)
        if let Some(receiver) = &self.state.history.history_result_receiver {
            if let Ok((acc_data, audio_data)) = receiver.try_recv() {
                // Store original data
                self.state.history.original_history_data = acc_data.clone();
                self.state.history.original_audio_data = audio_data.clone();

                // If currently showing original data, update display
                if !self.state.history.show_aligned_data {
                    self.state.history.loaded_history_data = acc_data;
                    self.state.history.loaded_audio_data = audio_data;
                    self.state.history.loading_status = format!(
                        "Loaded original data: {} acc points, {} audio samples",
                        self.state.history.loaded_history_data.len(),
                        self.state.history.loaded_audio_data.len()
                    );
                }

                self.state.history.history_result_receiver = None; // Clear receiver
                info!("Loaded original history data: {} acc points, {} audio samples", 
                     self.state.history.original_history_data.len(), 
                     self.state.history.original_audio_data.len());
            }
        }

        // Handle aligned history data loading results
        if let Some(receiver) = &self.state.history.aligned_history_result_receiver {
            if let Ok((acc_data, audio_data, common_time_range_ms)) = receiver.try_recv() {
                // Store aligned data
                self.state.history.aligned_history_data = acc_data.clone();
                self.state.history.aligned_audio_data = audio_data.clone();
                self.state.history.common_time_range_ms = common_time_range_ms;

                // If currently showing aligned data, update display
                if self.state.history.show_aligned_data {
                    self.state.history.loaded_history_data = acc_data.clone();
                    self.state.history.loaded_audio_data = audio_data.clone();
                    self.state.history.loading_status = format!(
                        "Loaded aligned data: {} acc points, {} audio samples",
                        self.state.history.loaded_history_data.len(),
                        self.state.history.loaded_audio_data.len()
                    );
                }

                self.state.history.aligned_history_result_receiver = None; // Clear receiver
                info!("Loaded aligned history data: {} acc points, {} audio samples, {}ms common range", 
                     acc_data.len(), 
                     audio_data.len(),
                     common_time_range_ms);
            }
        }

        // Handle delete session results
        if let Some(receiver) = &self.state.history.delete_result_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(()) => {
                        self.state.history.loading_status = "Session删除成功".to_string();
                        
                        // 清除相关状态
                        if let Some(deleted_session) = &self.state.history.session_to_delete {
                            // 如果删除的是当前选中的session，清除选中状态
                            if self.state.history.selected_session.as_ref() == Some(deleted_session) {
                                self.state.history.selected_session = None;
                                self.state.history.loaded_history_data.clear();
                                self.state.history.loaded_audio_data.clear();
                                self.state.history.original_history_data.clear();
                                self.state.history.original_audio_data.clear();
                                self.state.history.aligned_history_data.clear();
                                self.state.history.aligned_audio_data.clear();
                            }
                            
                            // 从session列表中移除已删除的session
                            self.state.history.history_sessions.retain(|s| s != deleted_session);
                        }
                        
                        self.state.history.session_to_delete = None;
                        info!("Session deleted successfully");
                    }
                    Err(error_msg) => {
                        self.state.history.loading_status = format!("删除失败: {}", error_msg);
                        self.state.history.session_to_delete = None;
                    }
                }
                
                self.state.history.delete_result_receiver = None;
            }
        }
    }

    fn handle_data_processing(&mut self) {
        if self.state.calibration.is_calibrating {
            crate::app::handlers::CalibrationHandler::handle_calibration(self);
        } else if self.state.collection.is_collecting {
            if self.state.collection.is_paused {
                // 暂停状态：清空接收缓冲区但不处理数据
                while let Ok(_) = self.state.channels.data_receiver.try_recv() {
                    // 丢弃数据
                }
                while let Ok(_) = self.state.channels.audio_receiver.try_recv() {
                    // 丢弃音频数据
                }
            } else {
                // 正常采集状态：处理数据
                crate::app::handlers::DataCollectionHandler::handle_collection(self);
                
                // 检查是否需要自动保存
                self.check_auto_save();
            }
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
                // 空格键同时处理文本切换和数据保存
                let mut performed_action = false;
                
                // 1. 如果文本阅读器启用，切换到下一行文本
                if self.state.text_reader.is_enabled {
                    self.state.next_text_line();
                    performed_action = true;
                }
                
                // 2. 如果正在采集数据且未暂停，保存当前窗口数据
                if self.state.collection.is_collecting && !self.state.collection.is_paused {
                    self.save_current_window_data_async();
                    performed_action = true;
                } else if self.state.collection.is_paused {
                    self.state.collection.save_status = "Data collection is paused".to_string();
                } else if !performed_action {
                    self.state.collection.save_status = "Not collecting data".to_string();
                }
            }
            
            // 左箭头键 - 上一行文本
            if i.key_pressed(egui::Key::ArrowLeft) && self.state.text_reader.is_enabled {
                self.state.previous_text_line();
            }
            
            // 右箭头键 - 下一行文本  
            if i.key_pressed(egui::Key::ArrowRight) && self.state.text_reader.is_enabled {
                self.state.next_text_line();
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

    /// 播放历史音频数据
    pub fn play_history_audio(&mut self) {
        if let Some(ref mut player) = self.audio_player {
            if !self.state.history.loaded_audio_data.is_empty() {
                // 加载音频数据到播放器
                player.load_audio_data(&self.state.history.loaded_audio_data, 16000.0);
                
                // 开始播放
                match player.play() {
                    Ok(()) => {
                        self.state.history.audio_playback.is_playing = true;
                        self.state.history.audio_playback.is_paused = false;
                        self.state.history.audio_playback.is_available = true;
                        info!("Started playing history audio");
                    }
                    Err(e) => {
                        warn!("Failed to start playing audio: {}", e);
                        self.state.history.audio_playback.is_available = false;
                    }
                }
            }
        }
    }

    /// 暂停历史音频播放
    pub fn pause_history_audio(&mut self) {
        if let Some(ref mut player) = self.audio_player {
            player.pause();
            self.state.history.audio_playback.is_playing = false;
            self.state.history.audio_playback.is_paused = true;
            info!("Paused history audio playback");
        }
    }

    /// 停止历史音频播放
    pub fn stop_history_audio(&mut self) {
        if let Some(ref mut player) = self.audio_player {
            player.stop();
            self.state.history.audio_playback.is_playing = false;
            self.state.history.audio_playback.is_paused = false;
            info!("Stopped history audio playback");
        }
    }


    /// 更新音频播放状态
    pub fn update_audio_playback_state(&mut self) {
        if let Some(ref player) = self.audio_player {
            // 更新来自工作线程的状态
            player.update_status();
            
            use crate::audio::player::PlaybackState;
            
            let state = player.get_state();
            self.state.history.audio_playback.is_available = player.is_available();

            match state {
                PlaybackState::Playing => {
                    self.state.history.audio_playback.is_playing = true;
                    self.state.history.audio_playback.is_paused = false;
                }
                PlaybackState::Paused => {
                    self.state.history.audio_playback.is_playing = false;
                    self.state.history.audio_playback.is_paused = true;
                }
                PlaybackState::Stopped => {
                    self.state.history.audio_playback.is_playing = false;
                    self.state.history.audio_playback.is_paused = false;
                }
            }
        }
    }

    /// 检查是否需要自动保存
    fn check_auto_save(&mut self) {
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
