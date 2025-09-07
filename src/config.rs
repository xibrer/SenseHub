use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 应用配置管理模块
/// 集中管理所有配置项，提供默认值和配置验证

/// 主配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub window: WindowConfig,
    pub database: DatabaseConfig,
    pub mqtt: MqttConfig,
    pub plot: PlotConfig,
    pub calibration: CalibrationConfig,
    pub channels: ChannelConfig,
}

/// 窗口配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub width: f32,
    pub height: f32,
    pub title: String,
    pub resizable: bool,
    pub vsync: bool,
    pub hardware_acceleration: bool,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
    pub channel_capacity: usize,
    pub auto_create_dir: bool,
}

/// MQTT配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    pub broker: String,
    pub port: u16,
    pub client_id: String,
    pub topics: MqttTopics,
    pub qos: u8,
    pub keep_alive: u16,
}

/// MQTT主题配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttTopics {
    pub accelerometer: String,
    pub audio: String,
}

/// 绘图配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotConfig {
    pub window_duration_seconds: f64,
    pub audio_window_duration_seconds: f64,
    pub plot_height: f32,
    pub show_axes: bool,
    pub allow_drag: bool,
    pub allow_zoom: bool,
    pub colors: PlotColors,
}

/// 绘图颜色配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotColors {
    pub x_axis: [u8; 3],
    pub y_axis: [u8; 3],
    pub z_axis: [u8; 3],
    pub audio: [u8; 3],
}

/// 校准配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationConfig {
    pub duration_seconds: f64,
    pub min_samples: usize,
    pub initial_sample_rate: usize,
    pub auto_start: bool,
}

/// 通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub data_channel_capacity: usize,
    pub audio_channel_capacity: usize,
    pub db_task_channel_capacity: usize,
    pub save_result_channel_capacity: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            database: DatabaseConfig::default(),
            mqtt: MqttConfig::default(),
            plot: PlotConfig::default(),
            calibration: CalibrationConfig::default(),
            channels: ChannelConfig::default(),
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1200.0,
            height: 800.0,
            title: "SenseHub - Sensor Data Viewer".to_string(),
            resizable: true,
            vsync: true,
            hardware_acceleration: true,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: "data/sensor_data.db".to_string(),
            channel_capacity: 100,
            auto_create_dir: true,
        }
    }
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            broker: "localhost".to_string(),
            port: 1883,
            client_id: "sensehub_client".to_string(),
            topics: MqttTopics::default(),
            qos: 1,
            keep_alive: 60,
        }
    }
}

impl Default for MqttTopics {
    fn default() -> Self {
        Self {
            accelerometer: "sensor/accelerometer".to_string(),
            audio: "sensor/audio".to_string(),
        }
    }
}

impl Default for PlotConfig {
    fn default() -> Self {
        Self {
            window_duration_seconds: 5.0,
            audio_window_duration_seconds: 5.0,
            plot_height: 150.0,
            show_axes: false,
            allow_drag: false,
            allow_zoom: false,
            colors: PlotColors::default(),
        }
    }
}

impl Default for PlotColors {
    fn default() -> Self {
        Self {
            x_axis: [255, 0, 0],    // 红色
            y_axis: [0, 255, 0],    // 绿色
            z_axis: [0, 0, 255],    // 蓝色
            audio: [128, 0, 128],   // 紫色
        }
    }
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            duration_seconds: 5.0,
            min_samples: 2,
            initial_sample_rate: 393,
            auto_start: true,
        }
    }
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            data_channel_capacity: 5000,
            audio_channel_capacity: 10000,
            db_task_channel_capacity: 100,
            save_result_channel_capacity: 100,
        }
    }
}

impl AppConfig {
    /// 从文件加载配置
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::IoError(e))?;
        
        let config: AppConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(e))?;
        
        config.validate()?;
        Ok(config)
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializeError(e))?;
        
        std::fs::write(path, content)
            .map_err(|e| ConfigError::IoError(e))?;
        
        Ok(())
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.window.width <= 0.0 || self.window.height <= 0.0 {
            return Err(ConfigError::ValidationError("Window dimensions must be positive".to_string()));
        }

        if self.calibration.duration_seconds <= 0.0 {
            return Err(ConfigError::ValidationError("Calibration duration must be positive".to_string()));
        }

        if self.calibration.min_samples < 2 {
            return Err(ConfigError::ValidationError("Minimum samples must be at least 2".to_string()));
        }

        if self.channels.data_channel_capacity == 0 {
            return Err(ConfigError::ValidationError("Data channel capacity must be positive".to_string()));
        }

        Ok(())
    }

    /// 获取数据库文件路径
    pub fn get_database_path(&self) -> PathBuf {
        PathBuf::from(&self.database.path)
    }

    /// 获取数据目录路径
    pub fn get_data_directory(&self) -> PathBuf {
        self.get_database_path().parent().unwrap_or(std::path::Path::new(".")).to_path_buf()
    }
}

/// 配置错误类型
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    IoError(std::io::Error),
    #[error("Parse error: {0}")]
    ParseError(toml::de::Error),
    #[error("Serialize error: {0}")]
    SerializeError(toml::ser::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// 配置管理器
pub struct ConfigManager {
    config: AppConfig,
    config_path: Option<PathBuf>,
}

impl ConfigManager {
    /// 创建配置管理器
    pub fn new() -> Self {
        Self {
            config: AppConfig::default(),
            config_path: None,
        }
    }

    /// 从文件加载配置
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ConfigError> {
        let config = AppConfig::load_from_file(&path)?;
        Ok(Self {
            config,
            config_path: Some(path.as_ref().to_path_buf()),
        })
    }

    /// 获取当前配置
    pub fn get_config(&self) -> &AppConfig {
        &self.config
    }

    /// 获取可变配置
    pub fn get_config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    /// 保存配置
    pub fn save(&self) -> Result<(), ConfigError> {
        if let Some(path) = &self.config_path {
            self.config.save_to_file(path)?;
        }
        Ok(())
    }

    /// 保存配置到指定文件
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), ConfigError> {
        self.config.save_to_file(path)
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}
