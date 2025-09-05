use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use log::{debug, warn, info};
use crate::types::AudioData;

/// 音频数据包缓冲器配置
#[derive(Debug, Clone)]
pub struct AudioBufferConfig {
    /// 缓冲窗口大小（包数量）
    pub buffer_window_size: usize,
    /// 最大等待时间（毫秒）
    pub max_wait_time_ms: u64,
}

impl Default for AudioBufferConfig {
    fn default() -> Self {
        Self {
            buffer_window_size: 10,
            max_wait_time_ms: 100,
        }
    }
}

/// 缓冲的音频数据包
#[derive(Debug, Clone)]
struct BufferedPacket {
    data: AudioData,
    received_at: Instant,
}

/// 音频数据包缓冲器
/// 用于处理乱序的音频数据包，提供有序的数据流输出
pub struct AudioPacketBuffer {
    config: AudioBufferConfig,
    buffer: BTreeMap<u64, BufferedPacket>,
    next_expected_id: u64,
    last_cleanup: Instant,
}

impl AudioPacketBuffer {
    /// 创建新的音频数据包缓冲器
    pub fn new(config: AudioBufferConfig) -> Self {
        Self {
            config,
            buffer: BTreeMap::new(),
            next_expected_id: 0,
            last_cleanup: Instant::now(),
        }
    }

    /// 处理接收到的音频数据包
    /// 返回可以立即输出的有序数据包列表
    pub fn process_packet(&mut self, audio_data: AudioData) -> Vec<AudioData> {
        let packet_id = audio_data.packet_id;
        let now = Instant::now();
        
        // 初始化期望ID（第一个包）
        if self.next_expected_id == 0 {
            self.next_expected_id = packet_id;
        }
        
        // 检查是否是重复包
        if self.buffer.contains_key(&packet_id) || packet_id < self.next_expected_id {
            debug!("收到重复或过期的音频数据包: ID {}", packet_id);
            return Vec::new();
        }
        
        let mut output_packets = Vec::new();
        
        // 如果是期望的下一个包，直接输出
        if packet_id == self.next_expected_id {
            output_packets.push(audio_data);
            self.next_expected_id += 1;
            
            // 检查缓冲区中是否有连续的包可以输出
            output_packets.extend(self.flush_consecutive_packets());
        } else {
            // 包乱序，放入缓冲区
            let buffered_packet = BufferedPacket {
                data: audio_data,
                received_at: now,
            };
            
            self.buffer.insert(packet_id, buffered_packet);
            debug!("音频数据包乱序，已缓冲: ID {}, 期望 ID {}, 缓冲区大小: {}", 
                   packet_id, self.next_expected_id, self.buffer.len());
        }
        
        // 定期清理超时的包
        if now.duration_since(self.last_cleanup) > Duration::from_millis(self.config.max_wait_time_ms) {
            output_packets.extend(self.cleanup_expired_packets());
            self.last_cleanup = now;
        }
        
        // 检查缓冲区大小限制
        if self.buffer.len() > self.config.buffer_window_size {
            output_packets.extend(self.force_flush_oldest_packets());
        }
        
        output_packets
    }
    
    /// 刷新缓冲区中连续的数据包
    fn flush_consecutive_packets(&mut self) -> Vec<AudioData> {
        let mut output_packets = Vec::new();
        let start_id = self.next_expected_id;
        
        while let Some(buffered_packet) = self.buffer.remove(&self.next_expected_id) {
            output_packets.push(buffered_packet.data);
            self.next_expected_id += 1;
        }
        
        if !output_packets.is_empty() {
            info!("🔄 音频数据包重排序: 从缓冲区输出 {} 个连续包 (ID: {}-{})", 
                  output_packets.len(), start_id, self.next_expected_id - 1);
        }
        
        output_packets
    }
    
    /// 清理过期的数据包
    fn cleanup_expired_packets(&mut self) -> Vec<AudioData> {
        let now = Instant::now();
        let timeout_duration = Duration::from_millis(self.config.max_wait_time_ms);
        let mut expired_packets = Vec::new();
        let mut expired_ids = Vec::new();
        
        for (&packet_id, buffered_packet) in &self.buffer {
            if now.duration_since(buffered_packet.received_at) > timeout_duration {
                expired_ids.push(packet_id);
                expired_packets.push(buffered_packet.data.clone());
            }
        }
        
        for id in &expired_ids {
            self.buffer.remove(id);
        }
        
        if !expired_packets.is_empty() {
            // 对过期的包按ID排序后输出
            expired_packets.sort_by_key(|packet| packet.packet_id);
            
            warn!("⏰ 音频数据包超时丢弃: {} 个包 (ID: {:?})", expired_ids.len(), expired_ids);
            
            // 更新期望ID到最大的过期包ID+1（如果更大的话）
            if let Some(max_expired_id) = expired_packets.iter().map(|p| p.packet_id).max() {
                if max_expired_id >= self.next_expected_id {
                    self.next_expected_id = max_expired_id + 1;
                    info!("📈 由于超时包，更新期望音频包ID到: {}", self.next_expected_id);
                }
            }
        }
        
        expired_packets
    }
    
    /// 强制刷新最旧的数据包（当缓冲区满时）
    fn force_flush_oldest_packets(&mut self) -> Vec<AudioData> {
        let mut flushed_packets = Vec::new();
        let mut flushed_ids = Vec::new();
        
        while self.buffer.len() > self.config.buffer_window_size {
            if let Some((oldest_id, buffered_packet)) = self.buffer.pop_first() {
                flushed_packets.push(buffered_packet.data);
                flushed_ids.push(oldest_id);
                
                // 如果强制输出的包ID大于期望ID，更新期望ID
                if oldest_id >= self.next_expected_id {
                    self.next_expected_id = oldest_id + 1;
                }
            }
        }
        
        if !flushed_packets.is_empty() {
            warn!("🚨 缓冲区已满，强制输出音频数据包: {} 个包 (ID: {:?})", 
                  flushed_packets.len(), flushed_ids);
            
            // 对强制输出的包排序
            flushed_packets.sort_by_key(|packet| packet.packet_id);
        }
        
        flushed_packets
    }
    
    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.buffer.clear();
        info!("音频数据包缓冲区已清空");
    }
    
    /// 获取缓冲区状态信息
    pub fn get_buffer_info(&self) -> String {
        format!(
            "缓冲区状态: 大小={}/{}, 期望ID={}",
            self.buffer.len(),
            self.config.buffer_window_size,
            self.next_expected_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AudioData;
    
    fn create_test_audio_data(packet_id: u64) -> AudioData {
        AudioData::new(
            packet_id,
            "test_audio_data".to_string(),
            44100,
            2,
            "PCM".to_string(),
            1024,
            chrono::Utc::now().timestamp(),
        )
    }
    
    #[test]
    fn test_in_order_packets() {
        let mut buffer = AudioPacketBuffer::new(AudioBufferConfig::default());
        
        // 按顺序发送包
        let result1 = buffer.process_packet(create_test_audio_data(1));
        assert_eq!(result1.len(), 1);
        assert_eq!(result1[0].packet_id, 1);
        
        let result2 = buffer.process_packet(create_test_audio_data(2));
        assert_eq!(result2.len(), 1);
        assert_eq!(result2[0].packet_id, 2);
    }
    
    #[test]
    fn test_out_of_order_packets() {
        let mut buffer = AudioPacketBuffer::new(AudioBufferConfig::default());
        
        // 先发送包1
        let result1 = buffer.process_packet(create_test_audio_data(1));
        assert_eq!(result1.len(), 1);
        
        // 发送包3（跳过包2）
        let result3 = buffer.process_packet(create_test_audio_data(3));
        assert_eq!(result3.len(), 0); // 应该被缓冲
        
        // 发送包2，应该同时输出包2和包3
        let result2 = buffer.process_packet(create_test_audio_data(2));
        assert_eq!(result2.len(), 2);
        assert_eq!(result2[0].packet_id, 2);
        assert_eq!(result2[1].packet_id, 3);
    }
    
    #[test]
    fn test_duplicate_packets() {
        let mut buffer = AudioPacketBuffer::new(AudioBufferConfig::default());
        
        // 发送包1
        let result1 = buffer.process_packet(create_test_audio_data(1));
        assert_eq!(result1.len(), 1);
        
        // 重复发送包1
        let result_dup = buffer.process_packet(create_test_audio_data(1));
        assert_eq!(result_dup.len(), 0);
    }
}
