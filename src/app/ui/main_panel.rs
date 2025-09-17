use eframe::egui;
use crate::app::app_core::SensorDataApp;

pub fn render_main_panel(app: &mut SensorDataApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // 控制面板
        ui.horizontal(|ui| {
            // 快捷键说明
            ui.label("Hotkey:");
            ui.colored_label(egui::Color32::from_rgb(0, 150, 0), "SPACE");
            
            // 显示空格键的双重功能
            let mut functions = Vec::new();
            if app.state.text_reader.is_enabled {
                functions.push("Next text line");
            }
            if app.state.collection.is_collecting && !app.state.collection.is_paused {
                functions.push("Save data");
            } else if !app.state.text_reader.is_enabled {
                functions.push("Save current window data to database");
            }
            
            if functions.is_empty() {
                ui.label("(No active functions)");
            } else {
                ui.label(functions.join(" + "));
            }
            
            ui.separator();
            
            // 用户名输入框
            ui.label("Username:");
            ui.add(egui::TextEdit::singleline(&mut app.state.collection.username)
                .desired_width(100.0)
                .hint_text("Enter username"));
            
            // 场景输入框
            ui.label("Scenario:");
            let mut scenario_text = app.state.collection.scenario.clone();
            if scenario_text.is_empty() {
                scenario_text = "standard".to_string();
                app.state.collection.scenario = scenario_text.clone();
            }
            if ui.add(egui::TextEdit::singleline(&mut scenario_text)
                .desired_width(100.0)
                .hint_text("standard")).changed() {
                app.state.collection.scenario = if scenario_text.is_empty() {
                    "standard".to_string()
                } else {
                    scenario_text
                };
            }
            
            ui.separator();
            
            // 文本阅读器控制
            ui.label("Text Reader:");
            if ui.checkbox(&mut app.state.text_reader.is_enabled, "Enable").changed() {
                // 当启用/禁用文本阅读器时的处理
            }
            
            ui.separator();
            
            // 显示选项控制
            ui.label("Display:");
            let mut show_gyroscope = app.config.get_config().plot.show_gyroscope;
            if ui.checkbox(&mut show_gyroscope, "Show Gyroscope").changed() {
                // 更新配置
                app.config.get_config_mut().plot.show_gyroscope = show_gyroscope;
            }
        });
        ui.add_space(10.0);

        // 文本阅读器面板
        if app.state.text_reader.is_enabled && app.state.text_reader.file_loaded {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("📖 Text Reader");
                    ui.separator();
                    ui.label(format!("Line: {}", app.state.get_text_info()));
                    ui.separator();
                    ui.label("Controls: SPACE=Next, ←=Previous, →=Next");
                });
                
                ui.add_space(5.0);
                
                // 显示当前文本，使用较大的字体
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&app.state.text_reader.current_text)
                                .size(24.0)
                                .color(egui::Color32::BLACK)
                        ).wrap()
                    );
                });
            });
            ui.add_space(10.0);
        }
        
        app.state.waveform_plot.ui(ui, &app.config.get_config().plot);
    });
}
