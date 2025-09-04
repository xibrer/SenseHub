#[derive(serde::Deserialize, Clone, Debug)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub timestamp: i64,
}

impl DataPoint {
    pub fn new(x: f64, y: f64, z: f64, timestamp: i64) -> Self {
        Self { x, y, z, timestamp }
    }
}
