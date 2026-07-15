use eocon_swarm::term::TermEmulator;

#[test]
fn alt_screen_enter_leave_restores_primary_grid() {
    let mut emu = TermEmulator::new(10, 40, 0);
    emu.advance(b"primary line 1\nprimary line 2");
    assert!(emu.full_text().to_string().contains("primary line 1"));

    // Enter alternate screen (CSI ?1049h), write something, then leave (CSI ?1049l).
    emu.advance(b"\x1b[?1049h");
    emu.advance(b"alternate content");
    assert!(emu.full_text().to_string().contains("alternate content"));
    assert!(!emu.full_text().to_string().contains("primary line 1"));

    emu.advance(b"\x1b[?1049l");
    let text = emu.full_text().to_string();
    assert!(text.contains("primary line 1"));
    assert!(text.contains("primary line 2"));
    assert!(!text.contains("alternate content"));
}

#[test]
fn sgr_truecolor_and_bold_map_to_style() {
    let mut emu = TermEmulator::new(5, 40, 0);
    // CSI 38;2;<r>;<g>;<b>m foreground truecolor, CSI 48;2;<r>;<g>;<b>m background truecolor, 1m bold.
    emu.advance(b"\x1b[38;2;255;100;50m\x1b[48;2;10;20;30m\x1b[1mstyled text\x1b[0m");

    let text = emu.full_text();
    let line = text.lines.first().expect("expected one line");
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content.trim_end(), "styled text");

    let non_empty = line
        .spans
        .iter()
        .find(|s| !s.content.is_empty())
        .expect("expected a non-empty span");
    let style = non_empty.style;
    assert_eq!(style.fg, Some(ratatui::style::Color::Rgb(255, 100, 50)));
    assert_eq!(style.bg, Some(ratatui::style::Color::Rgb(10, 20, 30)));
    assert!(style.add_modifier.contains(ratatui::style::Modifier::BOLD));
}

#[test]
fn absolute_cursor_addressing_writes_at_directed_position() {
    let mut emu = TermEmulator::new(10, 20, 0);
    // Move cursor to row 3, col 5 (1-based in CSI H) and write text.
    emu.advance(b"\x1b[3;5Hat cursor");

    let text = emu.full_text();
    let lines: Vec<String> = text.lines.iter().map(|l| l.to_string()).collect();
    assert_eq!(lines[2].trim_end(), "    at cursor");
}

#[test]
fn resize_does_not_panic_and_preserves_content() {
    let mut emu = TermEmulator::new(10, 20, 0);
    emu.advance(b"hello world\nthis is a longer line of text");

    emu.resize(5, 10);
    assert_eq!(emu.dims(), (5, 10));

    // Content should still be present (possibly wrapped/reflowed).
    let text = emu.full_text().to_string();
    assert!(text.contains("hello") && text.contains("worl"));
    assert!(text.contains("longer li"));

    emu.resize(15, 40);
    assert_eq!(emu.dims(), (15, 40));
    let text = emu.full_text().to_string();
    assert!(text.contains("hello"));
    assert!(text.contains("longer li"));
}

#[test]
fn real_tui_startup_sample_parses_cleanly() {
    let sample = include_bytes!("fixtures/claude_startup.bin");
    let mut emu = TermEmulator::new(24, 80, 1000);
    // Should not panic on a real-world escape-sequence stream.
    emu.advance(sample);
    let (rows, cols) = emu.dims();
    assert_eq!((rows, cols), (24, 80));
    // Title should be captured from the OSC window-title sequence.
    assert_eq!(emu.title(), Some("✳ Claude Code"));
}
