mod logger;
mod plotter;
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
    audio_level: f32,
}

impl SensorDataApp {
    pub fn new(data_receiver: Receiver<DataPoint>, audio_receiver: Receiver<AudioData>) -> Self {
        SensorDataApp {
            waveform_plot: plotter::WaveformPlot::new(393), // 5秒容量自动计算
            data_receiver,
            audio_receiver,
            is_collecting: true, // 默认开始采集
            audio_level: 0.0,
        }
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
                    
                    let status_text = if self.is_collecting { "Collecting" } else { "Stopped" };
                    let status_color = if self.is_collecting { 
                        egui::Color32::from_rgb(0, 150, 0) 
                    } else { 
                        egui::Color32::from_rgb(150, 0, 0) 
                    };
                    
                    ui.colored_label(status_color, status_text);
                    
                    ui.separator();
                    
                    if self.is_collecting {
                        if ui.button("Stop").clicked() {
                            self.is_collecting = false;
                        }
                    } else {
                        if ui.button("Start").clicked() {
                            self.is_collecting = true;
                        }
                    }
                    
                    ui.separator();
                    
                    // 显示一些统计信息
                    ui.label("Sample Rate: 393 Hz");
                    ui.separator();
                    ui.label("Window: 5.0s");
                    ui.separator();
                    ui.label(format!("Audio Level: {:.2}", self.audio_level));
                });
                ui.add_space(5.0);
            });
        
        // 只有在采集状态时才处理数据
        if self.is_collecting {
            while let Ok(data) = self.data_receiver.try_recv() {
                self.waveform_plot.add_data(data.x, data.y, data.z);
            }
            // 处理音频数据
            while let Ok(audio_data) = self.audio_receiver.try_recv() {
                self.process_audio_data(&audio_data);
            }
        } else {
            // 停止采集时，清空接收缓冲区但不添加到图表
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

        ctx.request_repaint_after(Duration::from_millis(15));
    }
}

impl SensorDataApp {
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
                    // 为了避免绘图过于密集，我们可以对样本进行下采样
                    // Android发送的是44.1kHz，我们下采样到1kHz用于显示
                    let downsample_factor = 44; // 44100 / 1000 ≈ 44
                    let downsampled: Vec<i16> = samples
                        .iter()
                        .step_by(downsample_factor)
                        .cloned()
                        .collect();
                    
                    self.waveform_plot.add_audio_samples(&downsampled);
                    
                    // 计算音频级别 (RMS) 用于状态栏显示
                    let sum_squares: f64 = samples.iter()
                        .map(|&sample| (sample as f64).powi(2))
                        .sum();
                    let rms = (sum_squares / samples.len() as f64).sqrt();
                    
                    // 归一化到0-1范围，并应用简单的低通滤波
                    let normalized_level = (rms / 32768.0) as f32;
                    self.audio_level = self.audio_level * 0.8 + normalized_level * 0.2;
                }
            }
            Err(e) => {
                warn!("Failed to decode audio data: {}", e);
            }
        }
    }
}

