//! Browser mode. Letters are letters here, so every command sits on a modifier or a special
//! key. That gives one rule to learn: Alt plus a letter is a browser command.

use super::{Key, KeyEvent};
use crate::{Action, Dir};
use engine::DeckId;

pub fn handle(e: KeyEvent, focused: DeckId) -> Option<Action> {
    if e.ctrl {
        return match e.key {
            Key::Char('u') => Some(Action::BrowserClear),
            _ => None,
        };
    }
    if e.alt {
        return match e.key {
            Key::Char('s') => Some(Action::BrowserSort(false)),
            Key::Char('S') => Some(Action::BrowserSort(true)),
            Key::Char('a') => Some(Action::AnalyseLibrary),
            Key::Char('b') => Some(Action::BrowserFilterBpm),
            Key::Char('k') => Some(Action::BrowserFilterKey),
            Key::Char('g') => Some(Action::BrowserFilterGenre),
            Key::Char('d') => Some(Action::BrowserLookup),
            Key::Char('f') => Some(Action::BrowserFullscreen),
            _ => None,
        };
    }
    match e.key {
        // Plain arrows move the list. A modified arrow is a mixer gesture in mix mode and
        // nothing at all here, so it is dropped rather than half-honoured.
        Key::Up if !e.shift => Some(Action::BrowserMove(Dir::Up)),
        Key::Down if !e.shift => Some(Action::BrowserMove(Dir::Down)),
        Key::Up | Key::Down | Key::Left | Key::Right => None,
        Key::Enter => Some(Action::Load(focused)),
        Key::Esc => Some(Action::BrowserLeave),
        Key::Backspace => Some(Action::BrowserBackspace),
        Key::Space => Some(Action::BrowserType(' ')),
        Key::Char(c) => Some(Action::BrowserType(c)),
        // Tab is handled ahead of the mode, so it never arrives here.
        Key::Tab => None,
    }
}
