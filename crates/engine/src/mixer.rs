use crate::Deck;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckId { A, B }
impl DeckId { pub fn other(self) -> Self { todo!() } }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CrossfaderCurve { Linear, #[default] ConstantPower, Cut }

pub fn crossfader_gains(_x: f32, _c: CrossfaderCurve) -> (f32, f32) { todo!() }

pub struct Engine;
impl Engine {
    pub fn new() -> Self { todo!() }
    pub fn deck(&self, _id: DeckId) -> &Deck { todo!() }
    pub fn deck_mut(&mut self, _id: DeckId) -> &mut Deck { todo!() }
    pub fn set_crossfader(&mut self, _x: f32) { todo!() }
    pub fn set_crossfader_curve(&mut self, _c: CrossfaderCurve) { todo!() }
    pub fn set_channel_fader(&mut self, _id: DeckId, _v: f32) { todo!() }
    pub fn set_headphone_cue(&mut self, _id: DeckId, _on: bool) { todo!() }
    pub fn set_cue_mix(&mut self, _v: f32) { todo!() }
    pub fn process(&mut self, _master: &mut [f32], _cue: &mut [f32]) { todo!() }
}
