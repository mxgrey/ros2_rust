use std::{
    sync::{Arc, Mutex},
    collections::VecDeque,
};

use crate::{
    GuardCondition, ContextHandle, Waitable,
};

/// This provides the API for executing anything that needs to be executed on
/// the same thread as the wait set.
///
/// Pass an `Arc<Executable>` into a [`WaitSetQueue`] in order to
///
/// This is only for user- or rclrs-defined executables that are not rcl
/// primitives. For rcl primitives, use [`RclExecutable`][crate::RclExecutable].
pub trait Executable: Send + Sync {
    fn execute(&self);
}

pub struct WaitSetStream {
    pool: Arc<WaitSetStreamPool>,
    guard_condition: GuardCondition,
}

impl WaitSetStream {
    pub fn new(context: &Arc<ContextHandle>) -> (Self, Waitable) {
        let pool = Arc::new(WaitSetStreamPool::default());
        let pool_inner = Arc::clone(&pool);
        let callback = move || {
            println!(" -------------------- flushing pool --------------------- ");
            pool_inner.flush();
        };

        let (guard_condition, waitable) = GuardCondition::new(context, Some(Box::new(callback)));
        let stream = Self { pool, guard_condition };
        (stream, waitable)
    }

    /// Wake up the wait set and run this executable.
    pub fn send(&self, executable: Arc<dyn Executable>) {
        self.pool.queue.lock().unwrap().push_back(executable);
        println!(" ------------------------ triggering guard condition ---------------------- ");
        self.guard_condition.trigger().unwrap();
    }
}

/// This is where the executables will be stored until they are executed. It is
/// also the item that implements the callback of the guard condition for the
/// [`WaitSetStream`].
#[derive(Default)]
struct WaitSetStreamPool {
    queue: Mutex<VecDeque<Arc<dyn Executable>>>,
}

impl WaitSetStreamPool {
    fn flush(&self) {
        for executable in self.queue.lock().unwrap().drain(..) {
            println!(" ------------------- executing stream pool ------------- ");
            executable.execute();
        }
    }
}
