#[derive(serde::Deserialize, Clone, Debug)]
pub struct DataPoint {
    pub packet_id: u64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub timestamp: i64,
}

impl DataPoint {
    pub fn new(packet_id: u64, x: f64, y: f64, z: f64, timestamp: i64) -> Self {
        Self { packet_id, x, y, z, timestamp }
    }
}
