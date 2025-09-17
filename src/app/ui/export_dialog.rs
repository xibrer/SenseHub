use eframe::egui;
use crate::app::app_core::SensorDataApp;
use crate::app::handlers::ExportHandler;

pub fn render_export_dialog(app: &mut SensorDataApp, ctx: &egui::Context) {
    if app.state.export.show_export_dialog {
        egui::Window::new("Export Database Data")
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.label("Export session data from database to CSV files (one file per session)");
                ui.add_space(10.0);
                
                // Refresh session list button
                if ui.button("🔄 Refresh Session List").clicked() {
                    ExportHandler::refresh_sessions(app);
                }
                
                ui.add_space(10.0);
                
                render_session_list(app, ui);
                
                ui.add_space(10.0);
                
                render_export_buttons(app, ui);
                
                ui.add_space(5.0);
                ui.label("Note: Each session will be exported as a separate CSV file, filename format: session_id.csv");
            });
    }
}

fn render_session_list(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    if app.state.export.sessions_with_export_status.is_empty() {
        ui.label("No session data found");
    } else {
        let total_sessions = app.state.export.sessions_with_export_status.len();
        let exported_count = app.state.export.sessions_with_export_status.iter()
            .filter(|(_, is_exported)| *is_exported).count();
        let unexported_count = total_sessions - exported_count;
        
        ui.label(format!("Found {} sessions ({} exported, {} unexported):", 
                        total_sessions, exported_count, unexported_count));
        ui.add_space(5.0);
        
        // Session selection list
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for (session_id, is_exported) in &app.state.export.sessions_with_export_status.clone() {
                    ui.horizontal(|ui| {
                        let mut selected = app.state.export.selected_sessions.contains(session_id);
                        if ui.checkbox(&mut selected, "").changed() {
                            if selected {
                                app.state.export.selected_sessions.insert(session_id.clone());
                            } else {
                                app.state.export.selected_sessions.remove(session_id);
                            }
                        }
                        ui.label(session_id);
                        
                        // Show export status with better visual indicators
                        if *is_exported {
                            ui.colored_label(egui::Color32::GRAY, "✓ Exported");
                        } else {
                            ui.colored_label(egui::Color32::GREEN, "● New");
                        }
                    });
                }
            });
    }
}

fn render_export_buttons(app: &mut SensorDataApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button("✅ Export Selected Sessions").clicked() {
            ExportHandler::export_selected_sessions(app);
            app.state.export.show_export_dialog = false;
        }
        
        if ui.button("📤 Export All New Sessions").clicked() {
            ExportHandler::export_new_sessions_only(app);
            app.state.export.show_export_dialog = false;
        }
        
        if ui.button("❌ Cancel").clicked() {
            app.state.export.show_export_dialog = false;
        }
    });
}
