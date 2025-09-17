use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use egui::Color32;
use crate::app::app_core::SensorDataApp;
use crate::types::DataPoint;
use super::history_controls::*;

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






