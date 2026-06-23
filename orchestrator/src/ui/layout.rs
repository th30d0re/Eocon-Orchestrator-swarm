use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwarmLayout {
    #[default]
    ChatWorkflow,
    ChatOnly,
    WorkflowOnly,
    Models,
}

impl SwarmLayout {
    pub fn next(self) -> Self {
        match self {
            SwarmLayout::ChatWorkflow => SwarmLayout::WorkflowOnly,
            SwarmLayout::WorkflowOnly => SwarmLayout::ChatOnly,
            SwarmLayout::ChatOnly => SwarmLayout::Models,
            SwarmLayout::Models => SwarmLayout::ChatWorkflow,
        }
    }
}

/// Compute main layout rectangles.
/// Returns `(chat_rect, workflow_rect, models_rect, status_rect, input_rect)`.
pub fn compute_swarm(
    area: Rect,
    layout: SwarmLayout,
    input_height: u16,
) -> (Option<Rect>, Option<Rect>, Option<Rect>, Rect, Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(input_height),
        ])
        .split(area);

    let top = main_chunks[0];
    let status_rect = main_chunks[1];
    let input_rect = main_chunks[2];

    let (chat, workflow, models) = match layout {
        SwarmLayout::ChatWorkflow => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Min(0)])
                .split(top);
            (Some(chunks[0]), Some(chunks[1]), None)
        }
        SwarmLayout::ChatOnly => (Some(top), None, None),
        SwarmLayout::WorkflowOnly => (None, Some(top), None),
        SwarmLayout::Models => (None, None, Some(top)),
    };

    (chat, workflow, models, status_rect, input_rect)
}
