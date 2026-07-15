use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

/// A thin wrapper around `vt100::Parser` exposing the surface the
/// orchestrator already relied on from `PtyScreen`, plus title, cursor,
/// and alternate-screen accessors.
pub struct TermEmulator {
    parser: vt100::Parser<TitleCallbacks>,
    scroll_offset: usize,
}

#[derive(Default)]
struct TitleCallbacks {
    title: Option<String>,
}

impl vt100::Callbacks for TitleCallbacks {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = Some(String::from_utf8_lossy(title).into_owned());
    }
}

impl TermEmulator {
    pub fn new(rows: usize, cols: usize, scrollback: usize) -> Self {
        let parser = vt100::Parser::new_with_callbacks(
            rows as u16,
            cols as u16,
            scrollback,
            TitleCallbacks::default(),
        );
        Self {
            parser,
            scroll_offset: 0,
        }
    }

    /// Feed raw terminal bytes into the parser.
    pub fn advance(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    /// Resize both the parser grid and the scrollback viewport.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.parser
            .screen_mut()
            .set_size(rows as u16, cols as u16);
        // Clamp the scroll offset to the actual stored scrollback length.
        let max = self.stored_scrollback_len();
        self.scroll_offset = self.scroll_offset.min(max);
        self.parser
            .screen_mut()
            .set_scrollback(self.scroll_offset);
    }

    /// Current grid dimensions `(rows, cols)`.
    pub fn dims(&self) -> (usize, usize) {
        let (rows, cols) = self.parser.screen().size();
        (rows as usize, cols as usize)
    }

    /// The terminal title last set by an OSC window-title sequence, if any.
    pub fn title(&self) -> Option<&str> {
        self.parser.callbacks().title.as_deref()
    }

    /// Whether the emulator is currently displaying the alternate screen buffer.
    pub fn is_alt_screen(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    /// Cursor position `(col, row)` and visibility.
    pub fn cursor(&self) -> (u16, u16, bool) {
        let (row, col) = self.parser.screen().cursor_position();
        let visible = !self.parser.screen().hide_cursor();
        (col, row, visible)
    }

    /// Scroll the viewport up by `count` lines into scrollback.
    pub fn scroll_up(&mut self, count: usize) {
        let max = self.stored_scrollback_len();
        self.scroll_offset = (self.scroll_offset + count).min(max);
        self.parser.screen_mut().set_scrollback(self.scroll_offset);
    }

    /// Scroll the viewport down by `count` lines toward the live grid.
    pub fn scroll_down(&mut self, count: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(count);
        self.parser.screen_mut().set_scrollback(self.scroll_offset);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.parser.screen_mut().set_scrollback(0);
    }

    /// Number of lines currently held in scrollback.
    pub fn scrollback_len(&self) -> usize {
        self.parser.screen().scrollback()
    }

    pub fn scrollback_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn set_scrollback_offset(&mut self, offset: usize) {
        let max = self.stored_scrollback_len();
        self.scroll_offset = offset.min(max);
        self.parser.screen_mut().set_scrollback(self.scroll_offset);
    }

    /// The visible viewport as styled `ratatui::Text`, honoring the current
    /// scrollback offset.
    pub fn as_text(&self) -> Text<'static> {
        let (rows, cols) = self.dims();
        let mut lines = Vec::with_capacity(rows);
        for row in 0..rows {
            lines.push(self.row_to_line(row as u16, cols));
        }
        Text::from(lines)
    }

    /// The entire scrollback + live grid as styled `ratatui::Text`, ignoring
    /// the viewport offset and trimming trailing blank rows.
    pub fn full_text(&mut self) -> Text<'static> {
        let saved_offset = self.scroll_offset;
        let (rows, cols) = self.dims();
        let stored_scrollback = self.stored_scrollback_len();

        // Slide the viewport from the oldest scrollback rows to the live grid,
        // capturing one viewport-sized chunk at a time.
        let mut lines: Vec<Line<'_>> = Vec::with_capacity(stored_scrollback + rows);

        let mut offset = stored_scrollback;
        while offset > rows {
            self.parser.screen_mut().set_scrollback(offset);
            for row in 0..rows {
                lines.push(self.row_to_line(row as u16, cols));
            }
            offset -= rows;
        }

        if offset > 0 {
            // This viewport straddles scrollback and the live grid: keep only
            // the `offset` scrollback rows.
            self.parser.screen_mut().set_scrollback(offset);
            for row in 0..offset {
                lines.push(self.row_to_line(row as u16, cols));
            }
        }

        // Live grid at offset 0.
        self.parser.screen_mut().set_scrollback(0);
        for row in 0..rows {
            lines.push(self.row_to_line(row as u16, cols));
        }

        // Restore the viewport offset.
        self.scroll_offset = saved_offset;
        self.parser
            .screen_mut()
            .set_scrollback(self.scroll_offset);

        // Drop trailing blank rows.
        while lines.len() > 1 && is_blank_line(lines.last().unwrap()) {
            lines.pop();
        }

        Text::from(lines)
    }

    fn row_to_line(&self, row: u16, cols: usize) -> Line<'static> {
        let mut spans = Vec::with_capacity(cols);
        for col in 0..cols {
            if let Some(cell) = self.parser.screen().cell(row, col as u16) {
                if cell.is_wide_continuation() {
                    continue;
                }
                let style = cell_to_style(cell);
                let symbol = if cell.has_contents() {
                    cell.contents().to_string()
                } else {
                    " ".to_string()
                };
                spans.push(Span::styled(symbol, style));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        Line::from(spans)
    }

    /// Return the visible symbol and style for the cell at `(row, col)`.
    /// Wide-character continuation cells produce an empty symbol so the caller
    /// can leave the underlying buffer cell untouched.
    pub(crate) fn cell_symbol_and_style(&self, row: u16, col: u16) -> (String, Style) {
        let (rows, cols) = self.dims();
        if row as usize >= rows || col as usize >= cols {
            return (" ".to_string(), Style::default());
        }
        match self.parser.screen().cell(row, col) {
            Some(cell) if cell.is_wide_continuation() => (String::new(), Style::default()),
            Some(cell) => {
                let symbol = if cell.has_contents() {
                    cell.contents().to_string()
                } else {
                    " ".to_string()
                };
                (symbol, cell_to_style(cell))
            }
            None => (" ".to_string(), Style::default()),
        }
    }

    fn stored_scrollback_len(&mut self) -> usize {
        let saved = self.parser.screen().scrollback();
        self.parser.screen_mut().set_scrollback(usize::MAX);
        let len = self.parser.screen().scrollback();
        self.parser.screen_mut().set_scrollback(saved);
        len
    }
}

fn cell_to_style(cell: &vt100::Cell) -> Style {
    let mut style = Style::default();
    if let Some(fg) = vt100_color_to_ratatui(cell.fgcolor()) {
        style = style.fg(fg);
    }
    if let Some(bg) = vt100_color_to_ratatui(cell.bgcolor()) {
        style = style.bg(bg);
    }
    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.inverse() {
        style = style.add_modifier(Modifier::REVERSED);
    }
    style
}

fn vt100_color_to_ratatui(color: vt100::Color) -> Option<Color> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Idx(i) => Some(Color::Indexed(i)),
        vt100::Color::Rgb(r, g, b) => Some(Color::Rgb(r, g, b)),
    }
}

fn is_blank_line(line: &Line<'_>) -> bool {
    line.spans.iter().all(|span| {
        span.content
            .chars()
            .all(|c| c.is_whitespace() || c == '\0')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_dims_match_constructor() {
        let emu = TermEmulator::new(24, 80, 0);
        assert_eq!(emu.dims(), (24, 80));
    }

    #[test]
    fn advance_text_appears() {
        let mut emu = TermEmulator::new(10, 20, 0);
        emu.advance(b"hello world");
        let text = emu.full_text().to_string();
        assert!(text.contains("hello world"));
    }
}
