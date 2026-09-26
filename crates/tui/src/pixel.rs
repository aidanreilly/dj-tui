use image::{Rgba, RgbaImage};
pub const PLAYHEAD_WIDTH: u32 = 2;
#[derive(Debug, Clone)] pub struct Palette { pub low: Rgba<u8>, pub high: Rgba<u8>, pub centre_line: Rgba<u8>, pub playhead: Rgba<u8> }
impl Default for Palette { fn default() -> Self { todo!() } }
pub struct WaveformBitmaps;
impl WaveformBitmaps { pub fn rasterize(_r: &[[f32; 2]], _w: u32, _h: u32, _p: &Palette) -> Self { todo!() } pub fn normal(&self) -> &RgbaImage { todo!() } pub fn dimmed(&self) -> &RgbaImage { todo!() } pub fn compose(&self, _x: Option<u32>, _p: &Palette) -> RgbaImage { todo!() } }
pub fn playhead_x(_pos: f64, _dur: f64, _w: u32) -> Option<u32> { todo!() }
#[derive(Default)] pub struct PixelWaveform;
impl PixelWaveform { pub fn update(&mut self, _r: &[[f32; 2]], _s: (u32, u32), _x: Option<u32>, _p: &Palette) -> Option<RgbaImage> { todo!() } pub fn rasterizations(&self) -> u64 { todo!() } pub fn has_image(&self) -> bool { todo!() } }
