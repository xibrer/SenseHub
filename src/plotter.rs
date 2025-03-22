use egui_plot::{Line, Plot, PlotPoints};
use egui::Color32;
use std::collections::VecDeque;

pub struct WaveformPlot {
    data_x: VecDeque<f64>, // 只存储传感器数值
    data_y: VecDeque<f64>,
    data_z: VecDeque<f64>,
    capacity: usize,       // 五秒数据容量（根据采样率计算）
}

impl WaveformPlot {
    pub fn new(sample_rate: usize) -> Self {
        let window_seconds = 5.0;
        let capacity = (window_seconds * sample_rate as f64) as usize;

        Self {
            data_x: VecDeque::with_capacity(capacity),
            data_y: VecDeque::with_capacity(capacity),
            data_z: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn add_data(&mut self, x: f64, y: f64, z: f64) {
        // 分别处理每个队列，不要共享self的可变借用
        self.data_x.push_back(x);
        while self.data_x.len() > self.capacity {
            self.data_x.pop_front();
        }

        self.data_y.push_back(y);
        while self.data_y.len() > self.capacity {
            self.data_y.pop_front();
        }

        self.data_z.push_back(z);
        while self.data_z.len() > self.capacity {
            self.data_z.pop_front();
        }
    }


    pub fn ui(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                self.plot_axis(ui, "X Axis", &self.data_x, Color32::RED);
                self.plot_axis(ui, "Y Axis", &self.data_y, Color32::GREEN);
                self.plot_axis(ui, "Z Axis", &self.data_z, Color32::BLUE);
            });
        });
    }

    fn plot_axis(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        data: &VecDeque<f64>,
        color: Color32,
    ) {
        Plot::new(title)
            .height(150.0)
            .x_axis_formatter(|v, _| format!("{:.1}s", v.value))
            .y_axis_formatter(|v, _| format!("{:.2}", v.value))
            .show_x(false)
            .show_y(false)
            .allow_drag(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                // 自动计算纵坐标范围
                let y_min = data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let y_max = data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

                // 添加5%的边距
                let y_margin = (y_max - y_min).abs().max(0.1) * 0.05;
                let y_min = y_min - y_margin;
                let y_max = y_max + y_margin;

                // 保持横坐标固定0-5秒，纵坐标动态调整
                plot_ui.set_plot_bounds(
                    egui_plot::PlotBounds::from_min_max(
                        [0.0, y_min],  // X固定从0开始
                        [5.0, y_max]   // X固定5秒结束
                    )
                );

                // 生成等间距时间坐标（保持不变）
                let points: Vec<[f64; 2]> = data.iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        let x_position = (i as f64) * 5.0 / (self.capacity as f64 - 1.0);
                        [x_position, y]
                    })
                    .collect();

                plot_ui.line(Line::new(PlotPoints::from(points)).color(color).width(1.0));
            });
    }
}