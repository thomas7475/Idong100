//! Public data model for marker frames.

#[derive(Debug, Clone)]
pub struct Marker {
    pub slot: u8,          // 0..=9 observed (chip: up to 15)
    pub x_mm: f32,
    pub y_mm: f32,
    pub z_mm: f32,
    pub quality: i32,      // slot+0x24; >= 50 = tracked
    pub age: f64,          // slot+0x18 gate value
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Quality { Green, Yellow, Red } // vendor thresholds 20/35/35

#[derive(Debug, Clone)]
pub struct MarkerFrame {
    pub frame_counter: u32, // record +0; continuity delta <= 1
    pub mode: u32,           // record +0x3B8
    pub markers: Vec<Marker>,
    pub link_quality: Quality,
    pub dropped: bool,
}
