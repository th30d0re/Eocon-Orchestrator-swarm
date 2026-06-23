use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwarmLayout {
    #[default]
    ChatWorkflow,
    ChatOnly,
    WorkflowOnly,
}

impl SwarmLayout {
    pub fn next(self) -> Self {
        match self {
            SwarmLayout::ChatWorkflow => SwarmLayout::WorkflowOnly,
            SwarmLayout::WorkflowOnly => SwarmLayout::ChatOnly,
            SwarmLayout::ChatOnly => SwarmLayout::ChatWorkflow,
        }
    }
}

pub fn compute_swarm(
    area: Rect,
    layout: SwarmLayout,
) -> (Option<Rect>, Option<Rect>, Rect, Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let top = main_chunks[0];
    let status_rect = main_chunks[1];
    let input_rect = main_chunks[2];

    let (chat, workflow) = match layout {
        SwarmLayout::ChatWorkflow => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Min(0)])
                .split(top);
            (Some(chunks[0]), Some(chunks[1]))
        }
        SwarmLayout::ChatOnly => (Some(top), None),
        SwarmLayout::WorkflowOnly => (None, Some(top)),
    };

    (chat, workflow, status_rect, input_rect)
}
