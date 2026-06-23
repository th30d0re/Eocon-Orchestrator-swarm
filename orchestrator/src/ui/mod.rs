pub mod agent_card;
pub mod chat_panel;
pub mod gist_widget;
pub mod layout;
pub mod prompt;
pub mod status_bar;
pub mod theme;
pub mod workflow_panel;

use ratatui::Frame;

use crate::app::App;

/// Render the full swarm UI.
pub fn render_swarm(frame: &mut Frame, app: &App) {
    let (chat, workflow, status, input) = layout::compute_swarm(frame.area(), app.swarm_layout);

    if let Some(rect) = chat {
        frame.render_widget(chat_panel::render(app), rect);
    }
    if let Some(rect) = workflow {
        frame.render_widget(workflow_panel::render(app), rect);
    }
    frame.render_widget(status_bar::render(app), status);
    frame.render_widget(prompt::input_bar(app), input);
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    #[test]
    fn swarm_render_smoke() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(true);
        terminal.draw(|frame| render_swarm(frame, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let row0: String = (0..80)
            .map(|col| buf[(col, 0)].symbol().to_string())
            .collect();
        assert!(row0.contains("Chat") || row0.contains("Workflow"));
    }
}
