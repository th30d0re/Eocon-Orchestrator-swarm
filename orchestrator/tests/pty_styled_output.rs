use std::time::Duration;

use eocon_swarm::agents::runner::{AgentCmd, AgentRunner, PtyRunner, PtySize};
use eocon_swarm::policy::policy::RiskLabel;
use eocon_swarm::term::TermEmulator;

/// Manually spawn a PTY process that emits styled escape sequences and verify
/// the vt100-backed emulator captures the styles faithfully.
#[test]
fn pty_agent_emits_styled_output() {
    let runner = PtyRunner;
    let cmd = AgentCmd {
        program: "bash".to_string(),
        args: vec![
            "-c".to_string(),
            "python3 -c \"import sys; sys.stdout.buffer.write(b'plain\\x1b[31mred\\x1b[0m\\x1b[1mbold\\x1b[0m\\n')\"".to_string(),
        ],
        env: Vec::new(),
        cwd: None,
        risk_tier: RiskLabel::Low,
        agent_id: Some("styled-test".to_string()),
    };

    let size = PtySize {
        rows: 10,
        cols: 40,
        pixel_width: 0,
        pixel_height: 0,
    };
    let mut handle = runner.spawn(cmd, size).expect("spawn should succeed");

    // Drain raw bytes from the PTY into the emulator.
    let mut emulator = TermEmulator::new(size.rows as usize, size.cols as usize, 100);
    let start = std::time::Instant::now();
    loop {
        while let Ok(msg) = handle.rx.try_recv() {
            match msg {
                eocon_swarm::agents::runner::RunnerMsg::Bytes(buf) => emulator.advance(&buf),
                eocon_swarm::agents::runner::RunnerMsg::Error(e) => panic!("PTY error: {}", e),
            }
        }
        if let Ok(Some(status)) = handle.child.try_wait() {
            // Child exited; drain anything left in the channel.
            while let Ok(msg) = handle.rx.try_recv() {
                if let eocon_swarm::agents::runner::RunnerMsg::Bytes(buf) = msg {
                    emulator.advance(&buf);
                }
            }
            assert_eq!(status.exit_code(), 0);
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = handle.child.kill();
            panic!("PTY child did not exit in time");
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let text = emulator.full_text().to_string();
    assert!(
        text.contains("plain"),
        "expected plain text in output: {}",
        text
    );
    assert!(text.contains("red"), "expected red text in output: {}", text);
    assert!(
        text.contains("bold"),
        "expected bold text in output: {}",
        text
    );

    // Verify the styled cells actually carry the expected attributes.
    let text = emulator.full_text();
    let line = text.lines.first().expect("one line");
    let red_r = line
        .spans
        .iter()
        .find(|s| s.content == "r")
        .expect("r cell exists");
    assert_eq!(red_r.style.fg, Some(ratatui::style::Color::Indexed(1)));

    let bold_b = line
        .spans
        .iter()
        .find(|s| s.content == "b")
        .expect("b cell exists");
    assert!(bold_b
        .style
        .add_modifier
        .contains(ratatui::style::Modifier::BOLD));
}
