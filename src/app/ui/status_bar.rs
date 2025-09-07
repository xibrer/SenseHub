use eframe::egui;
use crate::app::sensor_app::SensorDataApp;

pub fn render_status_bar(app: &mut SensorDataApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("status_bar")
        .min_height(40.0)
        .show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label("Status:");

                let (status_text, status_color) = if app.state.calibration.is_calibrating {
                    ("Calibrating", egui::Color32::from_rgb(255, 165, 0)) // 橙色
                } else if app.state.collection.is_collecting {
                    if app.state.collection.is_paused {
                        ("Paused", egui::Color32::from_rgb(255, 165, 0)) // 橙色
                    } else {
                        ("Collecting", egui::Color32::from_rgb(0, 150, 0)) // 绿色
                    }
                } else {
                    ("Stopped", egui::Color32::from_rgb(150, 0, 0)) // 红色
                };

                ui.colored_label(status_color, status_text);

                // 添加暂停/恢复按钮
                if app.state.collection.is_collecting && !app.state.calibration.is_calibrating {
                    ui.separator();

                    let pause_button_text = if app.state.collection.is_paused {
                        "▶ Resume"
                    } else {
                        "⏸ Pause"
                    };

                    if ui.button(pause_button_text).clicked() {
                        if app.state.collection.is_paused {
                            app.state.resume_collection();
                        } else {
                            app.state.pause_collection();
                        }
                    }
                }

                ui.separator();

                // 状态显示
                render_status_details(app, ui);

                ui.separator();

                // 显示采样率信息
                if let Some(rate) = app.state.calibration.calculated_sample_rate {
                    ui.label(format!("Sample Rate: {:.1} Hz", rate));
                } else {
                    ui.label("Sample Rate: Not calibrated");
                }

                ui.separator();
                ui.label("Window: 5.0s");

                ui.separator();


                // 在最右边添加导出按钮和历史面板按钮
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📤 Export Database").clicked() {
                        app.state.export.show_export_dialog = true;
                    }

                    // 历史面板切换按钮
                    let history_button_text = if app.state.history.show_history_panel {
                        "📊 Hide History"
                    } else {
                        "📊 Show History"
                    };

                    if ui.button(history_button_text).clicked() {
                        app.state.history.show_history_panel = !app.state.history.show_history_panel;
                        
                        // 当显示历史面板时，自动刷新用户列表
                        if app.state.history.show_history_panel {
                            crate::app::ui::history_panel::refresh_history_sessions(app);
                        }
                    }

                });
            });
            ui.add_space(5.0);
        });
}

fn render_status_details(app: &SensorDataApp, ui: &mut egui::Ui) {
    if app.state.calibration.is_calibrating {
        if let Some(start_time) = app.state.calibration.calibration_start_time {
            let elapsed = start_time.elapsed().as_secs_f64();
            let progress = (elapsed / 5.0).min(1.0);
            ui.label(format!("auto calibrating... {:.1}s / 5.0s ({} samples)",
                             elapsed, app.state.calibration.calibration_data.len()));

            // 进度条
            let progress_bar = egui::ProgressBar::new(progress as f32)
                .desired_width(150.0);
            ui.add(progress_bar);
        } else {
            ui.label("waiting for data...");
        }
    } else if app.state.collection.is_collecting {
        ui.label("data collecting...");
    } else {
        ui.label("waiting for data...");
    }
}

pub fn render_bottom_status_bar(app: &mut SensorDataApp, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("bottom_status_bar")
        .min_height(25.0)
        .show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                // 左侧：保存状态
                if !app.state.collection.save_status.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(0, 100, 200), &app.state.collection.save_status);
                    ui.separator();
                }
                
                // 数据库连接状态
                ui.label("DB: DuckDB");
                ui.separator();
                
                
                // 文本阅读器状态
                if app.state.text_reader.is_enabled && app.state.text_reader.file_loaded {
                    ui.label(format!("📖 Reading: {}", app.state.get_text_info()));
                    ui.separator();
                }
                
                // 右侧：导出状态
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !app.state.export.export_status.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(0, 150, 100), &app.state.export.export_status);
                    }
                    
                });
            });
            ui.add_space(3.0);
        });
}
