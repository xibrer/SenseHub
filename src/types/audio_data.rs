#[derive(serde::Deserialize, Clone, Debug)]
pub struct AudioData {
    pub audio_data: String,  // Base64 encoded audio data
    pub sample_rate: u32,
    pub channels: u8,
    pub format: String,
    pub samples: usize,
    pub timestamp: i64,
}

impl AudioData {
    pub fn new(
        audio_data: String,
        sample_rate: u32,
        channels: u8,
        format: String,
        samples: usize,
        timestamp: i64,
    ) -> Self {
        Self {
            audio_data,
            sample_rate,
            channels,
            format,
            samples,
            timestamp,
        }
    }
}
