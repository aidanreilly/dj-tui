use crate::ScreenView; use ratatui::Frame; use ratatui_image::picker::Picker;
pub struct Graphics;
impl Graphics { pub fn new(_p: Picker) -> Self { todo!() } pub fn render(&mut self, _f: &mut Frame, _v: &ScreenView) { todo!() } pub fn transmissions(&self) -> u64 { todo!() } }
