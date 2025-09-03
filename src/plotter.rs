use egui_plot::{Line, Plot, PlotPoints};
use egui::Color32;

pub struct WaveformPlot {
    buffer_x: Vec<f64>,
    buffer_y: Vec<f64>,
    buffer_z: Vec<f64>,
    audio_buffer: Vec<f64>,
    capacity: usize,
    write_index: usize,
    full: bool,
    // 音频相关
    audio_capacity: usize,
    audio_write_index: usize,
    audio_full: bool,
}

impl WaveformPlot {
    pub fn new(sample_rate: usize) -> Self {
        let window_seconds = 5.0;
        let capacity = (window_seconds * sample_rate as f64) as usize;
        
        // 音频缓冲区 - 直接使用16kHz音频数据，不下采样
        let audio_window_seconds = 5.0; // 显示5秒的音频数据
        let audio_sample_rate = 16000; // 16kHz完整采样率
        let audio_capacity = (audio_window_seconds * audio_sample_rate as f64) as usize;

        Self {
            buffer_x: vec![0.0; capacity],
            buffer_y: vec![0.0; capacity],
            buffer_z: vec![0.0; capacity],
            audio_buffer: vec![0.0; audio_capacity],
            capacity,
            write_index: 0,
            full: false,
            audio_capacity,
            audio_write_index: 0,
            audio_full: false,
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
    
    pub fn add_audio_samples(&mut self, samples: &[i16]) {
        // 控制音频数据的添加速率，使其与时间轴同步
        // 16kHz音频数据需要按照实际时间间隔添加到缓冲区
        for &sample in samples {
            // 将i16样本转换为归一化的f64值 (-1.0 到 1.0)
            let normalized_sample = sample as f64 / 32768.0;
            self.audio_buffer[self.audio_write_index] = normalized_sample;
            
            self.audio_write_index += 1;
            if self.audio_write_index >= self.audio_capacity {
                self.audio_write_index = 0;
                self.audio_full = true;
            }
        }
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                self.plot_axis(ui, "X Axis", &self.buffer_x, Color32::RED);
                self.plot_axis(ui, "Y Axis", &self.buffer_y, Color32::GREEN);
                self.plot_axis(ui, "Z Axis", &self.buffer_z, Color32::BLUE);
                
                // 添加音频波形显示
                self.plot_audio(ui, "Audio Waveform", &self.audio_buffer, Color32::PURPLE);
            });
        });
    }

    fn plot_axis(&self, ui: &mut egui::Ui, title: &str, buffer: &[f64], color: Color32) {
        // 如果未满则使用 [0, write_index)，满了则使用整个缓冲区，保持自然顺序
        let (data, data_count) = if self.full {
            (buffer, self.capacity)
        } else {
            (&buffer[..self.write_index], self.write_index)
        };

        if data.is_empty() {
            return;
        }

        // 计算动态Y轴范围
        let (y_min, y_max) = data.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(min, max), &val| (min.min(val), max.max(val))
        );

        let range = (y_max - y_min).max(0.1);
        let y_min = y_min - range * 0.05;
        let y_max = y_max + range * 0.05;

        Plot::new(title)
            .height(150.0)
            .x_axis_formatter(|v, _| format!("{:.1}s", v.value))
            .y_axis_formatter(|v, _| format!("{:.2}", v.value))
            .show_x(false)
            .show_y(false)
            .allow_drag(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                let total_duration = 5.0; // 总时间窗口 5 秒
                let dt = total_duration / (self.capacity as f64 - 1.0);

                // 根据缓冲区的自然顺序计算每个点的时间：索引0 => 0秒，索引capacity-1 => 5秒
                let points: Vec<[f64; 2]> = data
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        let time = i as f64 * dt;
                        [time, y]
                    })
                    .collect();

                plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                    [0.0, y_min],
                    [5.0, y_max],
                ));

                plot_ui.line(Line::new(title, PlotPoints::from(points)).color(color).width(1.0));
            });
    }
    
    fn plot_audio(&self, ui: &mut egui::Ui, title: &str, buffer: &[f64], color: Color32) {
        // 音频数据处理逻辑与传感器数据类似，但使用音频特定的参数
        let (data, _data_count) = if self.audio_full {
            (buffer, self.audio_capacity)
        } else {
            (&buffer[..self.audio_write_index], self.audio_write_index)
        };

        if data.is_empty() {
            return;
        }

        // 计算音频数据的动态Y轴范围
        let (y_min, y_max) = data.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(min, max), &val| (min.min(val), max.max(val))
        );

        let range = (y_max - y_min).max(0.1);
        let y_min = y_min - range * 0.05;
        let y_max = y_max + range * 0.05;

        Plot::new(title)
            .height(150.0)
            .x_axis_formatter(|v, _| format!("{:.2}s", v.value))
            .y_axis_formatter(|v, _| format!("{:.3}", v.value))
            .show_x(false)
            .show_y(false)
            .allow_drag(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                let total_duration = 5.0; // 音频显示窗口 5 秒
                let dt = total_duration / (self.audio_capacity as f64 - 1.0);

                // 根据缓冲区的自然顺序计算每个点的时间
                let points: Vec<[f64; 2]> = data
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        let time = i as f64 * dt;
                        [time, y]
                    })
                    .collect();

                plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                    [0.0, y_min],
                    [5.0, y_max],
                ));

                plot_ui.line(Line::new(title, PlotPoints::from(points)).color(color).width(1.0));
            });
    }

}