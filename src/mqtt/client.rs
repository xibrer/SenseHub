use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering, AtomicU64};
use std::time::Duration;
use crossbeam_channel::Sender;
use dotenv::dotenv;
use log::{info, warn, error, debug};
use rumqttc::{Client, Event, LastWill, MqttOptions, Packet, QoS};

use crate::types::{DataPoint, AudioData};
use crate::config::AppConfig;
use super::audio_buffer::{AudioPacketBuffer, AudioBufferConfig};

pub fn run_mqtt_client(
    data_sender: Arc<Sender<DataPoint>>, 
    audio_sender: Arc<Sender<AudioData>>,
    shutdown_signal: Arc<AtomicBool>,
    config: Option<AppConfig>
) -> Result<(), Box<dyn std::error::Error>> {
    // ID跟踪器，用于检测数据包丢失
    let expected_sensor_id = Arc::new(AtomicU64::new(0));
    let expected_audio_id = Arc::new(AtomicU64::new(0));
    
    // 初始化音频数据包缓冲器
    let audio_buffer_config = config
        .as_ref()
        .map(|c| AudioBufferConfig {
            buffer_window_size: c.mqtt.audio_buffer.buffer_window_size,
            max_wait_time_ms: c.mqtt.audio_buffer.max_wait_time_ms,
        })
        .unwrap_or_default();
    
    let mut audio_buffer = if config.as_ref().map(|c| c.mqtt.audio_buffer.enable_reordering).unwrap_or(true) {
        Some(AudioPacketBuffer::new(audio_buffer_config))
    } else {
        None
    };
    
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
                        // 智能检查传感器数据包ID连续性
                        check_packet_continuity(
                            &expected_sensor_id,
                            data.packet_id,
                            "传感器"
                        );
                        
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
                        let packets_to_send = if let Some(ref mut buffer) = audio_buffer {
                            // 使用缓冲器处理音频数据包
                            buffer.process_packet(data)
                        } else {
                            // 不使用缓冲器，直接处理（保持原有逻辑）
                            check_packet_continuity(
                                &expected_audio_id,
                                data.packet_id,
                                "音频"
                            );
                            vec![data]
                        };
                        
                        // 发送所有处理好的音频数据包
                        for audio_packet in packets_to_send {
                            if let Err(_e) = audio_sender.send(audio_packet) {
                                // 通道断开表示GUI已关闭，优雅退出
                                info!("Audio data channel disconnected, MQTT thread exiting");
                                break;
                            }
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

    // 在退出前打印最终缓冲区状态
    if let Some(ref buffer) = audio_buffer {
        info!("MQTT客户端退出 - 音频缓冲器最终状态: {}", buffer.get_buffer_info());
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

/// 智能检查数据包ID连续性
/// 能够区分真正的丢包和预期的ID跳跃（如校准期间）
fn check_packet_continuity(
    expected_id_counter: &Arc<AtomicU64>,
    received_id: u64,
    data_type: &str
) {
    let expected_id = expected_id_counter.load(Ordering::Relaxed);
    
    // 如果是第一个包或者ID完全匹配，直接更新计数器
    if expected_id == 0 || received_id == expected_id {
        expected_id_counter.store(received_id + 1, Ordering::Relaxed);
        return;
    }
    
    if received_id > expected_id {
        let missing_count = received_id - expected_id;
        
        // 根据丢失包的数量判断情况类型
        match missing_count {
            1..=10 => {
                // 少量丢包，可能是网络问题
                warn!("{}数据包丢失: 期望ID {}, 收到ID {}, 丢失 {} 个包", 
                      data_type, expected_id, received_id, missing_count);
            },
            11..=100 => {
                // 中等数量丢包，可能是网络不稳定
                warn!("{}数据包大量丢失: 期望ID {}, 收到ID {}, 丢失 {} 个包 (网络不稳定?)", 
                      data_type, expected_id, received_id, missing_count);
            },
            _ => {
                // 大量丢包，很可能是校准、重启或重新连接
                info!("{}数据包ID重新同步: 期望ID {}, 收到ID {}, 跳过 {} 个包 (校准/重启/重连)", 
                      data_type, expected_id, received_id, missing_count);
            }
        }
    } else {
        // received_id < expected_id，可能是乱序或重复包
        let id_diff = expected_id - received_id;
        if id_diff <= 10 {
            warn!("{}数据包乱序: 期望ID {}, 收到ID {} (延迟 {} 个包)", 
                  data_type, expected_id, received_id, id_diff);
        } else {
            // 大幅倒退，可能是客户端重启了
            info!("{}数据包ID倒退: 期望ID {}, 收到ID {} (客户端重启?)", 
                  data_type, expected_id, received_id);
        }
    }
    
    // 无论如何都更新计数器到当前收到的ID+1
    expected_id_counter.store(received_id + 1, Ordering::Relaxed);
}
