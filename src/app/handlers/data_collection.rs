use base64::{Engine as _, engine::general_purpose};
use crate::app::sensor_app::SensorDataApp;

pub struct DataCollectionHandler;

impl DataCollectionHandler {
    pub fn handle_collection(app: &mut SensorDataApp) {
        // 正常采集模式
        while let Ok(data) = app.state.channels.data_receiver.try_recv() {
            // info!("ACC data - x: {:.3}, y: {:.3}, z: {:.3}, time: {}", 
            //       data.x, data.y, data.z, format_timestamp(data.timestamp));
            app.state.waveform_plot.add_data(data.x, data.y, data.z, data.gx, data.gy, data.gz, data.timestamp);
        }
        
        // 处理音频数据
        while let Ok(audio_data) = app.state.channels.audio_receiver.try_recv() {
            // info!("Audio data - samples: {}, time: {}", 
            //       audio_data.samples, format_timestamp(audio_data.timestamp));
            
            app.state.database.last_audio_metadata = Some(audio_data.clone());
            Self::process_audio_data(app, &audio_data);
        }
    }
    
    fn process_audio_data(app: &mut SensorDataApp, audio_data: &crate::types::AudioData) {
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
                    app.state.waveform_plot.add_audio_samples(&samples, audio_data.timestamp, audio_data.sample_rate);
                }
            }
            Err(e) => {
                log::warn!("Failed to decode audio data: {}", e);
            }
        }
    }
}
