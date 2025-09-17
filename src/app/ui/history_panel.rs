use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use egui::Color32;
use crate::app::sensor_app::SensorDataApp;
use crate::types::DataPoint;
use log::warn;

/// 格式化数字为固定宽度的 y 轴标签
fn format_fixed_width_y_label(value: f64) -> String {
    let abs_value = value.abs();
    // 根据数值大小和正负选择格式，全部固定为6字符宽度，并显式显示符号
    if abs_value >= 1000.0 {
        // 极大或极小值：使用科学计数法，保留1位小数，总宽6位，强制显示符号
        format!("{:-6.1e}", value)
    } else if abs_value >= 100.0 {
        // 100-999：格式化为整数，总宽6位，强制显示符号（右对齐）
        format!("{:-6.0}", value)
    } else if abs_value >= 10.0 {
        // 10-99.9：保留1位小数，总宽6位，强制显示符号
        format!("{:-6.1}", value)
    } else if abs_value >= 1.0 {
        // 1-9.99：保留2位小数，总宽6位，强制显示符号
        format!("{:-6.2}", value)
    } else {
        // 0.001-0.999：保留3位小数，总宽6位，强制显示符号
        format!("{:-6.2}", value)
    }
}

pub fn render_history_panel(app: &mut SensorDataApp, ctx: &egui::Context) {
    if !app.state.history.show_history_panel {
        return;
    }

    egui::SidePanel::left("history_panel")
        .resizable(true)
        .default_width(app.state.history.panel_width)
        .width_range(250.0..=600.0)
        .show(ctx, |ui| {
            ui.heading("📊 History Data Visualization");
            ui.add_space(10.0);

            render_panel_controls(app, ui);
            ui.separator();
            ui.add_space(5.0);

            if app.state.history.selected_session.is_some() {
                render_history_visualization(app, ui);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(Color32::GRAY, "Please select a session to view history data");
                });
            }
        });

    // 渲染删除确认对话框
    render_delete_confirmation_dialog(app, ctx);
}

fn render_panel_controls(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Session Selection:");
        if ui.button("🔄").clicked() {
            refresh_history_sessions(app);
        }

        if ui.button("❌").clicked() {
            app.state.history.show_history_panel = false;
        }
    });

    ui.add_space(5.0);

    // Session selector dropdown
    render_session_selector(app, ui);

    ui.add_space(10.0);

    // Display options
    render_display_options(app, ui);

    ui.add_space(10.0);

    // Loading status
    if !app.state.history.loading_status.is_empty() {
        ui.colored_label(Color32::BLUE, &app.state.history.loading_status);
    }
}

fn render_session_selector(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    // First level: Username and Scenario selection
    ui.horizontal(|ui| {
        // Username selection
        ui.label("User:");
        
        let selected_username_text = app.state.history.selected_username
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("Select user...");

        egui::ComboBox::from_id_salt("username_selector")
            .selected_text(selected_username_text)
            .show_ui(ui, |ui| {
                for username in &app.state.history.available_usernames.clone() {
                    let response = ui.selectable_value(
                        &mut app.state.history.selected_username,
                        Some(username.clone()),
                        username
                    );

                    if response.clicked() {
                        // Reset session selection when username changes
                        app.state.history.selected_session = None;
                        app.state.history.history_sessions.clear();
                        // Load sessions for the selected username
                        load_sessions_for_username(app, username);
                    }
                }
            });

        ui.add_space(10.0);

        // Scenario selection
        ui.label("Scenario:");
        
        let selected_scenario_text = app.state.history.selected_scenario
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("Select scenario...");

        egui::ComboBox::from_id_salt("scenario_selector")
            .selected_text(selected_scenario_text)
            .show_ui(ui, |ui| {
                for scenario in &app.state.history.available_scenarios.clone() {
                    let response = ui.selectable_value(
                        &mut app.state.history.selected_scenario,
                        Some(scenario.clone()),
                        scenario
                    );

                    if response.clicked() {
                        // Reset session selection when scenario changes
                        app.state.history.selected_session = None;
                        app.state.history.history_sessions.clear();
                        // Load sessions for the selected username and scenario
                        if let Some(username) = app.state.history.selected_username.clone() {
                            load_sessions_for_username(app, &username);
                        }
                    }
                }
            });
    });

    ui.add_space(5.0);
    
    // Second level: Session selection (only if username is selected)
    if app.state.history.selected_username.is_some() {
        let username = app.state.history.selected_username.clone().unwrap();
        
        ui.horizontal(|ui| {
            ui.label("Session:");
            
            if app.state.history.history_sessions.is_empty() {
                ui.label(format!("Loading sessions for {}...", username));
            } else {
                // 显示当前session信息和导航按钮
                ui.horizontal(|ui| {
                    // 上一个按钮
                    if ui.button("◀").on_hover_text("Previous session").clicked() {
                        if let Some(session) = app.state.previous_session() {
                            load_both_data_types(app, &session);
                        }
                    }
                    
                    // 显示当前session信息
                    let session_info = app.state.get_current_session_info();
                    let current_session = app.state.history.selected_session
                        .as_ref()
                        .map(|s| s.as_str())
                        .unwrap_or("None");
                    ui.label(format!("{} ({})", current_session, session_info));
                    
                    // 下一个按钮
                    if ui.button("▶").on_hover_text("Next session").clicked() {
                        if let Some(session) = app.state.next_session() {
                            load_both_data_types(app, &session);
                        }
                    }
                    
                    // 添加删除按钮
                    if let Some(selected_session) = &app.state.history.selected_session {
                        if ui.button("🗑").on_hover_text("删除此session").clicked() {
                            app.state.history.session_to_delete = Some(selected_session.clone());
                            app.state.history.show_delete_confirmation = true;
                        }
                    }
                });
            }
        });
    } else {
        ui.label("Please select a user first");
    }
}

fn render_display_options(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    ui.label("Display Options:");
    ui.horizontal(|ui| {
        ui.label("加速度计:");
        ui.checkbox(&mut app.state.history.display_options.show_x_axis, "X-Axis");
        ui.checkbox(&mut app.state.history.display_options.show_y_axis, "Y-Axis");
        ui.checkbox(&mut app.state.history.display_options.show_z_axis, "Z-Axis");
    });
    
    ui.horizontal(|ui| {
        ui.label("陀螺仪:");
        ui.checkbox(&mut app.state.history.display_options.show_gx_axis, "GX-Axis");
        ui.checkbox(&mut app.state.history.display_options.show_gy_axis, "GY-Axis");
        ui.checkbox(&mut app.state.history.display_options.show_gz_axis, "GZ-Axis");
    });
    
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.state.history.display_options.show_audio, "Audio");
    });

    ui.add_space(5.0);

    // Data alignment toggle
    ui.label("Data Alignment:");
    ui.horizontal(|ui| {
        if ui.selectable_label(app.state.history.show_aligned_data, "🔄 Aligned").clicked() {
            if !app.state.history.show_aligned_data {
                app.state.history.show_aligned_data = true;
                switch_to_aligned_data(app);
            }
        }
        if ui.selectable_label(!app.state.history.show_aligned_data, "📊 Original").clicked() {
            if app.state.history.show_aligned_data {
                app.state.history.show_aligned_data = false;
                switch_to_original_data(app);
            }
        }
    });
}

fn render_history_visualization(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    if app.state.history.loaded_history_data.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.colored_label(Color32::GRAY, "Loading history data...");
        });
        return;
    }

    ui.horizontal(|ui| {
        ui.label(format!("Data Points: {}", app.state.history.loaded_history_data.len()));

        // 在Data Points后面显示Common Time Range
        if app.state.history.show_aligned_data && app.state.history.common_time_range_ms > 0 {
            ui.separator();
            ui.label(format!("Common Time Range: {}ms", app.state.history.common_time_range_ms));
        }

        ui.separator();
        if app.state.history.show_aligned_data {
            ui.colored_label(Color32::from_rgb(0, 150, 0), "🔄 Aligned");
        } else {
            ui.colored_label(Color32::from_rgb(0, 100, 200), "📊 Original");
        }
    });

    // Show comparison info if both data types are available
    if !app.state.history.original_history_data.is_empty() && !app.state.history.aligned_history_data.is_empty() {
        ui.horizontal(|ui| {
            ui.label(format!("Original: {} acc, {} audio",
                             app.state.history.original_history_data.len(),
                             app.state.history.original_audio_data.len()
            ));
            ui.separator();
            ui.label(format!("Aligned: {} acc, {} audio",
                             app.state.history.aligned_history_data.len(),
                             app.state.history.aligned_audio_data.len()
            ));
        });
    }

    // 音频播放控制区域（在滚动区域外面）
    if app.state.history.display_options.show_audio && !app.state.history.loaded_audio_data.is_empty() {
        ui.separator();
        render_audio_playback_controls(app, ui);
    }

    ui.add_space(5.0);

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 100.0)
        .show(ui, |ui| {
            // Render accelerometer data
            if app.state.history.display_options.show_x_axis {
                render_history_axis(ui, "ACC X-Axis History", &app.state.history.loaded_history_data, |dp| dp.x, Color32::RED);
            }

            if app.state.history.display_options.show_y_axis {
                render_history_axis(ui, "ACC Y-Axis History", &app.state.history.loaded_history_data, |dp| dp.y, Color32::GREEN);
            }

            if app.state.history.display_options.show_z_axis {
                render_history_axis(ui, "ACC Z-Axis History", &app.state.history.loaded_history_data, |dp| dp.z, Color32::BLUE);
            }

            // Render gyroscope data
            if app.state.history.display_options.show_gx_axis {
                render_history_axis(ui, "GYRO X-Axis History", &app.state.history.loaded_history_data, |dp| dp.gx, Color32::from_rgb(255, 165, 0));
            }

            if app.state.history.display_options.show_gy_axis {
                render_history_axis(ui, "GYRO Y-Axis History", &app.state.history.loaded_history_data, |dp| dp.gy, Color32::from_rgb(255, 20, 147));
            }

            if app.state.history.display_options.show_gz_axis {
                render_history_axis(ui, "GYRO Z-Axis History", &app.state.history.loaded_history_data, |dp| dp.gz, Color32::from_rgb(0, 255, 255));
            }

            // Render audio data (without controls)
            if app.state.history.display_options.show_audio && !app.state.history.loaded_audio_data.is_empty() {
                render_history_audio_waveform(ui, "Audio History", &app.state.history.loaded_audio_data, Color32::PURPLE, &app.state.history.audio_playback);
            }
        });
}

fn render_history_axis<F>(ui: &mut egui::Ui, title: &str, data: &[DataPoint], value_extractor: F, color: Color32)
where
    F: Fn(&DataPoint) -> f64,
{
    if data.is_empty() {
        return;
    }

    // 计算时间范围和数据范围
    let start_time = data.first().unwrap().timestamp as f64 / 1000.0; // 转换为秒
    let values: Vec<f64> = data.iter().map(&value_extractor).collect();

    let (y_min, y_max) = values.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(min, max), &val| (min.min(val), max.max(val))
    );

    let range = (y_max - y_min).max(0.1);
    let y_min_padded = y_min - range * 0.05;
    let y_max_padded = y_max + range * 0.05;

    Plot::new(title)
        .height(75.0)
        .x_axis_formatter(|v, _| format!("{:.2}s", v.value))
        .y_axis_formatter(|v, _| format_fixed_width_y_label(v.value))
        .allow_drag(true)
        .allow_zoom(true)
        .show(ui, |plot_ui| {
            let points: Vec<[f64; 2]> = data
                .iter()
                .map(|dp| {
                    let time_offset = (dp.timestamp as f64 / 1000.0) - start_time;
                    [time_offset, value_extractor(dp)]
                })
                .collect();

            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [0.0, y_min_padded],
                [(data.last().unwrap().timestamp as f64 / 1000.0) - start_time, y_max_padded],
            ));

            plot_ui.line(Line::new(title, PlotPoints::from(points)).color(color).width(0.75));
        });
}

// 音频播放控制区域
fn render_audio_playback_controls(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("🎵 Audio Playback:");
        
        // 播放按钮
        if app.state.history.audio_playback.is_playing {
            if ui.button("⏸ 暂停").clicked() {
                app.pause_history_audio();
            }
        } else {
            if ui.button("▶ 播放").clicked() {
                app.play_history_audio();
            }
        }

        // 停止按钮
        if ui.button("⏹ 停止").clicked() {
            app.stop_history_audio();
        }

        // 显示播放状态
        if app.state.history.audio_playback.is_available {
            ui.separator();
            if app.state.history.audio_playback.is_playing {
                ui.label("🔊 播放中");
            } else if app.state.history.audio_playback.is_paused {
                ui.label("⏸ 已暂停");
            } else {
                ui.label("⏹ 已停止");
            }
        }
    });
}

// 音频波形显示（不带控制按钮）
fn render_history_audio_waveform(ui: &mut egui::Ui, title: &str, audio_data: &[f64], color: Color32, _playback_state: &crate::app::state::AudioPlaybackState) {
    if audio_data.is_empty() {
        return;
    }

    let (y_min, y_max) = audio_data.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(min, max), &val| (min.min(val), max.max(val))
    );

    let range = (y_max - y_min).max(0.1);
    let y_min_padded = y_min - range * 0.05;
    let y_max_padded = y_max + range * 0.05;

    Plot::new(title)
        .height(100.0)
        .x_axis_formatter(|v, _| format!("{:.2}s", v.value))
        .y_axis_formatter(|v, _| format_fixed_width_y_label(v.value))
        .allow_drag(true)
        .allow_zoom(true)
        .show(ui, |plot_ui| {
            // 假设16kHz采样率
            let sample_rate = 16000.0;
            let points: Vec<[f64; 2]> = audio_data
                .iter()
                .enumerate()
                .map(|(i, &value)| {
                    let time = i as f64 / sample_rate;
                    [time, value]
                })
                .collect();

            let duration = audio_data.len() as f64 / sample_rate;
            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [0.0, y_min_padded],
                [duration, y_max_padded],
            ));

            plot_ui.line(Line::new(title, PlotPoints::from(points)).color(color).width(1.0));

        });
}

// Helper function: refresh history sessions list
pub fn refresh_history_sessions(app: &mut SensorDataApp) {
    use crate::types::DatabaseTask;

    // 检查是否已经有正在进行的用户名请求
    if app.state.history.usernames_result_receiver.is_some() {
        app.state.history.loading_status = "Already refreshing users list...".to_string();
        return;
    }

    app.state.history.loading_status = "Refreshing users and scenarios list...".to_string();

    // 发送用户名查询请求
    let (usernames_sender, usernames_receiver) = crossbeam_channel::unbounded();
    let usernames_task = DatabaseTask::GetUsernames { response_sender: usernames_sender };

    if let Ok(()) = app.state.database.db_task_sender.try_send(usernames_task) {
        app.state.history.usernames_result_receiver = Some(usernames_receiver);
    } else {
        app.state.history.loading_status = "Unable to send usernames query request".to_string();
        return;
    }

    // 发送scenarios查询请求
    let (scenarios_sender, scenarios_receiver) = crossbeam_channel::unbounded();
    let scenarios_task = DatabaseTask::GetScenarios { response_sender: scenarios_sender };

    if let Ok(()) = app.state.database.db_task_sender.try_send(scenarios_task) {
        app.state.history.scenarios_result_receiver = Some(scenarios_receiver);
    } else {
        warn!("Unable to send scenarios query request");
    }
}

fn load_sessions_for_username(app: &mut SensorDataApp, username: &str) {
    use crate::types::DatabaseTask;

    // 检查是否已经有正在进行的会话请求
    if app.state.history.sessions_result_receiver.is_some() {
        app.state.history.loading_status = format!("Already loading sessions for user: {}", username);
        return;
    }

    // 获取选中的scenario，如果没有选择则使用"standard"
    let scenario = app.state.history.selected_scenario
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("standard");

    app.state.history.loading_status = format!("Loading sessions for user: {} in scenario: {}", username, scenario);

    let (sender, receiver) = crossbeam_channel::unbounded();
    let task = DatabaseTask::GetSessionsByUsernameAndScenario { 
        username: username.to_string(),
        scenario: scenario.to_string(),
        response_sender: sender 
    };

    if let Ok(()) = app.state.database.db_task_sender.try_send(task) {
        app.state.history.sessions_result_receiver = Some(receiver);
    } else {
        app.state.history.loading_status = "Unable to send sessions query request".to_string();
    }
}

// Helper function: load session data (using aligned data)
fn load_session_data(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

    // 检查是否已经有正在进行的对齐数据请求
    if app.state.history.aligned_history_result_receiver.is_some() {
        app.state.history.loading_status = format!("Already loading aligned session data: {}", session_id);
        return;
    }

    app.state.history.loading_status = format!("Loading aligned session data: {}", session_id);

    let (sender, receiver) = crossbeam_channel::unbounded();
    let task = DatabaseTask::LoadAlignedHistoryData {
        session_id: session_id.to_string(),
        response_sender: sender,
    };

    if let Ok(()) = app.state.database.db_task_sender.try_send(task) {
        app.state.history.aligned_history_result_receiver = Some(receiver);
    } else {
        app.state.history.loading_status = "Unable to send aligned data loading request".to_string();
    }
}

// Helper function: load both original and aligned data
pub fn load_both_data_types_from_main(app: &mut SensorDataApp, session_id: &str) {
    load_both_data_types(app, session_id);
}

pub fn load_sessions_for_username_from_main(app: &mut SensorDataApp, username: &str) {
    load_sessions_for_username(app, username);
}

fn load_both_data_types(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

    app.state.history.loading_status = format!("Loading both original and aligned data: {}", session_id);

    // Load original data
    let (original_sender, original_receiver) = crossbeam_channel::unbounded();
    let original_task = DatabaseTask::LoadHistoryData {
        session_id: session_id.to_string(),
        response_sender: original_sender,
    };

    // Load aligned data
    let (aligned_sender, aligned_receiver) = crossbeam_channel::unbounded();
    let aligned_task = DatabaseTask::LoadAlignedHistoryData {
        session_id: session_id.to_string(),
        response_sender: aligned_sender,
    };

    // Send both tasks
    let original_sent = app.state.database.db_task_sender.try_send(original_task).is_ok();
    let aligned_sent = app.state.database.db_task_sender.try_send(aligned_task).is_ok();

    if original_sent && aligned_sent {
        app.state.history.history_result_receiver = Some(original_receiver);
        app.state.history.aligned_history_result_receiver = Some(aligned_receiver);
    } else {
        app.state.history.loading_status = "Unable to send data loading requests".to_string();
    }
}

// Helper function: switch to aligned data view
fn switch_to_aligned_data(app: &mut SensorDataApp) {
    // If we have aligned data stored, switch to it
    if !app.state.history.aligned_history_data.is_empty() || !app.state.history.aligned_audio_data.is_empty() {
        app.state.history.loaded_history_data = app.state.history.aligned_history_data.clone();
        app.state.history.loaded_audio_data = app.state.history.aligned_audio_data.clone();
        app.state.history.loading_status = format!(
            "Showing aligned data: {} acc points, {} audio samples",
            app.state.history.loaded_history_data.len(),
            app.state.history.loaded_audio_data.len()
        );
        return;
    }

    // If we don't have aligned data, reload it
    if let Some(session_id) = app.state.history.selected_session.clone() {
        load_session_data(app, &session_id);
    }
}

// Helper function: switch to original data view
fn switch_to_original_data(app: &mut SensorDataApp) {
    // Switch current display to original data if available
    if !app.state.history.original_history_data.is_empty() || !app.state.history.original_audio_data.is_empty() {
        app.state.history.loaded_history_data = app.state.history.original_history_data.clone();
        app.state.history.loaded_audio_data = app.state.history.original_audio_data.clone();
        app.state.history.loading_status = format!(
            "Showing original data: {} acc points, {} audio samples",
            app.state.history.loaded_history_data.len(),
            app.state.history.loaded_audio_data.len()
        );
    } else if let Some(session_id) = app.state.history.selected_session.clone() {
        // Load original data if not available
        load_original_data(app, &session_id);
    }
}

// Helper function: load original data only
fn load_original_data(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

    // 检查是否已经有正在进行的原始数据请求
    if app.state.history.history_result_receiver.is_some() {
        app.state.history.loading_status = format!("Already loading original session data: {}", session_id);
        return;
    }

    app.state.history.loading_status = format!("Loading original session data: {}", session_id);

    let (sender, receiver) = crossbeam_channel::unbounded();
    let task = DatabaseTask::LoadHistoryData {
        session_id: session_id.to_string(),
        response_sender: sender,
    };

    if let Ok(()) = app.state.database.db_task_sender.try_send(task) {
        app.state.history.history_result_receiver = Some(receiver);
    } else {
        app.state.history.loading_status = "Unable to send original data loading request".to_string();
    }
}

// 渲染删除确认对话框
fn render_delete_confirmation_dialog(app: &mut SensorDataApp, ctx: &egui::Context) {
    if !app.state.history.show_delete_confirmation {
        return;
    }

    egui::Window::new("确认删除")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            if let Some(session_id) = app.state.history.session_to_delete.clone() {
                ui.label(format!("确定要删除session '{}'吗？", session_id));
                ui.add_space(10.0);
                ui.colored_label(egui::Color32::from_rgb(200, 100, 100), "⚠ 此操作不可撤销！");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("❌ 取消").clicked() {
                        app.state.history.show_delete_confirmation = false;
                        app.state.history.session_to_delete = None;
                    }

                    ui.add_space(20.0);

                    if ui.button("🗑 确认删除").clicked() {
                        delete_selected_session(app, &session_id);
                        app.state.history.show_delete_confirmation = false;
                    }
                });
            }
        });
}

// 删除选中的session
fn delete_selected_session(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

    // 检查是否已经有正在进行的删除请求
    if app.state.history.delete_result_receiver.is_some() {
        app.state.history.loading_status = format!("已经在删除session: {}", session_id);
        return;
    }

    app.state.history.loading_status = format!("正在删除session: {}", session_id);

    let (sender, receiver) = crossbeam_channel::unbounded();
    let task = DatabaseTask::DeleteSession {
        session_id: session_id.to_string(),
        response_sender: sender,
    };

    if let Ok(()) = app.state.database.db_task_sender.try_send(task) {
        app.state.history.delete_result_receiver = Some(receiver);
    } else {
        app.state.history.loading_status = "无法发送删除请求".to_string();
    }
}
