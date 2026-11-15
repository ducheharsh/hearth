//! In-memory activity feed. The UI polls `/api/events?after=<seq>` to show toasts
//! and to let the pets "say" things.

use std::collections::VecDeque;

use serde::Serialize;

const KEEP: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub seq: u64,
    pub at: u64,
    pub level: Level,
    /// Area: `storage`, `apps`, `gateway`, `cloudflare`, `tailscale`, `discover`.
    pub source: &'static str,
    pub title: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct EventLog {
    next: u64,
    items: VecDeque<Event>,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl EventLog {
    pub fn push(
        &mut self,
        level: Level,
        source: &'static str,
        title: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.next += 1;
        let e = Event {
            seq: self.next,
            at: now(),
            level,
            source,
            title: title.into(),
            message: message.into(),
        };
        tracing::info!(source, level = ?e.level, title = %e.title, "{}", e.message);
        if self.items.len() == KEEP {
            self.items.pop_front();
        }
        self.items.push_back(e);
    }

    #[must_use]
    pub fn after(&self, seq: u64) -> Vec<Event> {
        self.items.iter().filter(|e| e.seq > seq).cloned().collect()
    }

    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_and_ordered() {
        let mut log = EventLog::default();
        for i in 0..250 {
            log.push(Level::Info, "test", format!("e{i}"), "");
        }
        let all = log.after(0);
        assert_eq!(all.len(), KEEP);
        assert_eq!(all.last().map(|e| e.seq), Some(250));
        assert_eq!(log.after(248).len(), 2);
    }
}
