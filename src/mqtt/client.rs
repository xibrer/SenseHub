use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use crossbeam_channel::Sender;
use dotenv::dotenv;
use log::{info, warn, error};
use rumqttc::{Client, Event, LastWill, MqttOptions, Packet, QoS};

use crate::types::{DataPoint, AudioData};

pub fn run_mqtt_client(
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
                        if let Err(_e) = data_sender.send(data) {
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
                        if let Err(_e) = audio_sender.send(data) {
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
