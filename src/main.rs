mod audio;
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
use log::{error, info, warn};

use types::{DataPoint, AudioData, DatabaseTask, SaveResult};
use database::run_database_handler;
use mqtt::run_mqtt_client;
use app::SensorDataApp;
use config::ConfigManager;

fn setup_custom_fonts(ctx: &egui::Context) {
    // 配置字体以支持中文显示
    let mut fonts = egui::FontDefinitions::default();
    
    // 尝试加载系统中文字体
    let mut chinese_font_loaded = false;
    
    // 尝试不同的Windows中文字体路径
    let font_paths = [
        "C:/Windows/Fonts/msyh.ttc",      // 微软雅黑
        "C:/Windows/Fonts/simhei.ttf",    // 黑体  
        "C:/Windows/Fonts/simsun.ttc",    // 宋体
        "C:/Windows/Fonts/simkai.ttf",    // 楷体
    ];
    
    for (index, path) in font_paths.iter().enumerate() {
        if let Ok(font_data) = std::fs::read(path) {
            let font_name = format!("chinese_font_{}", index);
            fonts.font_data.insert(
                font_name.clone(),
                egui::FontData::from_owned(font_data).into(),
            );
            
            // 将中文字体插入到字体族的开头
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, font_name.clone());
                
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, font_name);
                
            chinese_font_loaded = true;
            log::info!("Successfully loaded Chinese font: {}", path);
            break;
        }
    }
    
    if !chinese_font_loaded {
        log::warn!("Could not load system Chinese fonts, Chinese text may not display correctly");
    }

    // 设置字体
    ctx.set_fonts(fonts);
}

fn load_icon() -> Option<egui::IconData> {
    // 尝试加载应用图标
    let icon_path = "src/images/icon.jpg";
    
    match std::fs::read(icon_path) {
        Ok(icon_bytes) => {
            match image::load_from_memory(&icon_bytes) {
                Ok(image) => {
                    // 将图像转换为 RGBA 格式
                    let rgba_image = image.to_rgba8();
                    let (width, height) = rgba_image.dimensions();
                    
                    let icon_data = egui::IconData {
                        rgba: rgba_image.into_raw(),
                        width: width as u32,
                        height: height as u32,
                    };
                    
                    info!("Successfully loaded application icon from {}", icon_path);
                    Some(icon_data)
                },
                Err(e) => {
                    warn!("Failed to decode icon image: {}", e);
                    None
                }
            }
        },
        Err(e) => {
            warn!("Failed to read icon file {}: {}", icon_path, e);
            None
        }
    }
}

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
    let mut viewport_builder = egui::ViewportBuilder::default()
        .with_inner_size([config.window.width, config.window.height])
        .with_resizable(config.window.resizable);
    
    // 设置应用图标
    if let Some(icon) = load_icon() {
        viewport_builder = viewport_builder.with_icon(icon);
    }
    
    // 如果配置了窗口位置，则设置位置
    if let (Some(x), Some(y)) = (config.window.x, config.window.y) {
        viewport_builder = viewport_builder.with_position([x, y]);
    } else if let Some(y) = config.window.y {
        // 如果只设置了y坐标，x坐标居中
        viewport_builder = viewport_builder.with_position([
            (1920.0 - config.window.width) / 2.0,  // 假设屏幕宽度1920，居中显示
            y
        ]);
    }

    let options = eframe::NativeOptions {
        vsync: config.window.vsync,
        hardware_acceleration: if config.window.hardware_acceleration {
            eframe::HardwareAcceleration::Preferred
        } else {
            eframe::HardwareAcceleration::Off
        },
        renderer: eframe::Renderer::Glow,
        viewport: viewport_builder,
        ..Default::default()
    };

    eframe::run_native(
        &config.window.title,
        options,
        Box::new(|cc| {
            // 配置中文字体
            setup_custom_fonts(&cc.egui_ctx);
            
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
