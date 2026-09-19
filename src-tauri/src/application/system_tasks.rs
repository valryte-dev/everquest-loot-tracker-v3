use chrono::Utc;
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemTask {
    pub id: String,
    pub label: String,
    pub state: String,
    pub detail: String,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub started_at: String,
    pub finished_at: Option<String>,
}
#[derive(Clone, Default)]
pub struct TaskRegistry {
    tasks: Arc<Mutex<HashMap<String, SystemTask>>>,
}
impl TaskRegistry {
    pub fn start(&self, id: &str, label: &str, detail: &str, total: Option<u64>) {
        self.tasks.lock().unwrap_or_else(|e| e.into_inner()).insert(
            id.into(),
            SystemTask {
                id: id.into(),
                label: label.into(),
                state: "running".into(),
                detail: detail.into(),
                completed: total.map(|_| 0),
                total,
                started_at: Utc::now().to_rfc3339(),
                finished_at: None,
            },
        );
    }
    pub fn progress(&self, id: &str, detail: &str, completed: Option<u64>, total: Option<u64>) {
        if let Some(t) = self
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(id)
        {
            t.detail = detail.into();
            t.completed = completed;
            t.total = total;
        }
    }
    pub fn transition(
        &self,
        id: &str,
        label: &str,
        detail: &str,
        completed: Option<u64>,
        total: Option<u64>,
    ) {
        if let Some(task) = self
            .tasks
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get_mut(id)
        {
            task.label = label.into();
            task.detail = detail.into();
            task.completed = completed;
            task.total = total;
        }
    }
    pub fn finish(&self, id: &str, result: &Result<serde_json::Value, String>, detail: &str) {
        if let Some(t) = self
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(id)
        {
            t.state = if result.is_ok() {
                "completed"
            } else {
                "failed"
            }
            .into();
            t.detail = result
                .as_ref()
                .map(|_| detail.into())
                .unwrap_or_else(|e| e.clone());
            t.completed = t.total.or(t.completed);
            t.finished_at = Some(Utc::now().to_rfc3339());
        }
    }
    pub fn snapshot(&self) -> Vec<SystemTask> {
        let mut r: Vec<_> = self
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        r.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        r.truncate(20);
        r
    }
}
#[cfg(test)]
mod tests {
    use super::TaskRegistry;
    use serde_json::json;
    #[test]
    fn lifecycle() {
        let r = TaskRegistry::default();
        r.start("x", "Scan", "Start", Some(4));
        r.progress("x", "Half", Some(2), Some(4));
        r.finish("x", &Ok(json!({})), "Done");
        let s = r.snapshot();
        assert_eq!(
            (s[0].state.as_str(), s[0].completed),
            ("completed", Some(4))
        );
    }

    #[test]
    fn transition_changes_the_visible_phase_without_restarting_the_task() {
        let registry = TaskRegistry::default();
        registry.start("purge", "Backing up database", "Copying pages", Some(20));
        let started_at = registry.snapshot()[0].started_at.clone();

        registry.transition(
            "purge",
            "Purging old combat fights",
            "Deleting eligible fights",
            Some(0),
            Some(4000),
        );

        let task = registry.snapshot().remove(0);
        assert_eq!(task.label, "Purging old combat fights");
        assert_eq!(task.detail, "Deleting eligible fights");
        assert_eq!(task.completed, Some(0));
        assert_eq!(task.total, Some(4000));
        assert_eq!(task.started_at, started_at);
    }
}
