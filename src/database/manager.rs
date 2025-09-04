use duckdb::{Connection, Result as DuckResult};
use std::fs;
use log::{info, error, warn};
use crate::{DataPoint, AudioData};
use chrono::Utc;

pub struct DatabaseManager {
    conn: Connection,
}

impl DatabaseManager {
    pub fn new() -> DuckResult<Self> {
        // 确保data目录存在
        if let Err(e) = fs::create_dir_all("data") {
            error!("Failed to create data directory: {}", e);
        }

        let db_path = "data/sensor_data.db";
        let conn = Connection::open(db_path)?;
        
        info!("Database connection established at: {}", db_path);
        
        let manager = DatabaseManager { conn };
        manager.create_tables()?;
        
        Ok(manager)
    }

    fn create_tables(&self) -> DuckResult<()> {
        // 创建加速度数据表
        self.conn.execute(
            "CREATE SEQUENCE IF NOT EXISTS accelerometer_data_seq",
            [],
        )?;
        
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS accelerometer_data (
                id INTEGER PRIMARY KEY DEFAULT nextval('accelerometer_data_seq'),
                timestamp_ms BIGINT,
                x DOUBLE,
                y DOUBLE,
                z DOUBLE,
                session_id VARCHAR,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // 创建音频数据表
        self.conn.execute(
            "CREATE SEQUENCE IF NOT EXISTS audio_data_seq",
            [],
        )?;
        
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS audio_data (
                id INTEGER PRIMARY KEY DEFAULT nextval('audio_data_seq'),
                timestamp_ms BIGINT,
                sample_rate INTEGER,
                channels TINYINT,
                format VARCHAR,
                samples_count INTEGER,
                audio_blob BLOB,
                session_id VARCHAR,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        info!("Database tables created successfully");
        Ok(())
    }

    pub fn save_accelerometer_data(&self, data: &[DataPoint], session_id: &str) -> DuckResult<usize> {
        if data.is_empty() {
            warn!("No accelerometer data to save");
            return Ok(0);
        }

        let mut stmt = self.conn.prepare(
            "INSERT INTO accelerometer_data (timestamp_ms, x, y, z, session_id) 
             VALUES (?, ?, ?, ?, ?)"
        )?;

        let mut count = 0;
        for point in data {
            // 直接保存Unix毫秒时间戳
            stmt.execute(duckdb::params![
                point.timestamp,
                point.x,
                point.y,
                point.z,
                session_id
            ])?;
            count += 1;
        }

        info!("Saved {} accelerometer data points to database", count);
        Ok(count)
    }

    pub fn save_audio_data(&self, audio_samples: &[f64], audio_metadata: Option<&AudioData>, session_id: &str) -> DuckResult<usize> {
        if audio_samples.is_empty() {
            warn!("No audio data to save");
            return Ok(0);
        }

        // 将f64音频样本转换为i16字节数组
        let mut audio_bytes = Vec::with_capacity(audio_samples.len() * 2);
        for &sample in audio_samples {
            let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            audio_bytes.extend_from_slice(&sample_i16.to_le_bytes());
        }

        let (timestamp_ms, sample_rate, channels, format) = if let Some(metadata) = audio_metadata {
            (
                metadata.timestamp,
                metadata.sample_rate as i32,
                metadata.channels as i32,
                metadata.format.clone()
            )
        } else {
            (
                Utc::now().timestamp_millis(),
                16000, // 默认采样率
                1,     // 默认单声道
                "PCM_16".to_string()
            )
        };

        let mut stmt = self.conn.prepare(
            "INSERT INTO audio_data (timestamp_ms, sample_rate, channels, format, samples_count, audio_blob, session_id) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )?;
        
        stmt.execute(duckdb::params![
            timestamp_ms,
            sample_rate,
            channels,
            format,
            audio_samples.len() as i32,
            audio_bytes,
            session_id
        ])?;

        info!("Saved audio data with {} samples to database", audio_samples.len());
        Ok(1)
    }

    pub fn get_stats(&self) -> DuckResult<(usize, usize)> {
        let acc_count: usize = self.conn
            .query_row("SELECT COUNT(*) FROM accelerometer_data", [], |row| {
                Ok(row.get::<_, i64>(0)? as usize)
            })?;

        let audio_count: usize = self.conn
            .query_row("SELECT COUNT(*) FROM audio_data", [], |row| {
                Ok(row.get::<_, i64>(0)? as usize)
            })?;

        Ok((acc_count, audio_count))
    }

    // 获取所有session ID列表
    pub fn get_all_sessions(&self) -> DuckResult<Vec<String>> {
        let mut sessions = Vec::new();
        
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT session_id FROM accelerometer_data 
             UNION 
             SELECT DISTINCT session_id FROM audio_data 
             ORDER BY session_id DESC"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok(row.get::<_, String>(0)?)
        })?;
        
        for row in rows {
            sessions.push(row?);
        }
        
        Ok(sessions)
    }

    // 获取指定session的加速度数据
    pub fn get_accelerometer_data_by_session(&self, session_id: &str) -> DuckResult<Vec<DataPoint>> {
        let mut data = Vec::new();
        
        let mut stmt = self.conn.prepare(
            "SELECT timestamp_ms, x, y, z FROM accelerometer_data 
             WHERE session_id = ? 
             ORDER BY timestamp_ms"
        )?;
        
        let rows = stmt.query_map([session_id], |row| {
            Ok(DataPoint {
                timestamp: row.get::<_, i64>(0)?,
                x: row.get::<_, f64>(1)?,
                y: row.get::<_, f64>(2)?,
                z: row.get::<_, f64>(3)?,
            })
        })?;
        
        for row in rows {
            data.push(row?);
        }
        
        Ok(data)
    }

    // 获取指定session的音频数据
    pub fn get_audio_data_by_session(&self, session_id: &str) -> DuckResult<Vec<(i64, Vec<f64>, u32, u8, String)>> {
        let mut data = Vec::new();
        
        let mut stmt = self.conn.prepare(
            "SELECT timestamp_ms, audio_blob, sample_rate, channels, format FROM audio_data 
             WHERE session_id = ? 
             ORDER BY timestamp_ms"
        )?;
        
        let rows = stmt.query_map([session_id], |row| {
            let timestamp: i64 = row.get(0)?;
            let audio_blob: Vec<u8> = row.get(1)?;
            let sample_rate: i32 = row.get(2)?;
            let channels: i32 = row.get(3)?;
            let format: String = row.get(4)?;
            
            // 将音频字节数据转换回f64样本
            let mut samples = Vec::new();
            for chunk in audio_blob.chunks_exact(2) {
                let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
                let sample_f64 = sample_i16 as f64 / 32767.0;
                samples.push(sample_f64);
            }
            
            Ok((timestamp, samples, sample_rate as u32, channels as u8, format))
        })?;
        
        for row in rows {
            data.push(row?);
        }
        
        Ok(data)
    }

    // 检查session是否已经导出过
    pub fn is_session_exported(&self, session_id: &str) -> DuckResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM exported_sessions WHERE session_id = ?",
            [session_id],
            |row| row.get(0)
        ).unwrap_or(0);
        
        Ok(count > 0)
    }

    // 标记session为已导出
    pub fn mark_session_exported(&self, session_id: &str) -> DuckResult<()> {
        // 首先创建导出记录表（如果不存在）
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS exported_sessions (
                session_id VARCHAR PRIMARY KEY,
                exported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        
        self.conn.execute(
            "INSERT OR IGNORE INTO exported_sessions (session_id) VALUES (?)",
            [session_id],
        )?;
        
        Ok(())
    }
}

pub fn generate_session_id() -> String {
    use chrono::Utc;
    format!("session_{}", Utc::now().format("%Y%m%d_%H%M%S"))
}
