use egui_plot::{Line, Plot, PlotPoints};
use egui::Color32;
use std::collections::VecDeque;

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

#[derive(Debug)]
pub struct WaveformPlot {
    buffer_x: VecDeque<f64>,
    buffer_y: VecDeque<f64>,
    buffer_z: VecDeque<f64>,
    buffer_timestamp: VecDeque<i64>, // 添加时间戳缓冲区
    audio_buffer: VecDeque<f64>,
    audio_timestamps: VecDeque<i64>, // 添加音频时间戳缓冲区
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
            buffer_x: VecDeque::with_capacity(max_samples),
            buffer_y: VecDeque::with_capacity(max_samples),
            buffer_z: VecDeque::with_capacity(max_samples),
            buffer_timestamp: VecDeque::with_capacity(max_samples), // 初始化时间戳缓冲区
            audio_buffer: VecDeque::with_capacity(audio_max_samples),
            audio_timestamps: VecDeque::with_capacity(audio_max_samples), // 初始化音频时间戳缓冲区
            max_samples,
            window_duration: window_seconds,
            audio_max_samples,
            audio_window_duration: audio_window_seconds,
        }
    }

    pub fn add_data(&mut self, x: f64, y: f64, z: f64, timestamp: i64) {
        // 将新数据添加到缓冲区末尾
        self.buffer_x.push_back(x);
        self.buffer_y.push_back(y);
        self.buffer_z.push_back(z);
        self.buffer_timestamp.push_back(timestamp);

        // 如果超过最大样本数，移除最旧的数据（从前面移除）- O(1)操作
        if self.buffer_x.len() > self.max_samples {
            self.buffer_x.pop_front();
            self.buffer_y.pop_front();
            self.buffer_z.pop_front();
            self.buffer_timestamp.pop_front();
        }
    }
    
    pub fn add_audio_samples(&mut self, samples: &[i16], base_timestamp: i64, sample_rate: u32) {
        // 批量转换音频样本为归一化的f64值 (-1.0 到 1.0)
        let normalized_samples: Vec<f64> = samples
            .iter()
            .map(|&sample| sample as f64 / 32768.0)
            .collect();
        
        // 计算每个样本的时间戳
        let sample_interval_ms = 1000.0 / sample_rate as f64;
        let timestamps: Vec<i64> = (0..samples.len())
            .map(|i| base_timestamp + (i as f64 * sample_interval_ms) as i64)
            .collect();
        
        // 批量添加到缓冲区末尾
        self.audio_buffer.extend(normalized_samples);
        self.audio_timestamps.extend(timestamps);
        
        // 如果超过最大样本数，批量移除最旧的数据 - O(1)操作
        while self.audio_buffer.len() > self.audio_max_samples {
            self.audio_buffer.pop_front();
            self.audio_timestamps.pop_front();
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

    fn plot_axis(&self, ui: &mut egui::Ui, title: &str, buffer: &VecDeque<f64>, color: Color32) {
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
            .y_axis_formatter(|v, _| format_fixed_width_y_label(v.value))
            .show_x(false)
            .show_y(false)
            .allow_drag(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                // 计算时间点：最旧的数据在左侧（时间=0），最新的数据在右侧（时间=window_duration）
                let data_len = buffer.len();
                if data_len == 0 {
                    return;
                }

                let dt = self.window_duration / (self.max_samples as f64);

                // 从左到右的时间轴：最旧数据时间为0，向右递增
                let points: Vec<[f64; 2]> = buffer
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        // 索引0是最旧的数据，索引data_len-1是最新的数据
                        let time = i as f64 * dt; // 正时间，从0开始递增
                        [time, y]
                    })
                    .collect();

                plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                    [0.0, y_min],
                    [self.window_duration, y_max],
                ));

                plot_ui.line(Line::new(title, PlotPoints::from(points)).color(color).width(1.0));
            });
    }
    
    fn plot_audio(&self, ui: &mut egui::Ui, title: &str, buffer: &VecDeque<f64>, color: Color32) {
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
            .y_axis_formatter(|v, _| format_fixed_width_y_label(v.value))
            .show_x(false)
            .show_y(false)
            .allow_drag(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                // 计算时间点：最旧的数据在左侧（时间=0），最新的数据在右侧（时间=window_duration）
                let data_len = buffer.len();
                if data_len == 0 {
                    return;
                }

                let dt = self.audio_window_duration / (self.audio_max_samples as f64);

                // 从左到右的时间轴：最旧数据时间为0，向右递增
                let points: Vec<[f64; 2]> = buffer
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        // 索引0是最旧的数据，索引data_len-1是最新的数据
                        let time = i as f64 * dt; // 正时间，从0开始递增
                        [time, y]
                    })
                    .collect();

                plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                    [0.0, y_min],
                    [self.audio_window_duration, y_max],
                ));

                plot_ui.line(Line::new(title, PlotPoints::from(points)).color(color).width(1.0));
            });
    }

    // 获取当前缓冲区数据的方法
    pub fn get_current_accelerometer_data(&self) -> Vec<(f64, f64, f64, i64)> {
        let mut data = Vec::new();
        for i in 0..self.buffer_x.len() {
            if let (Some(&x), Some(&y), Some(&z), Some(&timestamp)) = (
                self.buffer_x.get(i),
                self.buffer_y.get(i),
                self.buffer_z.get(i),
                self.buffer_timestamp.get(i)
            ) {
                data.push((x, y, z, timestamp));
            }
        }
        data
    }

    pub fn get_current_audio_data(&self) -> Vec<f64> {
        self.audio_buffer.iter().cloned().collect()
    }

    pub fn get_current_audio_data_with_timestamps(&self) -> Vec<(f64, i64)> {
        self.audio_buffer.iter()
            .zip(self.audio_timestamps.iter())
            .map(|(&sample, &timestamp)| (sample, timestamp))
            .collect()
    }

    pub fn get_current_audio_first_timestamp(&self) -> Option<i64> {
        self.audio_timestamps.front().copied()
    }

    pub fn get_current_audio_last_timestamp(&self) -> Option<i64> {
        self.audio_timestamps.back().copied()
    }

}