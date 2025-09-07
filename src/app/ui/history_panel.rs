use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use egui::Color32;
use crate::app::sensor_app::SensorDataApp;
use crate::types::DataPoint;

/// 格式化数字为固定宽度的 y 轴标签
fn format_fixed_width_y_label(value: f64) -> String {
    // 使用固定6字符宽度的格式
    if value == 0.0 {
        return " 0.00 ".to_string();
    }

    let abs_value = value.abs();

    // 根据数值大小选择合适的格式，但保持固定宽度
    if abs_value >= 1000.0 {
        // 大于等于1000：使用科学计数法，固定宽度
        format!("{:6.1e}", value)
    } else if abs_value >= 100.0 {
        // 100-999：整数格式，右对齐6字符宽度
        format!("{:6.0}", value)
    } else if abs_value >= 10.0 {
        // 10-99.9：一位小数，右对齐6字符宽度
        format!("{:6.1}", value)
    } else if abs_value >= 1.0 {
        // 1-9.99：两位小数，右对齐6字符宽度
        format!("{:6.2}", value)
    } else if abs_value >= 0.01 {
        // 0.01-0.999：三位小数，右对齐6字符宽度
        format!("{:6.3}", value)
    } else {
        // 小于0.01：使用科学计数法，固定宽度
        format!("{:6.1e}", value)
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
    if app.state.history.history_sessions.is_empty() {
        ui.label("No available history sessions");
        return;
    }

    let selected_text = app.state.history.selected_session
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("Select session...");

    egui::ComboBox::from_label("Session")
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            for session in &app.state.history.history_sessions.clone() {
                let response = ui.selectable_value(
                    &mut app.state.history.selected_session,
                    Some(session.clone()),
                    session
                );

                if response.clicked() {
                    load_both_data_types(app, session);
                }
            }
        });
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

    ui.add_space(5.0);

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 50.0)
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

            // Render audio data
            if app.state.history.display_options.show_audio && !app.state.history.loaded_audio_data.is_empty() {
                render_history_audio(ui, "Audio History", &app.state.history.loaded_audio_data, Color32::PURPLE);
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

fn render_history_audio(ui: &mut egui::Ui, title: &str, audio_data: &[f64], color: Color32) {
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
        .height(75.0)
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
fn refresh_history_sessions(app: &mut SensorDataApp) {
    use crate::types::DatabaseTask;

    app.state.history.loading_status = "Refreshing sessions list...".to_string();

    let (sender, receiver) = crossbeam_channel::unbounded();
    let task = DatabaseTask::GetSessions { response_sender: sender };

    if let Ok(()) = app.state.database.db_task_sender.try_send(task) {
        app.state.history.sessions_result_receiver = Some(receiver);
    } else {
        app.state.history.loading_status = "Unable to send session query request".to_string();
    }
}

// Helper function: load session data (using aligned data)
fn load_session_data(app: &mut SensorDataApp, session_id: &str) {
    use crate::types::DatabaseTask;

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
