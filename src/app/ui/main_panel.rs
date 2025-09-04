use eframe::egui;
use crate::app::sensor_app::SensorDataApp;

pub fn render_main_panel(app: &mut SensorDataApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Add instruction text
        ui.horizontal(|ui| {
            ui.label("Hotkey:");
            ui.colored_label(egui::Color32::from_rgb(0, 150, 0), "SPACE");
            ui.label("Save current window data to database");
        });
        ui.add_space(10.0);
        
        app.state.waveform_plot.ui(ui);
    });
}
