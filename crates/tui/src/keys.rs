use input::{Key, KeyEvent};
use ratatui::crossterm::event::{KeyCode, KeyEvent as CtKey, KeyEventKind, KeyModifiers};

/// Convert a crossterm key event. Auto-repeat is dropped so held keys (cue) don't retrigger.
pub fn convert_key(e: CtKey) -> Option<KeyEvent> {
    let key = match e.code {
        KeyCode::Char(' ') => Key::Space,
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Tab | KeyCode::BackTab => Key::Tab,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        _ => return None,
    };
    let mut ev = match e.kind {
        KeyEventKind::Press => KeyEvent::press(key),
        KeyEventKind::Release => KeyEvent::release(key),
        KeyEventKind::Repeat => return None,
    };
    ev.alt = e.modifiers.contains(KeyModifiers::ALT);
    ev.shift = e.modifiers.contains(KeyModifiers::SHIFT);
    ev.ctrl = e.modifiers.contains(KeyModifiers::CONTROL);
    Some(ev)
}
