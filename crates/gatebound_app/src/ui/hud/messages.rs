use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Default)]
pub struct HudMessages {
    pub entries: Vec<String>,
}

impl HudMessages {
    pub fn push(&mut self, message: String) {
        self.entries.push(message);
        if self.entries.len() > 8 {
            let drain_len = self.entries.len() - 8;
            self.entries.drain(0..drain_len);
        }
    }
}
