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
        // 首先创建所有基础表，然后再执行迁移
        
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
                start_timestamp_ms BIGINT,
                end_timestamp_ms BIGINT,
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

        info!("Basic database tables created successfully");

        // 现在执行迁移，添加缺失的列
        self.migrate_accelerometer_table()?;
        self.migrate_username_columns()?;
        self.migrate_scenario_column()?;

        info!("Database migration completed successfully");
        Ok(())
    }

    fn migrate_accelerometer_table(&self) -> DuckResult<()> {
        // 检查是否需要添加陀螺仪列
        let has_gyro_columns = self.check_gyro_columns_exist()?;
        
        if !has_gyro_columns {
            info!("Adding gyroscope columns to accelerometer_data table");
            
            // 添加陀螺仪列
            self.conn.execute("ALTER TABLE accelerometer_data ADD COLUMN gx DOUBLE DEFAULT 0.0", [])?;
            self.conn.execute("ALTER TABLE accelerometer_data ADD COLUMN gy DOUBLE DEFAULT 0.0", [])?;
            self.conn.execute("ALTER TABLE accelerometer_data ADD COLUMN gz DOUBLE DEFAULT 0.0", [])?;
            
            info!("Successfully added gyroscope columns");
        } else {
            info!("Gyroscope columns already exist in accelerometer_data table");
        }
        
        Ok(())
    }

    fn check_gyro_columns_exist(&self) -> DuckResult<bool> {
        // 尝试查询陀螺仪列，如果出错说明列不存在
        let result = self.conn.execute("SELECT gx, gy, gz FROM accelerometer_data LIMIT 1", []);
        
        match result {
            Ok(_) => {
                info!("Gyroscope columns found in database");
                Ok(true)
            },
            Err(_) => {
                info!("Gyroscope columns not found in database");
                Ok(false)
            }
        }
    }

    fn migrate_username_columns(&self) -> DuckResult<()> {
        // 检查加速度数据表是否需要添加用户名列
        let acc_has_username = self.check_username_column_exists("accelerometer_data")?;
        
        if !acc_has_username {
            info!("Adding username column to accelerometer_data table");
            self.conn.execute("ALTER TABLE accelerometer_data ADD COLUMN username VARCHAR DEFAULT ''", [])?;
            info!("Successfully added username column to accelerometer_data table");
        } else {
            info!("Username column already exists in accelerometer_data table");
        }

        // 检查音频数据表是否需要添加用户名列
        let audio_has_username = self.check_username_column_exists("audio_data")?;
        
        if !audio_has_username {
            info!("Adding username column to audio_data table");
            self.conn.execute("ALTER TABLE audio_data ADD COLUMN username VARCHAR DEFAULT ''", [])?;
            info!("Successfully added username column to audio_data table");
        } else {
            info!("Username column already exists in audio_data table");
        }
        
        Ok(())
    }

    fn check_username_column_exists(&self, table_name: &str) -> DuckResult<bool> {
        // 首先检查表是否存在
        let table_exists_query = format!(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = '{}'", 
            table_name
        );
        
        let table_exists = match self.conn.query_row(&table_exists_query, [], |row| {
            Ok(row.get::<_, i64>(0)? > 0)
        }) {
            Ok(exists) => exists,
            Err(_) => {
                // 如果无法查询信息架构，尝试直接查询表
                let test_query = format!("SELECT COUNT(*) FROM {} LIMIT 1", table_name);
                self.conn.execute(&test_query, []).is_ok()
            }
        };
        
        if !table_exists {
            info!("Table {} does not exist, username column does not exist", table_name);
            return Ok(false);
        }
        
        // 表存在，检查username列是否存在
        let query = format!("SELECT username FROM {} LIMIT 1", table_name);
        let result = self.conn.execute(&query, []);
        
        match result {
            Ok(_) => {
                info!("Username column found in {} table", table_name);
                Ok(true)
            },
            Err(_) => {
                info!("Username column not found in {} table", table_name);
                Ok(false)
            }
        }
    }

    fn migrate_scenario_column(&self) -> DuckResult<()> {
        // 检查加速度数据表是否需要添加场景列
        let has_scenario = self.check_scenario_column_exists()?;
        
        if !has_scenario {
            info!("Adding scenario column to accelerometer_data table");
            self.conn.execute("ALTER TABLE accelerometer_data ADD COLUMN scenario VARCHAR DEFAULT 'standard'", [])?;
            info!("Successfully added scenario column to accelerometer_data table");
        } else {
            info!("Scenario column already exists in accelerometer_data table");
        }
        
        Ok(())
    }

    fn check_scenario_column_exists(&self) -> DuckResult<bool> {
        // 尝试查询场景列，如果出错说明列不存在
        let result = self.conn.execute("SELECT scenario FROM accelerometer_data LIMIT 1", []);
        
        match result {
            Ok(_) => {
                info!("Scenario column found in accelerometer_data table");
                Ok(true)
            },
            Err(_) => {
                info!("Scenario column not found in accelerometer_data table");
                Ok(false)
            }
        }
    }

    pub fn save_accelerometer_data(&self, data: &[DataPoint], session_id: &str, username: &str, scenario: &str) -> DuckResult<usize> {
        if data.is_empty() {
            warn!("No accelerometer data to save");
            return Ok(0);
        }

        let mut stmt = self.conn.prepare(
            "INSERT INTO accelerometer_data (timestamp_ms, x, y, z, gx, gy, gz, session_id, username, scenario) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )?;

        let mut count = 0;
        for point in data {
            // 直接保存Unix毫秒时间戳
            stmt.execute(duckdb::params![
                point.timestamp,
                point.x,
                point.y,
                point.z,
                point.gx,
                point.gy,
                point.gz,
                session_id,
                username,
                scenario
            ])?;
            count += 1;
        }

        info!("Saved {} accelerometer data points to database for user {} in scenario {}", count, username, scenario);
        Ok(count)
    }

    pub fn save_audio_data(&self, audio_samples: &[f64], audio_metadata: Option<&AudioData>, session_id: &str, start_timestamp_ms: Option<i64>, end_timestamp_ms: Option<i64>, username: &str) -> DuckResult<usize> {
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

        let (default_timestamp_ms, sample_rate, channels, format) = if let Some(metadata) = audio_metadata {
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

        // 使用提供的开始和结束时间戳，如果没有提供则使用默认时间戳
        let start_timestamp = start_timestamp_ms.unwrap_or(default_timestamp_ms);
        let end_timestamp = end_timestamp_ms.unwrap_or(default_timestamp_ms);

        let mut stmt = self.conn.prepare(
            "INSERT INTO audio_data (start_timestamp_ms, end_timestamp_ms, sample_rate, channels, format, samples_count, audio_blob, session_id, username) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )?;
        
        stmt.execute(duckdb::params![
            start_timestamp,
            end_timestamp,
            sample_rate,
            channels,
            format,
            audio_samples.len() as i32,
            audio_bytes,
            session_id,
            username
        ])?;

        info!("Saved audio data with {} samples to database for user {}", audio_samples.len(), username);
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

    // 获取所有session及其导出状态（优化版本）
    pub fn get_all_sessions_with_export_status(&self) -> DuckResult<Vec<(String, bool)>> {
        let mut sessions_with_status = Vec::new();
        
        // 使用单个查询获取所有session及其用户名和场景信息
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT 
                a.session_id,
                COALESCE(NULLIF(a.username, ''), 'unknown_user') as username,
                COALESCE(NULLIF(a.scenario, ''), 'standard') as scenario
             FROM accelerometer_data a
             UNION
             SELECT DISTINCT 
                a.session_id,
                COALESCE(NULLIF(a.username, ''), 'unknown_user') as username,
                COALESCE(NULLIF(a.scenario, ''), 'standard') as scenario
             FROM audio_data ad
             JOIN accelerometer_data a ON ad.session_id = a.session_id
             ORDER BY session_id DESC"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,  // session_id
                row.get::<_, String>(1)?,  // username
                row.get::<_, String>(2)?,  // scenario
            ))
        })?;
        
        for row in rows {
            let (session_id, username, scenario) = row?;
            
            // 构建文件路径并检查是否存在
            let file_path = format!("data_export/{}/{}/{}.csv", username, scenario, session_id);
            let is_exported = std::path::Path::new(&file_path).exists();
            
            sessions_with_status.push((session_id, is_exported));
        }
        
        Ok(sessions_with_status)
    }

    // 获取未导出的session ID列表（优化版本）
    pub fn get_unexported_sessions(&self) -> DuckResult<Vec<String>> {
        let sessions_with_status = self.get_all_sessions_with_export_status()?;
        
        let unexported_sessions: Vec<String> = sessions_with_status
            .into_iter()
            .filter_map(|(session_id, is_exported)| {
                if !is_exported {
                    Some(session_id)
                } else {
                    info!("Session {} already exported, skipping", session_id);
                    None
                }
            })
            .collect();
        
        Ok(unexported_sessions)
    }

    // 获取所有用户名列表
    pub fn get_all_usernames(&self) -> DuckResult<Vec<String>> {
        let mut usernames = Vec::new();
        
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT 
                CASE 
                    WHEN username IS NULL OR username = '' THEN 'unknown_user'
                    ELSE username 
                END as effective_username
             FROM accelerometer_data 
             UNION 
             SELECT DISTINCT 
                CASE 
                    WHEN username IS NULL OR username = '' THEN 'unknown_user'
                    ELSE username 
                END as effective_username
             FROM audio_data 
             ORDER BY effective_username"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok(row.get::<_, String>(0)?)
        })?;
        
        for row in rows {
            usernames.push(row?);
        }
        
        // 如果没有用户名，添加默认用户
        if usernames.is_empty() {
            usernames.push("unknown_user".to_string());
        }
        
        Ok(usernames)
    }

    // 获取所有scenarios列表
    pub fn get_all_scenarios(&self) -> DuckResult<Vec<String>> {
        let mut scenarios = std::collections::HashSet::new();
        
        // 从加速度数据表查询所有不同的scenario
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT 
                CASE 
                    WHEN scenario IS NULL OR scenario = '' THEN 'standard'
                    ELSE scenario 
                END as effective_scenario
             FROM accelerometer_data 
             ORDER BY effective_scenario"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok(row.get::<_, String>(0)?)
        })?;
        
        for row in rows {
            scenarios.insert(row?);
        }
        
        // 转换为排序的向量
        let mut scenarios_vec: Vec<String> = scenarios.into_iter().collect();
        scenarios_vec.sort();
        
        // 如果没有scenario，添加默认scenario
        if scenarios_vec.is_empty() {
            scenarios_vec.push("standard".to_string());
        }
        
        Ok(scenarios_vec)
    }

    // 获取指定用户的session列表
    pub fn get_sessions_by_username(&self, username: &str) -> DuckResult<Vec<String>> {
        let mut sessions = Vec::new();
        
        if username == "unknown_user" {
            // 对于unknown_user，查找username为空或NULL的记录
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT session_id FROM accelerometer_data 
                 WHERE username IS NULL OR username = '' 
                 UNION 
                 SELECT DISTINCT a.session_id FROM audio_data a
                 JOIN accelerometer_data acc ON a.session_id = acc.session_id
                 WHERE acc.username IS NULL OR acc.username = ''
                 ORDER BY session_id DESC"
            )?;
            
            let rows = stmt.query_map([], |row| {
                Ok(row.get::<_, String>(0)?)
            })?;
            
            for row in rows {
                sessions.push(row?);
            }
        } else {
            // 对于其他用户，正常查询
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT session_id FROM accelerometer_data 
                 WHERE username = ? 
                 UNION 
                 SELECT DISTINCT a.session_id FROM audio_data a
                 JOIN accelerometer_data acc ON a.session_id = acc.session_id
                 WHERE acc.username = ?
                 ORDER BY session_id DESC"
            )?;
            
            let rows = stmt.query_map([username, username], |row| {
                Ok(row.get::<_, String>(0)?)
            })?;
            
            for row in rows {
                sessions.push(row?);
            }
        }
        
        Ok(sessions)
    }

    // 获取指定用户和scenario的session列表
    pub fn get_sessions_by_username_and_scenario(&self, username: &str, scenario: &str) -> DuckResult<Vec<String>> {
        let mut sessions = Vec::new();
        
        // 根据用户名和scenario查询sessions
        if username == "unknown_user" {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT session_id FROM accelerometer_data 
                 WHERE (username IS NULL OR username = '') 
                 AND (scenario IS NULL OR scenario = '' OR scenario = ?)
                 ORDER BY session_id DESC"
            )?;
            
            let rows = stmt.query_map([scenario], |row| {
                Ok(row.get::<_, String>(0)?)
            })?;
            
            for row in rows {
                sessions.push(row?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT session_id FROM accelerometer_data 
                 WHERE username = ? 
                 AND (scenario IS NULL OR scenario = '' OR scenario = ?)
                 ORDER BY session_id DESC"
            )?;
            
            let rows = stmt.query_map([username, scenario], |row| {
                Ok(row.get::<_, String>(0)?)
            })?;
            
            for row in rows {
                sessions.push(row?);
            }
        }
        
        Ok(sessions)
    }

    // 获取session对应的用户名
    pub fn get_username_for_session(&self, session_id: &str) -> DuckResult<String> {
        // 首先尝试从加速度数据表获取用户名
        let mut stmt = self.conn.prepare(
            "SELECT username FROM accelerometer_data WHERE session_id = ? LIMIT 1"
        )?;
        
        match stmt.query_row([session_id], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(username) => return Ok(username),
            Err(_) => {
                // 如果加速度数据表中没有，尝试从音频数据表获取
                let mut stmt = self.conn.prepare(
                    "SELECT username FROM audio_data WHERE session_id = ? LIMIT 1"
                )?;
                
                match stmt.query_row([session_id], |row| {
                    row.get::<_, String>(0)
                }) {
                    Ok(username) => Ok(username),
                    Err(_) => Ok(String::new()), // 如果都没有找到，返回空字符串
                }
            }
        }
    }

    // 获取session对应的场景
    pub fn get_scenario_for_session(&self, session_id: &str) -> DuckResult<String> {
        // 从加速度数据表获取场景信息
        let mut stmt = self.conn.prepare(
            "SELECT scenario FROM accelerometer_data WHERE session_id = ? LIMIT 1"
        )?;
        
        match stmt.query_row([session_id], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(scenario) => Ok(scenario),
            Err(_) => Ok("standard".to_string()), // 如果没有找到，返回默认值
        }
    }

    // 检查session是否已经导出
    pub fn is_session_exported(&self, session_id: &str) -> DuckResult<bool> {
        let username = self.get_username_for_session(session_id)?;
        let scenario = self.get_scenario_for_session(session_id)?;
        
        // 处理空用户名和场景
        let user_dir = if username.is_empty() {
            "unknown_user"
        } else {
            &username
        };
        
        let scenario_dir = if scenario.is_empty() {
            "standard"
        } else {
            &scenario
        };
        
        // 构建文件路径
        let file_path = format!("data_export/{}/{}/{}.csv", user_dir, scenario_dir, session_id);
        
        // 检查文件是否存在
        Ok(std::path::Path::new(&file_path).exists())
    }

    // 获取指定session的加速度数据
    pub fn get_accelerometer_data_by_session(&self, session_id: &str) -> DuckResult<Vec<DataPoint>> {
        let mut data = Vec::new();
        
        let mut stmt = self.conn.prepare(
            "SELECT timestamp_ms, x, y, z, gx, gy, gz FROM accelerometer_data 
             WHERE session_id = ? 
             ORDER BY timestamp_ms"
        )?;
        
        let rows = stmt.query_map([session_id], |row| {
            Ok(DataPoint {
                timestamp: row.get::<_, i64>(0)?,
                x: row.get::<_, f64>(1)?,
                y: row.get::<_, f64>(2)?,
                z: row.get::<_, f64>(3)?,
                gx: row.get::<_, f64>(4)?,
                gy: row.get::<_, f64>(5)?,
                gz: row.get::<_, f64>(6)?,
            })
        })?;
        
        for row in rows {
            data.push(row?);
        }
        
        Ok(data)
    }

    // 获取指定session的音频数据
    pub fn get_audio_data_by_session(&self, session_id: &str) -> DuckResult<Vec<(i64, i64, Vec<f64>, u32, u8, String)>> {
        let mut data = Vec::new();
        
        let mut stmt = self.conn.prepare(
            "SELECT start_timestamp_ms, end_timestamp_ms, audio_blob, sample_rate, channels, format FROM audio_data 
             WHERE session_id = ? 
             ORDER BY start_timestamp_ms"
        )?;
        
        let rows = stmt.query_map([session_id], |row| {
            let start_timestamp: i64 = row.get(0)?;
            let end_timestamp: i64 = row.get(1)?;
            let audio_blob: Vec<u8> = row.get(2)?;
            let sample_rate: i32 = row.get(3)?;
            let channels: i32 = row.get(4)?;
            let format: String = row.get(5)?;
            
            // 将音频字节数据转换回f64样本
            let mut samples = Vec::new();
            for chunk in audio_blob.chunks_exact(2) {
                let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
                let sample_f64 = sample_i16 as f64 / 32767.0;
                samples.push(sample_f64);
            }
            
            Ok((start_timestamp, end_timestamp, samples, sample_rate as u32, channels as u8, format))
        })?;
        
        for row in rows {
            data.push(row?);
        }
        
        Ok(data)
    }


    // 标记session为已导出（现在不需要，因为通过文件存在性检查）
    pub fn mark_session_exported(&self, _session_id: &str) -> DuckResult<()> {
        // 不再需要数据库表记录，文件存在即表示已导出
        Ok(())
    }

    // 删除指定session的所有数据
    pub fn delete_session(&self, session_id: &str) -> DuckResult<usize> {
        let mut total_deleted = 0;
        
        // 删除加速度数据
        let acc_deleted = self.conn.execute(
            "DELETE FROM accelerometer_data WHERE session_id = ?",
            [session_id],
        )?;
        total_deleted += acc_deleted;
        
        // 删除音频数据
        let audio_deleted = self.conn.execute(
            "DELETE FROM audio_data WHERE session_id = ?",
            [session_id],
        )?;
        total_deleted += audio_deleted;
        
        info!("Deleted session {}: {} accelerometer records, {} audio records", 
              session_id, acc_deleted, audio_deleted);
        
        Ok(total_deleted)
    }
}

pub fn generate_session_id() -> String {
    use chrono::Utc;
    format!("session_{}", Utc::now().format("%Y%m%d_%H%M%S"))
}
