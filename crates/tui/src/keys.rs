use input::{Key, KeyEvent};
use ratatui::crossterm::event::{KeyCode, KeyEvent as CtKey, KeyEventKind, KeyModifiers};

/// Convert a crossterm key event. Auto-repeat counts as a press, so holding an arrow keeps
/// stepping a fader; the keymap is what stops a held cue from retriggering.
pub fn convert_key(e: CtKey) -> Option<KeyEvent> {
    let key = match e.code {
        KeyCode::Char(' ') => Key::Space,
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Tab | KeyCode::BackTab => Key::Tab,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        _ => return None,
    };
    let mut ev = match e.kind {
        KeyEventKind::Press | KeyEventKind::Repeat => KeyEvent::press(key),
        KeyEventKind::Release => KeyEvent::release(key),
    };
    ev.alt = e.modifiers.contains(KeyModifiers::ALT);
    ev.shift = e.modifiers.contains(KeyModifiers::SHIFT);
    ev.ctrl = e.modifiers.contains(KeyModifiers::CONTROL);
    Some(ev)
}
