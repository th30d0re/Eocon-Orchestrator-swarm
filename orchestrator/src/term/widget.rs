use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    widgets::Widget,
};

use crate::term::emulator::TermEmulator;

/// Renders a `TermEmulator` into a `ratatui::Buffer` area.
///
/// Paints the visible cell grid with styles, handles wide-character cells
/// (skipping continuation cells), and highlights the cursor when the pane is
/// attached.
pub struct TerminalPaneWidget<'a> {
    emulator: &'a TermEmulator,
    attached: bool,
}

impl<'a> TerminalPaneWidget<'a> {
    pub fn new(emulator: &'a TermEmulator) -> Self {
        Self {
            emulator,
            attached: false,
        }
    }

    /// When `true`, the cursor is drawn reversed in the cell at the emulator's
    /// current cursor position.
    pub fn attached(mut self, attached: bool) -> Self {
        self.attached = attached;
        self
    }
}

impl Widget for TerminalPaneWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (cursor_col, cursor_row, cursor_visible) = self.emulator.cursor();

        for row in 0..area.height {
            for col in 0..area.width {
                let (symbol, mut style) = self.emulator.cell_symbol_and_style(row, col);

                if self.attached
                    && cursor_visible
                    && col == cursor_col
                    && row == cursor_row
                {
                    style = style.add_modifier(Modifier::REVERSED);
                }

                let cell = &mut buf[(area.x + col, area.y + row)];
                if !symbol.is_empty() {
                    cell.set_symbol(&symbol);
                }
                cell.set_style(style);
            }
        }
    }
}
