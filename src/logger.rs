use chrono::Local;
use env_logger::Builder;
use log::Level;
use std::io::Write;

pub fn init_logger() {
    Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let _time = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let level_color = match record.level() {
                Level::Error => "\x1b[31m\x1b[1m", // 红色
                Level::Warn => "\x1b[33m\x1b[1m",  // 黄色
                Level::Info => "\x1b[32m\x1b[1m",  // 绿色
                Level::Debug => "\x1b[36m\x1b[1m", // 青色
                Level::Trace => "\x1b[90m\x1b[1m", // 灰色
            };
            writeln!(
                buf,
                "{}{} {}\x1b[0m [{}:{}] {}",
                _time,
                level_color,

                record.level(),

                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args(),
            )
        })
        .init();
}
