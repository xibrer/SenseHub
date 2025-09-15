use rodio::{OutputStreamBuilder, Sink, Source};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::io::Cursor;

/// 音频播放器状态
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// 音频播放器命令
#[derive(Debug, Clone)]
pub enum AudioCommand {
    LoadAudio { data: Vec<f32>, sample_rate: f32 },
    Play,
    Pause,
    Stop,
    Shutdown,
}

/// 音频播放器状态更新
#[derive(Debug, Clone)]
pub struct AudioStatus {
    pub state: PlaybackState,
    pub is_available: bool,
}

/// 自定义音频源，用于播放f32样本数据
struct F32Source {
    data: Vec<f32>,
    position: usize,
    sample_rate: u32,
}

impl F32Source {
    fn new(data: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            data,
            position: 0,
            sample_rate,
        }
    }
}

impl Iterator for F32Source {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position < self.data.len() {
            let sample = self.data[self.position];
            self.position += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl Source for F32Source {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.data.len() - self.position)
    }

    fn channels(&self) -> u16 {
        1 // 单声道
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.data.len() as f32 / self.sample_rate as f32))
    }
}

/// 音频播放器
pub struct AudioPlayer {
    command_sender: mpsc::Sender<AudioCommand>,
    status_receiver: Arc<Mutex<mpsc::Receiver<AudioStatus>>>,
    worker_handle: Option<JoinHandle<()>>,
    current_status: Arc<Mutex<AudioStatus>>,
}

impl AudioPlayer {
    /// 创建新的音频播放器
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (command_sender, command_receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();
        
        let initial_status = AudioStatus {
            state: PlaybackState::Stopped,
            is_available: false,
        };

        let current_status = Arc::new(Mutex::new(initial_status.clone()));
        let worker_status = Arc::clone(&current_status);

        // 启动音频工作线程
        let worker_handle = thread::spawn(move || {
            if let Err(e) = audio_worker_thread(command_receiver, status_sender, worker_status) {
                eprintln!("Audio worker thread error: {}", e);
            }
        });

        Ok(AudioPlayer {
            command_sender,
            status_receiver: Arc::new(Mutex::new(status_receiver)),
            worker_handle: Some(worker_handle),
            current_status,
        })
    }

    /// 加载音频数据（从f64音频样本，16kHz采样率）
    pub fn load_audio_data(&mut self, data: &[f64], original_sample_rate: f32) {
        let audio_data: Vec<f32> = data.iter().map(|&x| x as f32).collect();
        let _ = self.command_sender.send(AudioCommand::LoadAudio { 
            data: audio_data, 
            sample_rate: original_sample_rate 
        });
    }

    /// 开始播放
    pub fn play(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let _ = self.command_sender.send(AudioCommand::Play);
        Ok(())
    }

    /// 暂停播放
    pub fn pause(&mut self) {
        let _ = self.command_sender.send(AudioCommand::Pause);
    }

    /// 停止播放
    pub fn stop(&mut self) {
        let _ = self.command_sender.send(AudioCommand::Stop);
    }

    /// 获取当前播放状态
    pub fn get_state(&self) -> PlaybackState {
        self.current_status.lock().unwrap().state.clone()
    }



    /// 更新状态（从工作线程接收状态更新）
    pub fn update_status(&self) {
        if let Ok(receiver) = self.status_receiver.lock() {
            while let Ok(status) = receiver.try_recv() {
                *self.current_status.lock().unwrap() = status;
            }
        }
    }

    /// 检查播放器是否可用
    pub fn is_available(&self) -> bool {
        self.current_status.lock().unwrap().is_available
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        // 发送关闭命令
        let _ = self.command_sender.send(AudioCommand::Shutdown);
        
        // 等待工作线程结束
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

/// 音频工作线程
fn audio_worker_thread(
    command_receiver: mpsc::Receiver<AudioCommand>,
    status_sender: mpsc::Sender<AudioStatus>,
    current_status: Arc<Mutex<AudioStatus>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 在 rodio 0.21 中，使用简单的方法创建音频输出流
    let _stream = OutputStreamBuilder::open_default_stream()
        .map_err(|e| format!("Failed to open default audio stream: {}", e))?;

    // 音频数据状态
    let audio_data = Arc::new(Mutex::new(Vec::<f32>::new()));
    let sample_rate = Arc::new(Mutex::new(16000.0f32));
    let sink = Arc::new(Mutex::new(Option::<Sink>::None));
    let playback_state = Arc::new(Mutex::new(PlaybackState::Stopped));

    // 发送初始状态
    let _ = status_sender.send(AudioStatus {
        state: PlaybackState::Stopped,
        is_available: false,
    });

    println!("Debug: Audio worker thread started with rodio 0.21");

    // 主循环
    loop {
        match command_receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(AudioCommand::LoadAudio { data, sample_rate: sr }) => {
                println!("Debug: Loading audio data");
                println!("  Sample rate: {}", sr);
                println!("  Data length: {}", data.len());
                println!("  Duration: {:.2}s", data.len() as f32 / sr);

                // 停止当前播放
                if let Some(current_sink) = sink.lock().unwrap().take() {
                    current_sink.stop();
                }
                *playback_state.lock().unwrap() = PlaybackState::Stopped;

                // 存储音频数据
                *audio_data.lock().unwrap() = data.clone();
                *sample_rate.lock().unwrap() = sr;

                // 更新状态
                let status = AudioStatus {
                    state: PlaybackState::Stopped,
                    is_available: true,
                };
                *current_status.lock().unwrap() = status.clone();
                let _ = status_sender.send(status);
            },
            Ok(AudioCommand::Play) => {
                let data = audio_data.lock().unwrap().clone();
                let sr = *sample_rate.lock().unwrap();
                
                if data.is_empty() {
                    continue;
                }

                println!("Debug: Starting playback with rodio 0.21");

                // 在 rodio 0.21 中，使用 Sink::connect_new()
                // 首先需要获取 mixer
                let mixer = _stream.mixer();
                let new_sink = Sink::connect_new(&mixer);
                
                let source = F32Source::new(data, sr as u32);
                new_sink.append(source);
                new_sink.play();
                
                *sink.lock().unwrap() = Some(new_sink);
                *playback_state.lock().unwrap() = PlaybackState::Playing;
                
                println!("Debug: Playback started successfully");
            },
            Ok(AudioCommand::Pause) => {
                if let Some(current_sink) = sink.lock().unwrap().as_ref() {
                    current_sink.pause();
                    *playback_state.lock().unwrap() = PlaybackState::Paused;
                    println!("Debug: Playback paused");
                }
            },
            Ok(AudioCommand::Stop) => {
                if let Some(current_sink) = sink.lock().unwrap().take() {
                    current_sink.stop();
                }
                *playback_state.lock().unwrap() = PlaybackState::Stopped;
                println!("Debug: Playback stopped");
            },
            Ok(AudioCommand::Shutdown) => {
                if let Some(current_sink) = sink.lock().unwrap().take() {
                    current_sink.stop();
                }
                println!("Debug: Audio worker thread shutting down");
                break;
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // 定期发送状态更新
                let data = audio_data.lock().unwrap();
                let state = playback_state.lock().unwrap().clone();
                
                // 检查播放是否完成
                let current_state = if let Some(current_sink) = sink.lock().unwrap().as_ref() {
                    if current_sink.empty() && matches!(state, PlaybackState::Playing) {
                        // 播放完成
                        *playback_state.lock().unwrap() = PlaybackState::Stopped;
                        PlaybackState::Stopped
                    } else {
                        state
                    }
                } else {
                    PlaybackState::Stopped
                };
                
                let status = AudioStatus {
                    state: current_state,
                    is_available: !data.is_empty(),
                };
                
                *current_status.lock().unwrap() = status.clone();
                let _ = status_sender.send(status);
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}