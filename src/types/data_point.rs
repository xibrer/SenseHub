#[derive(serde::Deserialize, Clone, Debug)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub gx: f64,  // 陀螺仪 X 轴
    pub gy: f64,  // 陀螺仪 Y 轴
    pub gz: f64,  // 陀螺仪 Z 轴
    pub timestamp: i64,
}

impl DataPoint {
    pub fn new(x: f64, y: f64, z: f64, gx: f64, gy: f64, gz: f64, timestamp: i64) -> Self {
        Self { x, y, z, gx, gy, gz, timestamp }
    }
}
