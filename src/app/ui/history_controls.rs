use eframe::egui;
use egui::Color32;
use crate::app::app_core::SensorDataApp;
use log::warn;

pub fn render_panel_controls(app: &mut SensorDataApp, ui: &mut egui::Ui) {
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

    render_session_selector(app, ui);

    ui.add_space(10.0);

    render_display_options(app, ui);

    ui.add_space(10.0);

    if !app.state.history.loading_status.is_empty() {
        ui.colored_label(Color32::BLUE, &app.state.history.loading_status);
    }
}

pub fn render_session_selector(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
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
                        app.state.history.selected_session = None;
                        app.state.history.history_sessions.clear();
                        app.state.history.selected_scenario = None;
                        load_sessions_for_username(app, username);
                        load_scenarios_for_username(app, username);
                    }
                }
            });

        ui.add_space(10.0);

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
                        app.state.history.selected_session = None;
                        app.state.history.history_sessions.clear();
                        if let Some(username) = app.state.history.selected_username.clone() {
                            load_sessions_for_username(app, &username);
                        }
                    }
                }
            });
    });

    ui.add_space(5.0);

    if app.state.history.selected_username.is_some() {
        let username = app.state.history.selected_username.clone().unwrap();

        ui.horizontal(|ui| {
            ui.label("Session:");

            if app.state.history.history_sessions.is_empty() {
                ui.label(format!("Loading sessions for {}...", username));
            } else {
                ui.horizontal(|ui| {
                    if ui.button("◀").on_hover_text("Previous session").clicked() {
                        if let Some(session) = app.state.previous_session() {
                            load_both_data_types(app, &session);
                        }
                    }

                    let session_info = app.state.get_current_session_info();
                    let current_session = app.state.history.selected_session
                        .as_ref()
                        .map(|s| s.as_str())
                        .unwrap_or("None");
                    ui.label(format!("{} ({})", current_session, session_info));

                    if ui.button("▶").on_hover_text("Next session").clicked() {
                        if let Some(session) = app.state.next_session() {
                            load_both_data_types(app, &session);
                        }
                    }

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

pub fn render_display_options(app: &mut SensorDataApp, ui: &mut egui::Ui) {
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

pub fn render_delete_confirmation_dialog(app: &mut SensorDataApp, ctx: &egui::Context) {
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

pub fn render_audio_playback_controls(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("🎵 Audio Playback:");

        if app.state.history.audio_playback.is_playing {
            if ui.button("⏸ 暂停").clicked() {
                app.pause_history_audio();
            }
        } else {
            if ui.button("▶ 播放").clicked() {
                app.play_history_audio();
            }
        }

        if ui.button("⏹ 停止").clicked() {
            app.stop_history_audio();
        }

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

pub fn refresh_history_sessions(app: &mut SensorDataApp) {
    use crate::types::DatabaseTask;

    if app.state.history.usernames_result_receiver.is_some() {
        app.state.history.loading_status = "Already refreshing users list...".to_string();
        return;
    }

    app.state.history.loading_status = "Refreshing users and scenarios list...".to_string();

    let (usernames_sender, usernames_receiver) = crossbeam_channel::unbounded();
    let usernames_task = DatabaseTask::GetUsernames { response_sender: usernames_sender };

    if let Ok(()) = app.state.database.db_task_sender.try_send(usernames_task) {
        app.state.history.usernames_result_receiver = Some(usernames_receiver);
    } else {
        app.state.history.loading_status = "Unable to send usernames query request".to_string();
        return;
    }

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

    if app.state.history.sessions_result_receiver.is_some() {
        app.state.history.loading_status = format!("Already loading sessions for user: {}", username);
        return;
    }

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

fn load_scenarios_for_username(app: &mut SensorDataApp, username: &str) {
    use crate::types::DatabaseTask;

    if app.state.history.scenarios_result_receiver.is_some() {
        app.state.history.loading_status = format!("Already loading scenarios for user: {}", username);
        return;
    }

    app.state.history.loading_status = format!("Loading scenarios for user: {}", username);

    let (sender, receiver) = crossbeam_channel::unbounded();
    let task = DatabaseTask::GetScenariosByUsername {
        username: username.to_string(),
        response_sender: sender
    };

    if let Ok(()) = app.state.database.db_task_sender.try_send(task) {
        app.state.history.scenarios_result_receiver = Some(receiver);
    } else {
        app.state.history.loading_status = "Unable to send scenarios query request".to_string();
    }
}

fn load_both_data_types(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

    app.state.history.loading_status = format!("Loading both original and aligned data: {}", session_id);

    let (original_sender, original_receiver) = crossbeam_channel::unbounded();
    let original_task = DatabaseTask::LoadHistoryData {
        session_id: session_id.to_string(),
        response_sender: original_sender,
    };

    let (aligned_sender, aligned_receiver) = crossbeam_channel::unbounded();
    let aligned_task = DatabaseTask::LoadAlignedHistoryData {
        session_id: session_id.to_string(),
        response_sender: aligned_sender,
    };

    let original_sent = app.state.database.db_task_sender.try_send(original_task).is_ok();
    let aligned_sent = app.state.database.db_task_sender.try_send(aligned_task).is_ok();

    if original_sent && aligned_sent {
        app.state.history.history_result_receiver = Some(original_receiver);
        app.state.history.aligned_history_result_receiver = Some(aligned_receiver);
    } else {
        app.state.history.loading_status = "Unable to send data loading requests".to_string();
    }
}

fn switch_to_aligned_data(app: &mut SensorDataApp) {
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

    if let Some(session_id) = app.state.history.selected_session.clone() {
        load_session_data(app, &session_id);
    }
}

fn switch_to_original_data(app: &mut SensorDataApp) {
    if !app.state.history.original_history_data.is_empty() || !app.state.history.original_audio_data.is_empty() {
        app.state.history.loaded_history_data = app.state.history.original_history_data.clone();
        app.state.history.loaded_audio_data = app.state.history.original_audio_data.clone();
        app.state.history.loading_status = format!(
            "Showing original data: {} acc points, {} audio samples",
            app.state.history.loaded_history_data.len(),
            app.state.history.loaded_audio_data.len()
        );
    } else if let Some(session_id) = app.state.history.selected_session.clone() {
        load_original_data(app, &session_id);
    }
}

fn load_session_data(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

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

fn load_original_data(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

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

fn delete_selected_session(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

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

// Public wrapper functions for external use
pub fn load_both_data_types_from_main(app: &mut SensorDataApp, session_id: &str) {
    load_both_data_types(app, session_id);
}

pub fn load_sessions_for_username_from_main(app: &mut SensorDataApp, username: &str) {
    load_sessions_for_username(app, username);
}