use std::time::Duration;
use eframe::{egui, Frame};
use log::{info, warn};

use crate::types::{DataPoint, AudioData, DatabaseTask, SaveResult};
use crate::database::generate_session_id;
use crate::config::ConfigManager;
use crate::audio::AudioPlayer;
use super::state::AppState;

pub struct SensorDataApp {
    // 统一的状态管理
    pub state: AppState,

    // 配置管理
    pub config: ConfigManager,

    // 音频播放器
    pub audio_player: Option<AudioPlayer>,
}

impl SensorDataApp {
    pub fn new(
        data_receiver: crossbeam_channel::Receiver<DataPoint>,
        audio_receiver: crossbeam_channel::Receiver<AudioData>,
        db_task_sender: crossbeam_channel::Sender<DatabaseTask>,
        save_result_receiver: crossbeam_channel::Receiver<SaveResult>
    ) -> Self {
        // 创建配置管理器
        let config = ConfigManager::new();

        // 创建应用状态
        let mut state = AppState::new(
            data_receiver,
            audio_receiver,
            db_task_sender,
            save_result_receiver,
            config.get_config(),
        );

        // 初始化会话ID
        state.collection.current_session_id = generate_session_id();

        // 初始化自动保存间隔为窗口长度
        let plot_config = config.get_config();
        state.collection.auto_save_interval_ms = (plot_config.plot.window_duration_seconds * 1000.0) as u64;

        // 初始化音频播放器
        let audio_player = match AudioPlayer::new() {
            Ok(player) => {
                info!("Audio player initialized successfully");
                Some(player)
            }
            Err(e) => {
                warn!("Failed to initialize audio player: {}", e);
                None
            }
        };

        let mut app = SensorDataApp {
            state,
            config,
            audio_player,
        };

        // 加载文本文件
        if let Err(e) = app.state.load_text_file("documents/chinese.txt") {
            warn!("Failed to load text file: {}", e);
        } else {
            info!("Text file loaded successfully");
        }

        // 打印启动信息
        info!("应用启动，等待数据到达开始校准...");

        app
    }
}

impl eframe::App for SensorDataApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 设置明亮模式主题
        ctx.set_visuals(egui::Visuals::light());

        // 渲染UI组件
        crate::app::ui::render_status_bar(self, ctx);
        crate::app::ui::render_bottom_status_bar(self, ctx);
        crate::app::ui::render_history_panel(self, ctx);
        crate::app::ui::render_main_panel(self, ctx);
        crate::app::ui::render_export_dialog(self, ctx);

        // 处理各种结果
        self.handle_save_results();
        self.handle_export_results();
        self.handle_sessions_results();
        self.handle_history_results();

        // 处理数据：校准、采集或丢弃
        self.handle_data_processing();

        // 处理键盘输入
        self.handle_keyboard_input(ctx);

        // 更新音频播放状态
        self.update_audio_playback_state();

        ctx.request_repaint_after(Duration::from_millis(150));
    }
}