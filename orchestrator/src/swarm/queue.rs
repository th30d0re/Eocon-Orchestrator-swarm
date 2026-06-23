#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub claimed_by: Option<String>,
    pub source_agent: Option<String>,
}

#[derive(Debug, Default)]
pub struct TaskQueue {
    tasks: Vec<Task>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn claim_next(&mut self, agent_id: &str) -> Option<Task> {
        if let Some(idx) = self.tasks.iter().position(|t| t.claimed_by.is_none()) {
            self.tasks[idx].claimed_by = Some(agent_id.to_string());
            Some(self.tasks[idx].clone())
        } else {
            None
        }
    }

    pub fn pending(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.claimed_by.is_none()).collect()
    }

    pub fn claimed_by(&self, agent_id: &str) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.claimed_by.as_deref() == Some(agent_id))
            .collect()
    }

    pub fn all(&self) -> &[Task] {
        &self.tasks
    }

    pub fn complete(&mut self, task_id: &str) {
        self.tasks.retain(|t| t.id != task_id);
    }
}
