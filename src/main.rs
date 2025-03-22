mod logger;
mod plotter;

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
        "Sensor Waveform Viewer",
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
    let mut mqtt_options = MqttOptions::new(
        "sensor-client-01",
        "10.90.129.114",
        1883
    );
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
    data_buffer: VecDeque<DataPoint>,
    pending_data: Vec<DataPoint>, // 新增临时缓冲区
}

impl SensorDataApp {
    pub fn new(data_receiver: Receiver<DataPoint>) -> Self {
        SensorDataApp {
            waveform_plot: plotter::WaveformPlot::new(393),
            data_receiver,
            data_buffer: VecDeque::new(),
            pending_data: Vec::new(), // 初始化缓冲区
        }
    }
}

impl eframe::App for SensorDataApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        let mut count = 0;
        // 接收数据到临时缓冲区
        loop {
            if count >= 20 {
                break;
            }

            match self.data_receiver.try_recv() {
                Ok(data) => {
                    self.pending_data.push(data);
                    count += 1;
                }
                Err(_) => break,
            }
        }

        // 每次处理五帧数据
        while self.pending_data.len() >= 5 {
            let batch = self.pending_data.drain(..5).collect::<Vec<_>>();
            for data in batch {
                self.waveform_plot.add_data(data.x, data.y, data.z);
                let timestamp = data.timestamp;

                // 更新数据缓冲区
                self.data_buffer.push_back(data);
                let cutoff = timestamp - 3000;
                while let Some(front) = self.data_buffer.front() {
                    if front.timestamp < cutoff {
                        self.data_buffer.pop_front();
                    } else {
                        break;
                    }
                }
            }
        }
        // ctx.request_repaint_after(Duration::from_millis(25));
        // ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            self.waveform_plot.ui(ui);
        });
    }
}

