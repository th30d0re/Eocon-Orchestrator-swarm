pub mod agent_card;
pub mod chat_panel;
pub mod gist_widget;
pub mod layout;
pub mod mention_picker;
pub mod models_panel;
pub mod picker;
pub mod prompt;
pub mod splash;
pub mod status_bar;
pub mod theme;
pub mod workflow_panel;

use ratatui::Frame;

use crate::app::App;

/// Render the full swarm UI.
pub fn render_swarm(frame: &mut Frame, app: &App) {
    // Startup splash takes over the whole screen until it finishes.
    if app.startup_phase != crate::app::StartupPhase::Done {
        splash::render(frame, app);
        return;
    }

    let input_height = prompt::input_height(app, frame.area().width);
    let (chat, workflow, models, status, input) =
        layout::compute_swarm(frame.area(), app.swarm_layout, input_height);

    if let Some(rect) = chat {
        frame.render_widget(chat_panel::render(app), rect);
    }
    if let Some(rect) = workflow {
        frame.render_widget(workflow_panel::render(app), rect);
    }
    if let Some(rect) = models {
        frame.render_widget(models_panel::render(app), rect);
    }
    frame.render_widget(status_bar::render(app), status);
    frame.render_widget(prompt::input_bar(app), input);

    // Render pickers last so they float above everything else.
    picker::render(frame, app);
    mention_picker::render(frame, app);
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
        let mut app = App::new(true);
        app.startup_phase = crate::app::StartupPhase::Done;
        terminal.draw(|frame| render_swarm(frame, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let row0: String = (0..80)
            .map(|col| buf[(col, 0)].symbol().to_string())
            .collect();
        assert!(row0.contains("Chat") || row0.contains("Workflow"));
    }

    #[test]
    fn splash_screen_renders() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(true);
        terminal.draw(|frame| render_swarm(frame, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let content: String = (0..24)
            .flat_map(|row| (0..80).map(move |col| (col, row)))
            .map(|(col, row)| buf[(col, row)].symbol().to_string())
            .collect();
        assert!(
            content.contains("___ ___ ___"),
            "splash should render ECOCON ASCII art"
        );
    }

    #[test]
    fn slash_command_picker_renders() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(true);
        app.startup_phase = crate::app::StartupPhase::Done;
        app.input_buffer = "/ag".to_string();
        app.input_cursor = 3;
        app.maybe_open_picker();
        terminal.draw(|frame| render_swarm(frame, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let content: String = (0..24)
            .flat_map(|row| (0..80).map(move |col| (col, row)))
            .map(|(col, row)| buf[(col, row)].symbol().to_string())
            .collect();
        assert!(content.contains("/agent"), "picker should render /agent command");
    }

    #[test]
    fn slash_command_picker_nested_renders() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(true);
        app.startup_phase = crate::app::StartupPhase::Done;
        app.input_buffer = "/agent s".to_string();
        app.input_cursor = 8;
        app.maybe_open_picker();
        terminal.draw(|frame| render_swarm(frame, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let content: String = (0..24)
            .flat_map(|row| (0..80).map(move |col| (col, row)))
            .map(|(col, row)| buf[(col, row)].symbol().to_string())
            .collect();
        assert!(
            content.contains("/agent spawn"),
            "picker should render /agent spawn subcommand"
        );
    }

    #[test]
    fn slash_command_preview_renders() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(true);
        app.startup_phase = crate::app::StartupPhase::Done;
        app.input_buffer = "/agent".to_string();
        app.input_cursor = 6;
        app.maybe_open_picker();
        terminal.draw(|frame| render_swarm(frame, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let content: String = (0..24)
            .flat_map(|row| (0..80).map(move |col| (col, row)))
            .map(|(col, row)| buf[(col, row)].symbol().to_string())
            .collect();
        assert!(
            content.contains("Subcommands"),
            "preview should list subcommands for /agent"
        );
    }

    #[test]
    fn mention_picker_renders() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(true);
        app.startup_phase = crate::app::StartupPhase::Done;
        app.input_buffer = "@ki".to_string();
        app.input_cursor = 3;
        app.maybe_open_mention_picker();
        terminal.draw(|frame| render_swarm(frame, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let content: String = (0..24)
            .flat_map(|row| (0..80).map(move |col| (col, row)))
            .map(|(col, row)| buf[(col, row)].symbol().to_string())
            .collect();
        assert!(content.contains("@kimi"), "mention picker should render @kimi");
    }

    #[test]
    fn models_panel_renders() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(true);
        terminal
            .draw(|frame| {
                frame.render_widget(models_panel::render(&app), frame.area());
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let content: String = (0..24)
            .flat_map(|row| (0..80).map(move |col| (col, row)))
            .map(|(col, row)| buf[(col, row)].symbol().to_string())
            .collect();
        assert!(content.contains("Models & Providers"), "models panel should render title");
        assert!(content.contains("@kimi"), "models panel should list @kimi");
    }
}
