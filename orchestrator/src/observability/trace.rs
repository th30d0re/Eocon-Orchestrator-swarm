use std::collections::HashMap;
use std::time::Instant;

pub struct TraceSpan {
    pub id: String,
    pub name: String,
    pub agent_id: Option<String>,
    pub started_at: Instant,
    pub finished_at: Option<Instant>,
    pub attributes: HashMap<String, String>,
    pub children: Vec<TraceSpan>,
}

impl TraceSpan {
    pub fn new(id: String, name: String, agent_id: Option<String>) -> Self {
        Self {
            id,
            name,
            agent_id,
            started_at: Instant::now(),
            finished_at: None,
            attributes: HashMap::new(),
            children: Vec::new(),
        }
    }

    pub fn set_attr(&mut self, key: &str, value: String) {
        self.attributes.insert(key.to_string(), value);
    }

    pub fn start_child(
        &mut self,
        id: String,
        name: String,
        agent_id: Option<String>,
    ) -> &mut TraceSpan {
        self.children.push(TraceSpan::new(id, name, agent_id));
        self.children.last_mut().unwrap()
    }

    pub fn finish(&mut self) {
        self.finished_at = Some(Instant::now());
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self).unwrap_or_else(|_| "{}".to_string())
    }
}

impl serde::Serialize for TraceSpan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("TraceSpan", 6)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("agent_id", &self.agent_id)?;
        state.serialize_field("started_at_ms", &self.started_at.elapsed().as_millis())?;
        state.serialize_field(
            "finished_at_ms",
            &self
                .finished_at
                .map(|_| self.started_at.elapsed().as_millis()),
        )?;
        state.serialize_field("attributes", &self.attributes)?;
        state.serialize_field("children", &self.children)?;
        state.end()
    }
}
