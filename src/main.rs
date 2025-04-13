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
use std::thread;
use std::time::Duration;
use std::collections::VecDeque;

#[derive(serde::Deserialize)]
struct DataPoint {
    x: f64,
    y: f64,
    z: f64,
    timestamp: i64,
}

fn main() {
    logger::init_logger();
    info!("Application starting");

    let (data_sender, data_receiver) = bounded(5000);
    let data_sender = Arc::new(data_sender);

    let mqtt_sender = Arc::clone(&data_sender);
    let mqtt_handle = thread::spawn(move || {
        if let Err(e) = run_mqtt_client(mqtt_sender) {
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
        Box::new(|cc| Ok(Box::new(SensorDataApp::new(data_receiver)))),
    ) {
        error!("GUI failed: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = mqtt_handle.join() {
        error!("MQTT thread panicked: {:?}", e);
    }
}

fn run_mqtt_client(sender: Arc<Sender<DataPoint>>) -> Result<(), Box<dyn std::error::Error>> {
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

    for event in connection.iter() {
        match event {
            Ok(Event::Incoming(Packet::Publish(publish))) if publish.topic == "sensors" => {
                match parse_sensor_data(&publish.payload) {
                    Ok(data) => {
                        if let Err(e) = sender.send(data) {
                            warn!("Channel send error: {}", e);
                        }
                    }
                    Err(e) => warn!("Invalid sensor data: {}", e),
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

struct SensorDataApp {
    waveform_plot: plotter::WaveformPlot,
    data_receiver: Receiver<DataPoint>,
}

impl SensorDataApp {
    pub fn new(data_receiver: Receiver<DataPoint>) -> Self {
        SensorDataApp {
            waveform_plot: plotter::WaveformPlot::new(393), // 5秒容量自动计算
            data_receiver,
        }
    }
}

impl eframe::App for SensorDataApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 直接批量处理所有可用数据
        while let Ok(data) = self.data_receiver.try_recv() {
            self.waveform_plot.add_data(data.x, data.y, data.z);
        }

        // 保持界面渲染逻辑
        egui::CentralPanel::default().show(ctx, |ui| {
            self.waveform_plot.ui(ui);
        });

        ctx.request_repaint_after(Duration::from_millis(25));
    }
}

