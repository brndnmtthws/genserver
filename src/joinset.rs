//! Internal JoinSet implementation. This API is subject to change.

use std::sync::{Arc, RwLock};

use tokio::task::JoinHandle;

/// This JoinSet is used internally by the registry to clean up spawned tasks at
/// shutdown.
#[derive(Clone)]
pub struct JoinSet {
    handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
}

/// Provides a joinset similar to Tokio's `tokio::task::JoinSet`.
impl JoinSet {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn add(&mut self, joinhandle: JoinHandle<()>) {
        let mut handles = self.handles.write().unwrap();
        handles.push(joinhandle);
    }

    pub fn shutdown(&mut self) {
        let handles = self.handles.read().unwrap();
        handles.iter().for_each(|handle| handle.abort());
    }
}

impl Drop for JoinSet {
    fn drop(&mut self) {
        if Arc::strong_count(&self.handles) == 1 {
            self.shutdown();
        }
    }
}

impl Default for JoinSet {
    fn default() -> Self {
        Self::new()
    }
}
