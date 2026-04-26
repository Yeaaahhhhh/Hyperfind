// File: crates/collector/src/scheduler.rs

use tracing::info;

#[derive(Debug, Clone)]
pub enum IndexTask {
    FullRebuild,
    ScanDirectory { path: String },
    ScanAllVolumes,
    IncrementalUpdate { events: Vec<ChangeEvent> },
}

#[derive(Debug, Clone)]
pub struct ChangeEvent {
    pub event_type: ChangeType,
    pub path: String,
}

#[derive(Debug, Clone)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
}

pub struct Scheduler {
    pending_tasks: Vec<IndexTask>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self { pending_tasks: Vec::new() }
    }

    pub fn enqueue(&mut self, task: IndexTask) {
        info!("Task enqueued: {:?}", task);
        self.pending_tasks.push(task);
    }

    pub fn next_task(&mut self) -> Option<IndexTask> {
        if self.pending_tasks.is_empty() { None }
        else { Some(self.pending_tasks.remove(0)) }
    }

    pub fn pending_count(&self) -> usize {
        self.pending_tasks.len()
    }

    pub fn clear(&mut self) {
        self.pending_tasks.clear();
    }
}

impl Default for Scheduler {
    fn default() -> Self { Self::new() }
}