use std::sync::mpsc::Receiver;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::agents::runner::RunnerMsg;

pub struct EventHandler {
    pub rx: Receiver<RunnerMsg>,
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(rx: Receiver<RunnerMsg>, tick_rate_ms: u64) -> Self {
        Self {
            rx,
            tick_rate: Duration::from_millis(tick_rate_ms),
        }
    }

    pub fn replace_rx(&mut self, rx: Receiver<RunnerMsg>) {
        self.rx = rx;
    }

    pub fn next(&self) -> Action {
        if event::poll(self.tick_rate).unwrap_or(false) {
            if let Ok(event) = event::read() {
                match event {
                    Event::Key(key) => return map_key(key),
                    Event::Resize(cols, rows) => return Action::Resize(cols, rows),
                    _ => {}
                }
            }
        }

        match self.rx.try_recv() {
            Ok(RunnerMsg::Bytes(buf)) => Action::AgentBytes(buf),
            Ok(RunnerMsg::Error(msg)) => Action::AgentError(msg),
            Err(_) => Action::Tick,
        }
    }
}

fn map_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Quit,
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                Action::ShiftTab
            } else {
                Action::Tab
            }
        }
        KeyCode::BackTab => Action::ShiftTab,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Abort,
        KeyCode::Up => Action::PickerUp,
        KeyCode::Down => Action::PickerDown,
        KeyCode::Backspace => Action::InputBackspace,
        KeyCode::Enter => Action::InputSubmit,
        KeyCode::Left => Action::InputCursorLeft,
        KeyCode::Right => Action::InputCursorRight,
        KeyCode::PageUp => Action::ScrollUp,
        KeyCode::PageDown => Action::ScrollDown,
        KeyCode::F(1) => Action::ToggleHelp,
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::ToggleSwarmLayout
        }
        KeyCode::Char(c) => Action::InputChar(c),
        _ => Action::Tick,
    }
}
