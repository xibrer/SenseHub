use eframe::egui;
use log::{info, warn};

use super::app_core::SensorDataApp;

impl SensorDataApp {
    pub fn handle_save_results(&mut self) {
        while let Ok(result) = self.state.database.save_result_receiver.try_recv() {
            if let Some(error) = result.error {
                self.state.collection.save_status = error;
            } else if result.acc_saved > 0 || result.audio_saved > 0 {
                self.state.collection.save_status = format!("Saved: {} ACC points, {} audio records", result.acc_saved, result.audio_saved);
                info!("Data saved successfully: {} ACC, {} audio", result.acc_saved, result.audio_saved);

                // 生成新的session ID for next save
                self.state.collection.current_session_id = crate::database::generate_session_id();
            } else {
                self.state.collection.save_status = "No data saved".to_string();
            }
        }
    }

    pub fn handle_export_results(&mut self) {
        if let Some(receiver) = &self.state.export.export_result_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.state.export.export_status = result.message;
                self.state.export.export_result_receiver = None; // 清除接收器
                info!("Export completed: {} succeeded, {} failed", result.success_count, result.error_count);
            }
        }
    }

    pub fn handle_sessions_results(&mut self) {
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

    pub fn handle_history_results(&mut self) {
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
                        crate::app::ui::history_controls::load_sessions_for_username_from_main(self, &first_username);
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
                        crate::app::ui::history_controls::load_sessions_for_username_from_main(self, &username);
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
                    crate::app::ui::history_controls::load_both_data_types_from_main(self, &first_session);
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
                            // 如果删除的是当前选中的session，需要重新选择session
                            if self.state.history.selected_session.as_ref() == Some(deleted_session) {
                                // 先清除当前选中的session
                                self.state.history.selected_session = None;
                                self.state.history.loaded_history_data.clear();
                                self.state.history.loaded_audio_data.clear();
                                self.state.history.original_history_data.clear();
                                self.state.history.original_audio_data.clear();
                                self.state.history.aligned_history_data.clear();
                                self.state.history.aligned_audio_data.clear();
                            }

                            // 记录删除前的索引位置
                            let deleted_index = self.state.history.history_sessions.iter().position(|s| s == deleted_session);
                            
                            // 从session列表中移除已删除的session
                            self.state.history.history_sessions.retain(|s| s != deleted_session);
                            
                            // 如果还有剩余的sessions，选择合适的session
                            if !self.state.history.history_sessions.is_empty() {
                                let target_index = if let Some(deleted_idx) = deleted_index {
                                    // 如果删除的不是第一个，选择上一个；否则选择第一个
                                    if deleted_idx > 0 { deleted_idx - 1 } else { 0 }
                                } else {
                                    // 如果找不到删除的session，选择第一个
                                    0
                                };
                                
                                // 确保索引不超出范围
                                let final_index = target_index.min(self.state.history.history_sessions.len() - 1);
                                let target_session = self.state.history.history_sessions[final_index].clone();
                                
                                self.state.history.selected_session = Some(target_session.clone());
                                self.state.history.current_session_index = final_index;
                                
                                // 自动加载选中的session的数据
                                crate::app::ui::history_controls::load_both_data_types_from_main(self, &target_session);
                            } else {
                                // 如果没有剩余的sessions，重置索引
                                self.state.history.current_session_index = 0;
                            }
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

    pub fn handle_data_processing(&mut self) {
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

    pub fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
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
}