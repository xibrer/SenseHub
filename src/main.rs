mod logger;
mod plotter;
mod utils;
use dotenv::dotenv;
use std::env;
use std::error;
use rumqttc::{Client, Event, LastWill, MqttOptions, Packet, Publish, QoS};
use serde_json::Value;
use crossbeam_channel::{bounded, Receiver, Sender};
use eframe::{egui, Frame};
use log::{error, info, warn};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use std::collections::VecDeque;
use base64::{Engine as _, engine::general_purpose};
use utils::format_timestamp;

#[derive(serde::Deserialize)]
struct DataPoint {
    x: f64,
    y: f64,
    z: f64,
    timestamp: i64,
}

#[derive(serde::Deserialize)]
struct AudioData {
    audio_data: String,  // Base64 encoded audio data
    sample_rate: u32,
    channels: u8,
    format: String,
    samples: usize,
    timestamp: i64,
}

fn main() {
    logger::init_logger();
    info!("Application starting");

    let (data_sender, data_receiver) = bounded(5000);
    let (audio_sender, audio_receiver) = bounded(1000);
    let data_sender = Arc::new(data_sender);
    let audio_sender = Arc::new(audio_sender);
    let shutdown_signal = Arc::new(AtomicBool::new(false));

    let mqtt_data_sender = Arc::clone(&data_sender);
    let mqtt_audio_sender = Arc::clone(&audio_sender);
    let mqtt_shutdown = Arc::clone(&shutdown_signal);
    let mqtt_handle = thread::spawn(move || {
        if let Err(e) = run_mqtt_client(mqtt_data_sender, mqtt_audio_sender, mqtt_shutdown) {
            error!("MQTT thread failed: {}", e);
        }
    });

    let options = eframe::NativeOptions {
        vsync: true,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred, // 硬件加速优先模式
        renderer: eframe::Renderer::Glow, // 使用Glow渲染器获得更好性能
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_resizable(true),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Sensor Data Viewer",
        options,
        Box::new(|cc| Ok(Box::new(SensorDataApp::new(data_receiver, audio_receiver)))),
    ) {
        error!("GUI failed: {}", e);
        std::process::exit(1);
    }

    // GUI 关闭后，发送关闭信号给 MQTT 线程
    info!("GUI closed, signaling MQTT thread to shutdown");
    shutdown_signal.store(true, Ordering::Relaxed);

    // 等待 MQTT 线程优雅退出，最多等待 3 秒
    let join_result = thread::spawn(move || {
        mqtt_handle.join()
    });
    
    match join_result.join() {
        Ok(Ok(())) => info!("MQTT thread shut down gracefully"),
        Ok(Err(e)) => error!("MQTT thread panicked: {:?}", e),
        Err(_) => {
            warn!("MQTT thread did not shut down within timeout");
        }
    }
}

fn run_mqtt_client(
    data_sender: Arc<Sender<DataPoint>>, 
    audio_sender: Arc<Sender<AudioData>>,
    shutdown_signal: Arc<AtomicBool>
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok(); // 加载 .env 文件

    let mqtt_user = env::var("MQTT_USER")?;
    let mqtt_pass = env::var("MQTT_PASS")?;
    let mqtt_host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".into());
    let mqtt_port = env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".into())
        .parse::<u16>()?;

    let mut mqtt_options = MqttOptions::new(
        "sensor-client-01",
        mqtt_host,
        mqtt_port
    );

    mqtt_options
        .set_credentials(mqtt_user, mqtt_pass);

    mqtt_options
        .set_keep_alive(Duration::from_secs(5))
        .set_last_will(LastWill::new(
            "sensors",
            "offline",
            QoS::AtLeastOnce,
            false,
        ));

    let (client, mut connection) = Client::new(mqtt_options, 10);
    client.subscribe("sensors", QoS::AtLeastOnce)?;
    client.subscribe("audio", QoS::AtLeastOnce)?;

    for event in connection.iter() {
        // 检查关闭信号
        if shutdown_signal.load(Ordering::Relaxed) {
            info!("MQTT thread received shutdown signal, exiting gracefully");
            break;
        }

        match event {
            Ok(Event::Incoming(Packet::Publish(publish))) if publish.topic == "sensors" => {
                match parse_sensor_data(&publish.payload) {
                    Ok(data) => {
                        if let Err(e) = data_sender.send(data) {
                            // 通道断开表示GUI已关闭，优雅退出
                            info!("Sensor data channel disconnected, MQTT thread exiting");
                            break;
                        }
                    }
                    Err(e) => warn!("Invalid sensor data: {}", e),
                }
            }
            Ok(Event::Incoming(Packet::Publish(publish))) if publish.topic == "audio" => {
                match parse_audio_data(&publish.payload) {
                    Ok(data) => {
                        if let Err(e) = audio_sender.send(data) {
                            // 通道断开表示GUI已关闭，优雅退出
                            info!("Audio data channel disconnected, MQTT thread exiting");
                            break;
                        }
                    }
                    Err(e) => warn!("Invalid audio data: {}", e),
                }
            }
            Ok(Event::Incoming(_)) => {} // 移除非关键日志
            Err(e) => {
                error!("MQTT connection error: {}", e);
                return Err(e.into());
            }
            _ => {}
        }
    }

    Ok(())
}

fn parse_sensor_data(payload: &[u8]) -> Result<DataPoint, String> {
    let payload_str = std::str::from_utf8(payload)
        .map_err(|e| format!("Invalid UTF-8: {}", e))?;

    serde_json::from_str::<DataPoint>(payload_str)
        .map_err(|e| format!("JSON parsing error: {}", e))
}

fn parse_audio_data(payload: &[u8]) -> Result<AudioData, String> {
    let payload_str = std::str::from_utf8(payload)
        .map_err(|e| format!("Invalid UTF-8: {}", e))?;

    serde_json::from_str::<AudioData>(payload_str)
        .map_err(|e| format!("Audio JSON parsing error: {}", e))
}


struct SensorDataApp {
    waveform_plot: plotter::WaveformPlot,
    data_receiver: Receiver<DataPoint>,
    audio_receiver: Receiver<AudioData>,
    is_collecting: bool,
    // 基于timestamp的校准
    is_calibrating: bool,
    calibration_data: Vec<DataPoint>,
    calibration_start_time: Option<std::time::Instant>,
    calculated_sample_rate: Option<f64>,
}

impl SensorDataApp {
    pub fn new(data_receiver: Receiver<DataPoint>, audio_receiver: Receiver<AudioData>) -> Self {
        let mut app = SensorDataApp {
            waveform_plot: plotter::WaveformPlot::new(393), // 初始采样率
            data_receiver,
            audio_receiver,
            is_collecting: false, // 默认不开始采集，先校准
            // 校准相关初始化
            is_calibrating: true, // 启动时自动开始校准
            calibration_data: Vec::new(),
            calibration_start_time: None, // 等待第一个样本到达时开始计时
            calculated_sample_rate: None,
        };
        
        // 打印启动信息
        info!("应用启动，等待数据到达开始校准...");
        
        app
    }
}

impl eframe::App for SensorDataApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 设置明亮模式主题
        ctx.set_visuals(egui::Visuals::light());
        
        // 顶部状态栏
        egui::TopBottomPanel::top("status_bar")
            .min_height(40.0)
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    
                    let (status_text, status_color) = if self.is_calibrating {
                        ("Calibrating", egui::Color32::from_rgb(255, 165, 0)) // 橙色
                    } else if self.is_collecting {
                        ("Collecting", egui::Color32::from_rgb(0, 150, 0)) // 绿色
                    } else {
                        ("Stopped", egui::Color32::from_rgb(150, 0, 0)) // 红色
                    };
                    
                    ui.colored_label(status_color, status_text);
                    
                    ui.separator();
                    
                    // 状态显示
                    if self.is_calibrating {
                        if let Some(start_time) = self.calibration_start_time {
                            let elapsed = start_time.elapsed().as_secs_f64();
                            let progress = (elapsed / 5.0).min(1.0);
                            ui.label(format!("auto calibrating... {:.1}s / 5.0s ({} samples)", 
                                            elapsed, self.calibration_data.len()));
                            
                            // 进度条
                            let progress_bar = egui::ProgressBar::new(progress as f32)
                                .desired_width(150.0);
                            ui.add(progress_bar);
                        } else {
                            ui.label("waiting for data...");
                        }
                    } else if self.is_collecting {
                        ui.label("data collecting...");
                    } else {
                        ui.label("waiting for data...");
                    }
                    
                    ui.separator();
                    
                    // 显示采样率信息
                    if let Some(rate) = self.calculated_sample_rate {
                        ui.label(format!("Sample Rate: {:.1} Hz", rate));
                    } else {
                        ui.label("Sample Rate: Not calibrated");
                    }
                    
                    ui.separator();
                    ui.label("Window: 5.0s");
                });
                ui.add_space(5.0);
            });
        
        // 处理数据：校准、采集或丢弃
        if self.is_calibrating {
            // 校准模式：收集timestamp数据
            while let Ok(data) = self.data_receiver.try_recv() {
                self.process_calibration_data(data);
            }
            
            // 检查是否达到5秒
            if let Some(start_time) = self.calibration_start_time {
                let elapsed = start_time.elapsed();
                if elapsed.as_secs_f64() >= 5.0 && !self.calibration_data.is_empty() {
                    self.calculate_sample_rate_from_timestamps();
                }
            }
            
            // 校准期间丢弃音频数据
            while let Ok(_) = self.audio_receiver.try_recv() {
                // 丢弃音频数据
            }
        } else if self.is_collecting {
            // 正常采集模式
            while let Ok(data) = self.data_receiver.try_recv() {
                info!("ACC data - x: {:.3}, y: {:.3}, z: {:.3}, time: {}", 
                      data.x, data.y, data.z, format_timestamp(data.timestamp));
                self.waveform_plot.add_data(data.x, data.y, data.z);
            }
            // 处理音频数据
            while let Ok(audio_data) = self.audio_receiver.try_recv() {
                info!("Audio data - samples: {}, time: {}", 
                      audio_data.samples, format_timestamp(audio_data.timestamp));
                self.process_audio_data(&audio_data);
            }
        } else {
            // 停止状态：清空接收缓冲区
            while let Ok(_) = self.data_receiver.try_recv() {
                // 丢弃数据
            }
            while let Ok(_) = self.audio_receiver.try_recv() {
                // 丢弃音频数据
            }
        }
        
        // 主要内容区域 - 波形图
        egui::CentralPanel::default().show(ctx, |ui| {
            self.waveform_plot.ui(ui);
        });

        ctx.request_repaint_after(Duration::from_millis(150));
    }
}

impl SensorDataApp {
    fn start_calibration(&mut self) {
        self.is_calibrating = true;
        self.calibration_start_time = Some(std::time::Instant::now());
        self.calibration_data.clear();
        self.calculated_sample_rate = None;
    }
    
    fn process_calibration_data(&mut self, data: DataPoint) {
        // 如果这是第一个样本，开始计时
        if self.calibration_start_time.is_none() {
            self.calibration_start_time = Some(std::time::Instant::now());
            info!("收到第一个样本，开始校准计时");
        }
        
        self.calibration_data.push(data);
    }
    
    fn calculate_sample_rate_from_timestamps(&mut self) {
        if self.calibration_data.len() < 2 {
            self.is_calibrating = false;
            return;
        }
        
        // 排序数据点以确保时间戳顺序正确
        self.calibration_data.sort_by_key(|d| d.timestamp);
        
        let first_timestamp = self.calibration_data.first().unwrap().timestamp;
        let last_timestamp = self.calibration_data.last().unwrap().timestamp;
        let sample_count = self.calibration_data.len();
        
        // 计算时间跨度（毫秒转秒）
        let time_span_ms = last_timestamp - first_timestamp;
        let time_span_s = time_span_ms as f64 / 1000.0;
        
        if time_span_s <= 0.0 {
            self.is_calibrating = false;
            return;
        }
        
        // 计算采样率：(样本数 - 1) / 时间跨度
        let sample_rate = (sample_count - 1) as f64 / time_span_s;
        self.calculated_sample_rate = Some(sample_rate);
        
        info!("校准完成! 采样率: {:.1} Hz，自动开始数据采集", sample_rate);
        
        // 使用校准后的采样率重新创建波形绘制器
        self.waveform_plot = plotter::WaveformPlot::new(sample_rate as usize);
        
        // 结束校准并自动开始采集
        self.is_calibrating = false;
        self.is_collecting = true; // 自动开始采集数据
        self.calibration_start_time = None;
        self.calibration_data.clear();
    }
    
    fn process_audio_data(&mut self, audio_data: &AudioData) {
        // 解码Base64音频数据
        match general_purpose::STANDARD.decode(&audio_data.audio_data) {
            Ok(decoded_bytes) => {
                // 将字节数据转换为i16样本
                let mut samples = Vec::new();
                for chunk in decoded_bytes.chunks_exact(2) {
                    let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                    samples.push(sample);
                }
                
                // 将音频样本添加到波形绘制器
                if !samples.is_empty() {
                    // 直接使用原始音频样本，不进行下采样
                    self.waveform_plot.add_audio_samples(&samples);
                }
            }
            Err(e) => {
                warn!("Failed to decode audio data: {}", e);
            }
        }
    }
}

