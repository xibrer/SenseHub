mod logger;
mod plotter;
mod utils;
mod types;
mod database;
mod mqtt;
mod app;
mod config;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use crossbeam_channel::bounded;
use eframe::egui;
use log::{error, info};

use types::{DataPoint, AudioData, DatabaseTask, SaveResult};
use database::run_database_handler;
use mqtt::run_mqtt_client;
use app::SensorDataApp;
use config::ConfigManager;

fn main() {
    // 初始化日志系统
    logger::init_logger();
    info!("SenseHub application starting");

    // 加载配置
    let config_manager = ConfigManager::new();
    let config = config_manager.get_config();

    // 创建应用通道
    let (data_sender, data_receiver) = bounded::<DataPoint>(config.channels.data_channel_capacity);
    let (audio_sender, audio_receiver) = bounded::<AudioData>(config.channels.audio_channel_capacity);
    let (db_task_sender, db_task_receiver) = bounded::<DatabaseTask>(config.channels.db_task_channel_capacity);
    let (save_result_sender, save_result_receiver) = bounded::<SaveResult>(config.channels.save_result_channel_capacity);

    // 创建共享的关闭信号
    let shutdown_signal = Arc::new(AtomicBool::new(false));

    // 启动后台线程
    let handles = start_background_threads(
        data_sender,
        audio_sender,
        db_task_receiver,
        save_result_sender,
        shutdown_signal.clone(),
    );

    // 配置并启动GUI
    let gui_result = run_gui_application(
        data_receiver,
        audio_receiver,
        db_task_sender,
        save_result_receiver,
        config,
    );

    // GUI关闭后的清理工作
    shutdown_and_cleanup(shutdown_signal, handles, gui_result);
}

fn start_background_threads(
    data_sender: crossbeam_channel::Sender<DataPoint>,
    audio_sender: crossbeam_channel::Sender<AudioData>,
    db_task_receiver: crossbeam_channel::Receiver<DatabaseTask>,
    save_result_sender: crossbeam_channel::Sender<SaveResult>,
    shutdown_signal: Arc<AtomicBool>,
) -> Vec<thread::JoinHandle<()>> {
    let mut handles = Vec::new();

    // 启动MQTT客户端线程
    let mqtt_data_sender = Arc::new(data_sender);
    let mqtt_audio_sender = Arc::new(audio_sender);
    let mqtt_shutdown = Arc::clone(&shutdown_signal);
    
    let mqtt_handle = thread::spawn(move || {
        if let Err(e) = run_mqtt_client(mqtt_data_sender, mqtt_audio_sender, mqtt_shutdown) {
            error!("MQTT thread failed: {}", e);
        }
    });
    handles.push(mqtt_handle);

    // 启动数据库处理线程
    let db_shutdown = Arc::clone(&shutdown_signal);
    let db_handle = thread::spawn(move || {
        if let Err(e) = run_database_handler(db_task_receiver, save_result_sender, db_shutdown) {
            error!("Database handler thread failed: {}", e);
        }
    });
    handles.push(db_handle);

    info!("Background threads started successfully");
    handles
}

fn run_gui_application(
    data_receiver: crossbeam_channel::Receiver<DataPoint>,
    audio_receiver: crossbeam_channel::Receiver<AudioData>,
    db_task_sender: crossbeam_channel::Sender<DatabaseTask>,
    save_result_receiver: crossbeam_channel::Receiver<SaveResult>,
    config: &config::AppConfig,
) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        vsync: config.window.vsync,
        hardware_acceleration: if config.window.hardware_acceleration {
            eframe::HardwareAcceleration::Preferred
        } else {
            eframe::HardwareAcceleration::Off
        },
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([config.window.width, config.window.height])
            .with_resizable(config.window.resizable),
        ..Default::default()
    };

    eframe::run_native(
        &config.window.title,
        options,
        Box::new(|_cc| {
            Ok(Box::new(SensorDataApp::new(
                data_receiver,
                audio_receiver,
                db_task_sender,
                save_result_receiver,
            )))
        }),
    )
}

fn shutdown_and_cleanup(
    shutdown_signal: Arc<AtomicBool>,
    handles: Vec<thread::JoinHandle<()>>,
    gui_result: Result<(), eframe::Error>,
) {
    // 检查GUI结果
    if let Err(e) = gui_result {
        error!("GUI failed: {}", e);
    }

    // 发送关闭信号给所有后台线程
    info!("GUI closed, signaling all threads to shutdown");
    shutdown_signal.store(true, Ordering::Relaxed);

    // 等待所有线程优雅退出
    for (index, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(()) => info!("Background thread {} shut down gracefully", index),
            Err(e) => error!("Background thread {} panicked: {:?}", index, e),
        }
    }

    info!("SenseHub application shutdown complete");
}
