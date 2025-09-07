use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::thread;
use crossbeam_channel::Sender;
use dotenv::dotenv;
use log::{info, warn, error, debug};
use rumqttc::{Client, Event, LastWill, MqttOptions, Packet, QoS, ConnectionError};

use crate::types::{DataPoint, AudioData};

pub fn run_mqtt_client(
    data_sender: Arc<Sender<DataPoint>>, 
    audio_sender: Arc<Sender<AudioData>>,
    shutdown_signal: Arc<AtomicBool>
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok(); // 加载 .env 文件
    
    let mqtt_user = env::var("MQTT_USER").unwrap_or_else(|_| "guest".into());
    let mqtt_pass = env::var("MQTT_PASS").unwrap_or_else(|_| "guest".into());
    let mqtt_host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".into());
    let mqtt_port = env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".into())
        .parse::<u16>()
        .unwrap_or(1883);

    info!("正在连接MQTT服务器: {}:{}", mqtt_host, mqtt_port);
    debug!("MQTT用户名: {}", mqtt_user);

    let max_retries = 5;
    let mut retry_count = 0;

    while retry_count < max_retries && !shutdown_signal.load(Ordering::Relaxed) {
        match attempt_mqtt_connection(
            &mqtt_host,
            mqtt_port,
            &mqtt_user,
            &mqtt_pass,
            data_sender.clone(),
            audio_sender.clone(),
            shutdown_signal.clone(),
        ) {
            Ok(_) => {
                info!("MQTT连接成功关闭");
                return Ok(());
            }
            Err(e) => {
                retry_count += 1;
                error!("MQTT连接尝试 {} 失败: {}", retry_count, e);
                
                if retry_count < max_retries {
                    let delay = std::cmp::min(5 * retry_count, 30); // 最大延迟30秒
                    warn!("将在{}秒后重试连接...", delay);
                    thread::sleep(Duration::from_secs(delay as u64));
                } else {
                    error!("达到最大重试次数，MQTT客户端停止");
                    return Err(format!("MQTT连接失败，已重试{}次", max_retries).into());
                }
            }
        }
    }

    if shutdown_signal.load(Ordering::Relaxed) {
        info!("收到关闭信号，MQTT客户端退出");
    }

    Ok(())
}

fn attempt_mqtt_connection(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    data_sender: Arc<Sender<DataPoint>>,
    audio_sender: Arc<Sender<AudioData>>,
    shutdown_signal: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mqtt_options = MqttOptions::new(
        "sensor-client-01",
        host,
        port
    );

    mqtt_options
        .set_credentials(user, pass)
        .set_keep_alive(Duration::from_secs(30))  // 使用更长的keep alive
        .set_last_will(LastWill::new(
            "sensors/status",
            "offline",
            QoS::AtLeastOnce,
            false,
        ));

    debug!("创建MQTT客户端连接...");
    let (client, mut connection) = Client::new(mqtt_options, 10);
    
    // 订阅主题
    client.subscribe("sensors", QoS::AtLeastOnce)?;
    client.subscribe("audio", QoS::AtLeastOnce)?;
    info!("已订阅MQTT主题: sensors, audio");

    let mut connected = false;
    let mut ping_failures = 0;
    const MAX_PING_FAILURES: i32 = 3;

    for event in connection.iter() {
        // 检查关闭信号
        if shutdown_signal.load(Ordering::Relaxed) {
            info!("MQTT线程收到关闭信号，正在断开连接");
            break;
        }

        match event {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                connected = true;
                ping_failures = 0;
                info!("MQTT连接建立成功");
            }
            Ok(Event::Incoming(Packet::PingResp)) => {
                ping_failures = 0;
                debug!("收到MQTT ping响应");
            }
            Ok(Event::Incoming(Packet::Publish(publish))) if publish.topic == "sensors" => {
                match parse_sensor_data(&publish.payload) {
                    Ok(data) => {
                        debug!("收到传感器数据: x={}, y={}, z={}", data.x, data.y, data.z);
                        if let Err(_e) = data_sender.send(data) {
                            info!("传感器数据通道已断开，MQTT线程退出");
                            break;
                        }
                    }
                    Err(e) => warn!("无效的传感器数据: {}", e),
                }
            }
            Ok(Event::Incoming(Packet::Publish(publish))) if publish.topic == "audio" => {
                match parse_audio_data(&publish.payload) {
                    Ok(data) => {
                        debug!("收到音频数据: {} 字节", data.audio_data.len());
                        if let Err(_e) = audio_sender.send(data) {
                            info!("音频数据通道已断开，MQTT线程退出");
                            break;
                        }
                    }
                    Err(e) => warn!("无效的音频数据: {}", e),
                }
            }
            Ok(Event::Incoming(_)) => {
                // 其他消息类型的调试信息
                debug!("收到其他MQTT消息");
            }
            Ok(Event::Outgoing(_)) => {
                // 发出的消息
                debug!("发送MQTT消息");
            }
            Err(ConnectionError::MqttState(rumqttc::StateError::AwaitPingResp)) => {
                ping_failures += 1;
                warn!("MQTT ping超时 ({}/{})", ping_failures, MAX_PING_FAILURES);
                
                if ping_failures >= MAX_PING_FAILURES {
                    error!("连续{}次ping失败，重新连接", MAX_PING_FAILURES);
                    return Err("MQTT ping连续失败".into());
                }
            }
            Err(e) => {
                error!("MQTT连接错误: {}", e);
                if connected {
                    warn!("连接断开，将尝试重连");
                }
                return Err(e.into());
            }
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
