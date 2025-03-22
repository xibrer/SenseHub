use egui_plot::{Line, Plot, PlotPoints};
use egui::Color32;

pub struct WaveformPlot {
    buffer_x: Vec<f64>,
    buffer_y: Vec<f64>,
    buffer_z: Vec<f64>,
    capacity: usize,
    write_index: usize,
    full: bool,
}

impl WaveformPlot {
    pub fn new(sample_rate: usize) -> Self {
        let window_seconds = 5.0;
        let capacity = (window_seconds * sample_rate as f64) as usize;

        Self {
            buffer_x: vec![0.0; capacity],
            buffer_y: vec![0.0; capacity],
            buffer_z: vec![0.0; capacity],
            capacity,
            write_index: 0,
            full: false,
        }
    }

    pub fn add_data(&mut self, x: f64, y: f64, z: f64) {
        self.buffer_x[self.write_index] = x;
        self.buffer_y[self.write_index] = y;
        self.buffer_z[self.write_index] = z;

        self.write_index += 1;
        if self.write_index >= self.capacity {
            self.write_index = 0;
            self.full = true;
        }
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                self.plot_axis(ui, "X Axis", &self.buffer_x, Color32::RED);
                self.plot_axis(ui, "Y Axis", &self.buffer_y, Color32::GREEN);
                self.plot_axis(ui, "Z Axis", &self.buffer_z, Color32::BLUE);
            });
        });
    }

    fn plot_axis(&self, ui: &mut egui::Ui, title: &str, buffer: &[f64], color: Color32) {
        let (ordered_data, data_count) = if self.full {
            // 当缓冲区满时，数据按时间顺序排序
            let mut data = Vec::with_capacity(self.capacity);
            data.extend_from_slice(&buffer[self.write_index..]);
            data.extend_from_slice(&buffer[..self.write_index]);
            (data, self.capacity)
        } else {
            // 当缓冲区未满时，只取有效数据
            (buffer[..self.write_index].to_vec(), self.write_index)
        };

        Plot::new(title)
            .height(150.0)
            .x_axis_formatter(|v, _| format!("{:.1}s", v.value))
            .y_axis_formatter(|v, _| format!("{:.2}", v.value))
            .show_x(false)
            .show_y(false)
            .allow_drag(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                // 计算动态范围
                let y_min = ordered_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let y_max = ordered_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                let y_margin = (y_max - y_min).abs().max(0.1) * 0.05;
                let y_min = y_min - y_margin;
                let y_max = y_max + y_margin;

                // 生成时间轴（最近的5秒数据）
                let points: Vec<[f64; 2]> = ordered_data
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        let x_position = 5.0 - (data_count - i) as f64 / self.capacity as f64 * 5.0;
                        [x_position, y]
                    })
                    .collect();

                plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                    [0.0, y_min],
                    [5.0, y_max],
                ));

                plot_ui.line(Line::new(PlotPoints::from(points)).color(color).width(1.0));
            });
    }
}