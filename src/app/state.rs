use std::collections::HashSet;
use std::time::Instant;
use crossbeam_channel::{Receiver, Sender};
use crate::types::{DataPoint, AudioData, DatabaseTask, SaveResult, ExportResult};
use crate::plotter::WaveformPlot;

/// 应用状态管理模块
/// 将原本分散在SensorDataApp中的状态分离到独立的结构体中

/// 数据采集状态
#[derive(Debug, Clone)]
pub struct CollectionState {
    pub is_collecting: bool,
    pub is_paused: bool,
    pub current_session_id: String,
    pub save_status: String,
    pub username: String,
    pub scenario: String,
}

impl Default for CollectionState {
    fn default() -> Self {
        Self {
            is_collecting: false,
            is_paused: false,
            current_session_id: String::new(),
            save_status: String::new(),
            username: "test".to_string(),
            scenario: "standard".to_string(),
        }
    }
}

/// 校准状态
#[derive(Debug, Clone)]
pub struct CalibrationState {
    pub is_calibrating: bool,
    pub calibration_data: Vec<DataPoint>,
    pub calibration_start_time: Option<Instant>,
    pub calculated_sample_rate: Option<f64>,
}

impl Default for CalibrationState {
    fn default() -> Self {
        Self {
            is_calibrating: true, // 启动时自动开始校准
            calibration_data: Vec::new(),
            calibration_start_time: None,
            calculated_sample_rate: None,
        }
    }
}

/// 导出状态
#[derive(Debug, Clone)]
pub struct ExportState {
    pub export_status: String,
    pub show_export_dialog: bool,
    pub available_sessions: Vec<String>,
    pub selected_sessions: HashSet<String>,
    pub export_result_receiver: Option<crossbeam_channel::Receiver<ExportResult>>,
    pub sessions_result_receiver: Option<crossbeam_channel::Receiver<Vec<String>>>,
}

/// 历史数据显示选项
#[derive(Debug, Clone)]
pub struct HistoryDisplayOptions {
    pub show_x_axis: bool,
    pub show_y_axis: bool,
    pub show_z_axis: bool,
    pub show_gx_axis: bool,
    pub show_gy_axis: bool,
    pub show_gz_axis: bool,
    pub show_audio: bool,
}

impl Default for HistoryDisplayOptions {
    fn default() -> Self {
        Self {
            show_x_axis: true,
            show_y_axis: true,
            show_z_axis: true,
            show_gx_axis: false,  // 默认不显示陀螺仪，避免界面过于拥挤
            show_gy_axis: false,
            show_gz_axis: false,
            show_audio: true,
        }
    }
}

/// 历史数据可视化状态
#[derive(Debug, Clone)]
pub struct HistoryVisualizationState {
    pub show_history_panel: bool,
    pub selected_username: Option<String>,
    pub selected_session: Option<String>,
    pub loaded_history_data: Vec<DataPoint>,
    pub loaded_audio_data: Vec<f64>,
    pub original_history_data: Vec<DataPoint>,
    pub original_audio_data: Vec<f64>,
    pub aligned_history_data: Vec<DataPoint>,
    pub aligned_audio_data: Vec<f64>,
    pub display_options: HistoryDisplayOptions,
    pub loading_status: String,
    pub available_usernames: Vec<String>,
    pub history_sessions: Vec<String>,
    pub history_result_receiver: Option<crossbeam_channel::Receiver<(Vec<DataPoint>, Vec<f64>)>>,
    pub aligned_history_result_receiver: Option<crossbeam_channel::Receiver<(Vec<DataPoint>, Vec<f64>, i64)>>,
    pub common_time_range_ms: i64,
    pub sessions_result_receiver: Option<crossbeam_channel::Receiver<Vec<String>>>,
    pub usernames_result_receiver: Option<crossbeam_channel::Receiver<Vec<String>>>,
    pub panel_width: f32,
    pub show_aligned_data: bool,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            export_status: String::new(),
            show_export_dialog: false,
            available_sessions: Vec::new(),
            selected_sessions: HashSet::new(),
            export_result_receiver: None,
            sessions_result_receiver: None,
        }
    }
}

impl Default for HistoryVisualizationState {
    fn default() -> Self {
        Self {
            show_history_panel: false,
            selected_username: None,
            selected_session: None,
            loaded_history_data: Vec::new(),
            loaded_audio_data: Vec::new(),
            original_history_data: Vec::new(),
            original_audio_data: Vec::new(),
            aligned_history_data: Vec::new(),
            aligned_audio_data: Vec::new(),
            display_options: HistoryDisplayOptions::default(),
            loading_status: String::new(),
            available_usernames: Vec::new(),
            history_sessions: Vec::new(),
            history_result_receiver: None,
            aligned_history_result_receiver: None,
            common_time_range_ms: 0,
            sessions_result_receiver: None,
            usernames_result_receiver: None,
            panel_width: 300.0, // 默认侧边面板宽度
            show_aligned_data: true, // 默认显示对齐后的数据
        }
    }
}

/// 数据库状态
#[derive(Debug, Clone)]
pub struct DatabaseState {
    pub db_task_sender: Sender<DatabaseTask>,
    pub save_result_receiver: Receiver<SaveResult>,
    pub last_audio_metadata: Option<AudioData>,
}

/// 数据通道状态
#[derive(Debug)]
pub struct DataChannels {
    pub data_receiver: Receiver<DataPoint>,
    pub audio_receiver: Receiver<AudioData>,
}

/// 文本阅读器状态
#[derive(Debug, Clone)]
pub struct TextReaderState {
    pub lines: Vec<String>,
    pub current_line_index: usize,
    pub current_text: String,
    pub is_enabled: bool,
    pub file_loaded: bool,
}

impl Default for TextReaderState {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            current_line_index: 0,
            current_text: String::new(),
            is_enabled: false,
            file_loaded: false,
        }
    }
}

/// 统一的应用状态管理
#[derive(Debug)]
pub struct AppState {
    pub collection: CollectionState,
    pub calibration: CalibrationState,
    pub export: ExportState,
    pub history: HistoryVisualizationState,
    pub database: DatabaseState,
    pub channels: DataChannels,
    pub waveform_plot: WaveformPlot,
    pub text_reader: TextReaderState,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(
        data_receiver: Receiver<DataPoint>,
        audio_receiver: Receiver<AudioData>,
        db_task_sender: Sender<DatabaseTask>,
        save_result_receiver: Receiver<SaveResult>,
    ) -> Self {
        let initial_sample_rate = 393; // 初始采样率

        Self {
            collection: CollectionState::default(),
            calibration: CalibrationState::default(),
            export: ExportState::default(),
            history: HistoryVisualizationState::default(),
            database: DatabaseState {
                db_task_sender,
                save_result_receiver,
                last_audio_metadata: None,
            },
            channels: DataChannels {
                data_receiver,
                audio_receiver,
            },
            waveform_plot: WaveformPlot::new(initial_sample_rate),
            text_reader: TextReaderState::default(),
        }
    }

    /// 获取当前状态摘要
    pub fn get_status_summary(&self) -> String {
        if self.calibration.is_calibrating {
            "Calibrating".to_string()
        } else if self.collection.is_collecting {
            if self.collection.is_paused {
                "Paused".to_string()
            } else {
                "Collecting".to_string()
            }
        } else {
            "Stopped".to_string()
        }
    }

    /// 检查是否有数据需要保存
    pub fn has_data_to_save(&self) -> bool {
        !self.waveform_plot.get_current_accelerometer_data().is_empty() ||
            !self.waveform_plot.get_current_audio_data().is_empty()
    }

    /// 重置校准状态
    pub fn reset_calibration(&mut self) {
        self.calibration.calibration_data.clear();
        self.calibration.calibration_start_time = None;
        self.calibration.calculated_sample_rate = None;
        self.calibration.is_calibrating = true;
    }

    /// 完成校准并开始采集
    pub fn complete_calibration(&mut self, sample_rate: f64) {
        self.calibration.is_calibrating = false;
        self.calibration.calculated_sample_rate = Some(sample_rate);
        self.collection.is_collecting = true;

        // 使用计算出的采样率重新创建 WaveformPlot
        self.waveform_plot = WaveformPlot::new(sample_rate as usize);

        // 清空校准数据
        self.calibration.calibration_data.clear();
        self.calibration.calibration_start_time = None;
    }

    /// 停止采集
    pub fn stop_collection(&mut self) {
        self.collection.is_collecting = false;
        self.collection.is_paused = false;
    }

    /// 开始采集
    pub fn start_collection(&mut self) {
        self.collection.is_collecting = true;
        self.collection.is_paused = false;
    }

    /// 暂停采集
    pub fn pause_collection(&mut self) {
        if self.collection.is_collecting {
            self.collection.is_paused = true;
        }
    }

    /// 恢复采集
    pub fn resume_collection(&mut self) {
        if self.collection.is_collecting {
            self.collection.is_paused = false;
        }
    }

    /// 检查是否正在活跃采集（采集中且未暂停）
    pub fn is_actively_collecting(&self) -> bool {
        self.collection.is_collecting && !self.collection.is_paused
    }

    /// 加载文本文件
    pub fn load_text_file(&mut self, file_path: &str) -> Result<(), String> {
        use std::fs;
        match fs::read_to_string(file_path) {
            Ok(content) => {
                self.text_reader.lines = content.lines().map(|s| s.to_string()).collect();
                self.text_reader.current_line_index = 0;
                self.text_reader.file_loaded = true;
                if !self.text_reader.lines.is_empty() {
                    self.text_reader.current_text = self.text_reader.lines[0].clone();
                }
                Ok(())
            }
            Err(e) => Err(format!("Failed to load file: {}", e))
        }
    }

    /// 切换到下一行文本
    pub fn next_text_line(&mut self) {
        if !self.text_reader.file_loaded || self.text_reader.lines.is_empty() {
            return;
        }
        
        if self.text_reader.current_line_index + 1 < self.text_reader.lines.len() {
            self.text_reader.current_line_index += 1;
            self.text_reader.current_text = self.text_reader.lines[self.text_reader.current_line_index].clone();
        }
    }

    /// 切换到上一行文本
    pub fn previous_text_line(&mut self) {
        if !self.text_reader.file_loaded || self.text_reader.lines.is_empty() {
            return;
        }
        
        if self.text_reader.current_line_index > 0 {
            self.text_reader.current_line_index -= 1;
            self.text_reader.current_text = self.text_reader.lines[self.text_reader.current_line_index].clone();
        }
    }

    /// 获取当前文本行信息
    pub fn get_text_info(&self) -> String {
        if !self.text_reader.file_loaded {
            return "No file loaded".to_string();
        }
        format!("{}/{}", self.text_reader.current_line_index + 1, self.text_reader.lines.len())
    }
}
