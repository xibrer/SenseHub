use egui_plot::{Line, Plot, PlotPoints};
use egui::Color32;

pub struct WaveformPlot {
    buffer_x: Vec<f64>,
    buffer_y: Vec<f64>,
    buffer_z: Vec<f64>,
    audio_buffer: Vec<f64>,
    max_samples: usize,
    window_duration: f64, // 窗口持续时间（秒）
    // 音频相关
    audio_max_samples: usize,
    audio_window_duration: f64,
}

impl WaveformPlot {
    pub fn new(sample_rate: usize) -> Self {
        let window_seconds = 5.0;
        let max_samples = (window_seconds * sample_rate as f64) as usize;
        
        // 音频缓冲区 - 直接使用16kHz音频数据，不下采样
        let audio_window_seconds = 5.0; // 显示5秒的音频数据
        let audio_sample_rate = 16000; // 16kHz完整采样率
        let audio_max_samples = (audio_window_seconds * audio_sample_rate as f64) as usize;

        Self {
            buffer_x: Vec::new(),
            buffer_y: Vec::new(),
            buffer_z: Vec::new(),
            audio_buffer: Vec::new(),
            max_samples,
            window_duration: window_seconds,
            audio_max_samples,
            audio_window_duration: audio_window_seconds,
        }
    }

    pub fn add_data(&mut self, x: f64, y: f64, z: f64) {
        // 将新数据添加到缓冲区末尾
        self.buffer_x.push(x);
        self.buffer_y.push(y);
        self.buffer_z.push(z);

        // 如果超过最大样本数，移除最旧的数据（从前面移除）
        if self.buffer_x.len() > self.max_samples {
            self.buffer_x.remove(0);
            self.buffer_y.remove(0);
            self.buffer_z.remove(0);
        }
    }
    
    pub fn add_audio_samples(&mut self, samples: &[i16]) {
        // 控制音频数据的添加速率，使其与时间轴同步
        // 16kHz音频数据需要按照实际时间间隔添加到缓冲区
        for &sample in samples {
            // 将i16样本转换为归一化的f64值 (-1.0 到 1.0)
            let normalized_sample = sample as f64 / 32768.0;
            self.audio_buffer.push(normalized_sample);
            
            // 如果超过最大样本数，移除最旧的数据（从前面移除）
            if self.audio_buffer.len() > self.audio_max_samples {
                self.audio_buffer.remove(0);
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
        if buffer.is_empty() {
            return;
        }

        // 计算动态Y轴范围
        let (y_min, y_max) = buffer.iter().fold(
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
                // 计算时间点：最新的数据在右侧（时间=0），最旧的数据在左侧（时间=-window_duration）
                let data_len = buffer.len();
                if data_len == 0 {
                    return;
                }

                let dt = self.window_duration / (self.max_samples as f64);

                // 从右到左的时间轴：最新数据时间为0，向左递减
                let points: Vec<[f64; 2]> = buffer
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        // 索引0是最旧的数据，索引data_len-1是最新的数据
                        let time_offset = (data_len - 1 - i) as f64 * dt;
                        let time = -time_offset; // 负时间表示过去
                        [time, y]
                    })
                    .collect();

                plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                    [-self.window_duration, y_min],
                    [0.0, y_max],
                ));

                plot_ui.line(Line::new(title, PlotPoints::from(points)).color(color).width(1.0));
            });
    }
    
    fn plot_audio(&self, ui: &mut egui::Ui, title: &str, buffer: &[f64], color: Color32) {
        if buffer.is_empty() {
            return;
        }

        // 计算音频数据的动态Y轴范围
        let (y_min, y_max) = buffer.iter().fold(
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
                // 计算时间点：最新的数据在右侧（时间=0），最旧的数据在左侧（时间=-window_duration）
                let data_len = buffer.len();
                if data_len == 0 {
                    return;
                }

                let dt = self.audio_window_duration / (self.audio_max_samples as f64);

                // 从右到左的时间轴：最新数据时间为0，向左递减
                let points: Vec<[f64; 2]> = buffer
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        // 索引0是最旧的数据，索引data_len-1是最新的数据
                        let time_offset = (data_len - 1 - i) as f64 * dt;
                        let time = -time_offset; // 负时间表示过去
                        [time, y]
                    })
                    .collect();

                plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                    [-self.audio_window_duration, y_min],
                    [0.0, y_max],
                ));

                plot_ui.line(Line::new(title, PlotPoints::from(points)).color(color).width(1.0));
            });
    }

}