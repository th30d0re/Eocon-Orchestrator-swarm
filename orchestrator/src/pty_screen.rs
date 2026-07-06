use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
};
use vte::{Params, Parser as VteParser, Perform};

#[derive(Debug, Clone)]
struct Cell {
    ch: char,
    style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style::default(),
        }
    }
}

pub struct PtyScreen {
    parser: VteParser,
    grid: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    rows: usize,
    cols: usize,
    current_style: Style,
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
    scrollback: Vec<Vec<Cell>>,
    scrollback_offset: usize,
    visible_rows: usize,
    visible_cols: usize,
}

impl PtyScreen {
    pub fn new(rows: usize, cols: usize) -> Self {
        let grid = vec![vec![Cell::default(); cols]; rows];
        Self {
            parser: VteParser::new(),
            grid,
            cursor_row: 0,
            cursor_col: 0,
            rows,
            cols,
            current_style: Style::default(),
            fg: None,
            bg: None,
            bold: false,
            scrollback: Vec::new(),
            scrollback_offset: 0,
            visible_rows: rows,
            visible_cols: cols,
        }
    }

    pub fn dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    pub fn visible_dims(&self) -> (usize, usize) {
        (self.visible_rows, self.visible_cols)
    }

    fn scroll_up_lines(&mut self, count: usize) {
        let count = count.min(self.rows);
        for _ in 0..count {
            let line = self.grid.remove(0);
            self.scrollback.push(line);
            self.grid.push(vec![Cell::default(); self.cols]);
        }
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.visible_rows = rows;
        self.visible_cols = cols;

        let buf_rows = rows.max(self.rows);
        let buf_cols = cols.max(self.cols);

        if buf_rows == self.rows && buf_cols == self.cols {
            self.cursor_row = self.cursor_row.min(self.rows.saturating_sub(1));
            self.cursor_col = self.cursor_col.min(self.cols.saturating_sub(1));
            return;
        }

        let mut new_grid = vec![vec![Cell::default(); buf_cols]; buf_rows];
        for r in 0..self.rows {
            for c in 0..self.cols {
                new_grid[r][c] = self.grid[r][c].clone();
            }
        }
        self.grid = new_grid;
        self.rows = buf_rows;
        self.cols = buf_cols;
        self.cursor_row = self.cursor_row.min(self.rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(self.cols.saturating_sub(1));
    }

    pub fn advance(&mut self, bytes: &[u8]) {
        let mut parser = std::mem::take(&mut self.parser);
        parser.advance(self, bytes);
        self.parser = parser;
    }

    pub fn as_text(&self) -> Text<'_> {
        let total = self.scrollback.len() + self.rows;
        let offset = self
            .scrollback_offset
            .min(total.saturating_sub(self.visible_rows));
        let start = total.saturating_sub(self.visible_rows + offset);
        let end = total.saturating_sub(offset);

        let mut lines = Vec::new();
        for i in start..end {
            if i < self.scrollback.len() {
                let line = cells_to_line(&self.scrollback[i]);
                lines.push(truncate_line(line, self.visible_cols));
            } else {
                let line = cells_to_line(&self.grid[i - self.scrollback.len()]);
                lines.push(truncate_line(line, self.visible_cols));
            }
        }

        Text::from(lines)
    }

    /// The entire buffer (scrollback + live grid) as text, ignoring the
    /// viewport offset and with trailing blank rows trimmed. Used by panes that
    /// do their own bottom-anchored windowing, so they can scroll into history.
    pub fn full_text(&self) -> Text<'_> {
        let mut rows: Vec<&Vec<Cell>> =
            Vec::with_capacity(self.scrollback.len() + self.grid.len());
        rows.extend(self.scrollback.iter());
        rows.extend(self.grid.iter());
        // Drop trailing blank rows so a bottom-anchored view isn't padded with
        // the empty cells that sit below the cursor in the live grid.
        while rows.len() > 1 && rows.last().map(|r| is_blank_row(r)).unwrap_or(false) {
            rows.pop();
        }
        let lines = rows
            .into_iter()
            .map(|row| truncate_line(cells_to_line(row), self.visible_cols))
            .collect::<Vec<_>>();
        Text::from(lines)
    }

    pub fn scroll_up(&mut self, count: usize) {
        let max = self.scrollback.len();
        self.scrollback_offset = (self.scrollback_offset + count).min(max);
    }

    pub fn scroll_down(&mut self, count: usize) {
        self.scrollback_offset = self.scrollback_offset.saturating_sub(count);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scrollback_offset = 0;
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    pub fn scrollback_offset(&self) -> usize {
        self.scrollback_offset
    }

    fn update_style(&mut self) {
        let mut style = Style::default();
        if let Some(fg) = self.fg {
            style = style.fg(fg);
        }
        if let Some(bg) = self.bg {
            style = style.bg(bg);
        }
        if self.bold {
            style = style.add_modifier(ratatui::style::Modifier::BOLD);
        }
        self.current_style = style;
    }

    fn set_sgr(&mut self, params: &Params) {
        for param in params.iter() {
            if param.is_empty() {
                continue;
            }
            match param[0] {
                0 => {
                    self.fg = None;
                    self.bg = None;
                    self.bold = false;
                }
                1 => self.bold = true,
                30 => self.fg = Some(Color::Black),
                31 => self.fg = Some(Color::Red),
                32 => self.fg = Some(Color::Green),
                33 => self.fg = Some(Color::Yellow),
                34 => self.fg = Some(Color::Blue),
                35 => self.fg = Some(Color::Magenta),
                36 => self.fg = Some(Color::Cyan),
                37 => self.fg = Some(Color::White),
                40 => self.bg = Some(Color::Black),
                41 => self.bg = Some(Color::Red),
                42 => self.bg = Some(Color::Green),
                43 => self.bg = Some(Color::Yellow),
                44 => self.bg = Some(Color::Blue),
                45 => self.bg = Some(Color::Magenta),
                46 => self.bg = Some(Color::Cyan),
                47 => self.bg = Some(Color::White),
                90 => self.fg = Some(Color::DarkGray),
                91 => self.fg = Some(Color::LightRed),
                92 => self.fg = Some(Color::LightGreen),
                93 => self.fg = Some(Color::LightYellow),
                94 => self.fg = Some(Color::LightBlue),
                95 => self.fg = Some(Color::LightMagenta),
                96 => self.fg = Some(Color::LightCyan),
                97 => self.fg = Some(Color::White),
                _ => {}
            }
        }
        self.update_style();
    }
}

impl Perform for PtyScreen {
    fn print(&mut self, c: char) {
        if self.cursor_row >= self.rows || self.cursor_col >= self.cols {
            return;
        }
        self.grid[self.cursor_row][self.cursor_col] = Cell {
            ch: c,
            style: self.current_style,
        };
        self.cursor_col += 1;
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
            if self.cursor_row >= self.rows {
                self.scroll_up_lines(1);
                self.cursor_row = self.rows - 1;
            }
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.cursor_row += 1;
                self.cursor_col = 0;
                if self.cursor_row >= self.rows {
                    self.scroll_up_lines(1);
                    self.cursor_row = self.rows - 1;
                }
            }
            b'\r' => {
                self.cursor_col = 0;
            }
            b'\t' => {
                let next_tab = (self.cursor_col + 8) & !7;
                self.cursor_col = next_tab.min(self.cols - 1);
            }
            0x08 => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, c: char) {
        if intermediates.is_empty() {
            match c {
                'm' => self.set_sgr(params),
                'H' | 'f' => {
                    let mut iter = params.iter();
                    let row = iter.next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                    let col = iter.next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                    self.cursor_row = row.saturating_sub(1).min(self.rows.saturating_sub(1));
                    self.cursor_col = col.saturating_sub(1).min(self.cols.saturating_sub(1));
                }
                'J' => {
                    let mode = params
                        .iter()
                        .next()
                        .and_then(|p| p.first())
                        .copied()
                        .unwrap_or(0);
                    match mode {
                        0 => {
                            for c in self.cursor_col..self.cols {
                                self.grid[self.cursor_row][c] = Cell::default();
                            }
                            for r in (self.cursor_row + 1)..self.rows {
                                for c in 0..self.cols {
                                    self.grid[r][c] = Cell::default();
                                }
                            }
                        }
                        1 => {
                            for r in 0..self.cursor_row {
                                for c in 0..self.cols {
                                    self.grid[r][c] = Cell::default();
                                }
                            }
                            for c in 0..=self.cursor_col {
                                self.grid[self.cursor_row][c] = Cell::default();
                            }
                        }
                        2 | 3 => {
                            for r in 0..self.rows {
                                for c in 0..self.cols {
                                    self.grid[r][c] = Cell::default();
                                }
                            }
                        }
                        _ => {}
                    }
                }
                'K' => {
                    let mode = params
                        .iter()
                        .next()
                        .and_then(|p| p.first())
                        .copied()
                        .unwrap_or(0);
                    match mode {
                        0 => {
                            for c in self.cursor_col..self.cols {
                                self.grid[self.cursor_row][c] = Cell::default();
                            }
                        }
                        1 => {
                            for c in 0..=self.cursor_col {
                                self.grid[self.cursor_row][c] = Cell::default();
                            }
                        }
                        2 => {
                            for c in 0..self.cols {
                                self.grid[self.cursor_row][c] = Cell::default();
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _c: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
}

fn is_blank_row(cells: &[Cell]) -> bool {
    cells.iter().all(|cell| cell.ch == ' ')
}

fn cells_to_line(cells: &[Cell]) -> Line<'_> {
    let spans: Vec<Span> = cells
        .iter()
        .map(|cell| Span::styled(cell.ch.to_string(), cell.style))
        .collect();
    Line::from(spans)
}

fn truncate_line(line: Line<'_>, max_cols: usize) -> Line<'_> {
    let spans = line.spans.into_iter().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut col = 0;
    for span in spans {
        let text = span.content.to_string();
        let chars: Vec<char> = text.chars().collect();
        if col + chars.len() > max_cols {
            let take = max_cols.saturating_sub(col);
            if take > 0 {
                let trimmed: String = chars.into_iter().take(take).collect();
                result.push(Span::styled(trimmed, span.style));
            }
            break;
        }
        col += chars.len();
        result.push(span);
    }
    Line::from(result)
}
