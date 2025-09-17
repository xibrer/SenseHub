use duckdb::{Connection, Result as DuckResult};
use log::{info, error};

pub struct DatabaseSchema;

impl DatabaseSchema {
    pub fn create_tables_and_migrate(conn: &Connection) -> DuckResult<()> {
        Self::create_basic_tables(conn)?;
        info!("Basic database tables created successfully");

        Self::migrate_accelerometer_table(conn)?;
        Self::migrate_username_columns(conn)?;
        Self::migrate_scenario_column(conn)?;

        info!("Database migration completed successfully");
        Ok(())
    }

    fn create_basic_tables(conn: &Connection) -> DuckResult<()> {
        conn.execute(
            "CREATE SEQUENCE IF NOT EXISTS accelerometer_data_seq",
            [],
        )?;

        conn.execute(
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

        conn.execute(
            "CREATE SEQUENCE IF NOT EXISTS audio_data_seq",
            [],
        )?;

        conn.execute(
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

        Ok(())
    }

    fn migrate_accelerometer_table(conn: &Connection) -> DuckResult<()> {
        let has_gyro_columns = Self::check_gyro_columns_exist(conn)?;

        if !has_gyro_columns {
            info!("Adding gyroscope columns to accelerometer_data table");

            conn.execute("ALTER TABLE accelerometer_data ADD COLUMN gx DOUBLE DEFAULT 0.0", [])?;
            conn.execute("ALTER TABLE accelerometer_data ADD COLUMN gy DOUBLE DEFAULT 0.0", [])?;
            conn.execute("ALTER TABLE accelerometer_data ADD COLUMN gz DOUBLE DEFAULT 0.0", [])?;

            info!("Successfully added gyroscope columns");
        } else {
            info!("Gyroscope columns already exist in accelerometer_data table");
        }

        Ok(())
    }

    fn check_gyro_columns_exist(conn: &Connection) -> DuckResult<bool> {
        let result = conn.execute("SELECT gx, gy, gz FROM accelerometer_data LIMIT 1", []);

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

    fn migrate_username_columns(conn: &Connection) -> DuckResult<()> {
        let acc_has_username = Self::check_username_column_exists(conn, "accelerometer_data")?;

        if !acc_has_username {
            info!("Adding username column to accelerometer_data table");
            conn.execute("ALTER TABLE accelerometer_data ADD COLUMN username VARCHAR DEFAULT ''", [])?;
            info!("Successfully added username column to accelerometer_data table");
        } else {
            info!("Username column already exists in accelerometer_data table");
        }

        let audio_has_username = Self::check_username_column_exists(conn, "audio_data")?;

        if !audio_has_username {
            info!("Adding username column to audio_data table");
            conn.execute("ALTER TABLE audio_data ADD COLUMN username VARCHAR DEFAULT ''", [])?;
            info!("Successfully added username column to audio_data table");
        } else {
            info!("Username column already exists in audio_data table");
        }

        Ok(())
    }

    fn check_username_column_exists(conn: &Connection, table_name: &str) -> DuckResult<bool> {
        let table_exists_query = format!(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = '{}'",
            table_name
        );

        let table_exists = match conn.query_row(&table_exists_query, [], |row| {
            Ok(row.get::<_, i64>(0)? > 0)
        }) {
            Ok(exists) => exists,
            Err(_) => {
                let test_query = format!("SELECT COUNT(*) FROM {} LIMIT 1", table_name);
                conn.execute(&test_query, []).is_ok()
            }
        };

        if !table_exists {
            info!("Table {} does not exist, username column does not exist", table_name);
            return Ok(false);
        }

        let query = format!("SELECT username FROM {} LIMIT 1", table_name);
        let result = conn.execute(&query, []);

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

    fn migrate_scenario_column(conn: &Connection) -> DuckResult<()> {
        let has_scenario = Self::check_scenario_column_exists(conn)?;

        if !has_scenario {
            info!("Adding scenario column to accelerometer_data table");
            conn.execute("ALTER TABLE accelerometer_data ADD COLUMN scenario VARCHAR DEFAULT 'standard'", [])?;
            info!("Successfully added scenario column to accelerometer_data table");
        } else {
            info!("Scenario column already exists in accelerometer_data table");
        }

        Ok(())
    }

    fn check_scenario_column_exists(conn: &Connection) -> DuckResult<bool> {
        let result = conn.execute("SELECT scenario FROM accelerometer_data LIMIT 1", []);

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
}