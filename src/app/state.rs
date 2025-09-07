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
    pub current_session_id: String,
    pub save_status: String,
}

impl Default for CollectionState {
    fn default() -> Self {
        Self {
            is_collecting: false,
            current_session_id: String::new(),
            save_status: String::new(),
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

/// 统一的应用状态管理
#[derive(Debug)]
pub struct AppState {
    pub collection: CollectionState,
    pub calibration: CalibrationState,
    pub export: ExportState,
    pub database: DatabaseState,
    pub channels: DataChannels,
    pub waveform_plot: WaveformPlot,
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
        }
    }

    /// 获取当前状态摘要
    pub fn get_status_summary(&self) -> String {
        if self.calibration.is_calibrating {
            "Calibrating".to_string()
        } else if self.collection.is_collecting {
            "Collecting".to_string()
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
    }

    /// 开始采集
    pub fn start_collection(&mut self) {
        self.collection.is_collecting = true;
    }
}
