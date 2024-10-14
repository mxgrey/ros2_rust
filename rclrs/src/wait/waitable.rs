use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, MutexGuard,
};

use crate::{
    error::ToResult,
    rcl_bindings::*,
    RclrsError, GuardCondition, InnerGuardConditionHandle,
};

/// This provides the public API for executing an rcl primitive.
pub trait RclExecutable {
    /// Trigger this executable to run.
    fn execute(&mut self) -> Result<(), RclrsError>;

    /// Indicate what kind of executable this is.
    fn kind(&self) -> RclExecutableKind;

    /// Provide the handle for this executable
    fn handle(&self) -> RclExecutableHandle;
}

/// Enum to describe the kind of an executable.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum RclExecutableKind {
    Subscription,
    GuardCondition,
    Timer,
    Client,
    Service,
    Event,
}

/// Used by the wait set to obtain the handle of an executable.
pub enum RclExecutableHandle<'a> {
    Subscription(MutexGuard<'a, rcl_subscription_t>),
    GuardCondition(MutexGuard<'a, InnerGuardConditionHandle>),
    Timer(MutexGuard<'a, rcl_timer_t>),
    Client(MutexGuard<'a, rcl_client_t>),
    Service(MutexGuard<'a, rcl_service_t>),
    Event(MutexGuard<'a, rcl_event_t>),
}

impl<'a> std::fmt::Debug for RclExecutableHandle<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ptr = unsafe {
            match self {
                Self::Subscription(ptr) => ptr.impl_ as *const (),
                Self::GuardCondition(ptr) => match &**ptr {
                    InnerGuardConditionHandle::Owned(ptr) => ptr.impl_ as *const (),
                    InnerGuardConditionHandle::Unowned { handle, .. } => (*(*handle)).impl_ as *const (),
                },
                Self::Client(ptr) => ptr.impl_ as *const (),
                Self::Service(ptr) => ptr.impl_ as *const (),
                Self::Timer(ptr) => ptr.impl_ as *const (),
                Self::Event(ptr) => ptr.impl_ as *const (),
            }
        };

        f.write_fmt(format_args!("{:?}* {ptr:?}", self.kind()))
    }
}

impl<'a> RclExecutableHandle<'a> {
    /// Get the equivalent [`RclExecutableKind`] for this primitive.
    pub fn kind(&self) -> RclExecutableKind {
        match self {
            Self::Subscription(_) => RclExecutableKind::Subscription,
            Self::GuardCondition(_) => RclExecutableKind::GuardCondition,
            Self::Timer(_) => RclExecutableKind::Timer,
            Self::Client(_) => RclExecutableKind::Client,
            Self::Service(_) => RclExecutableKind::Service,
            Self::Event(_) => RclExecutableKind::Event,
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WaitableCount {
    pub subscriptions: usize,
    pub guard_conditions: usize,
    pub timers: usize,
    pub clients: usize,
    pub services: usize,
    pub events: usize,
}

impl WaitableCount {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn add(&mut self, kind: RclExecutableKind, count: usize) {
        match kind {
            RclExecutableKind::Subscription => self.subscriptions += count,
            RclExecutableKind::GuardCondition => self.guard_conditions += count,
            RclExecutableKind::Timer => self.timers += count,
            RclExecutableKind::Client => self.clients += count,
            RclExecutableKind::Service => self.services += count,
            RclExecutableKind::Event => self.events += count,
        }
    }

    pub(super) unsafe fn initialize(
        &self,
        rcl_context: &mut rcl_context_s,
    ) -> Result<rcl_wait_set_s, RclrsError> {
        unsafe {
            // SAFETY: Getting a zero-initialized value is always safe
            let mut rcl_wait_set = rcl_get_zero_initialized_wait_set();
            // SAFETY: We're passing in a zero-initialized wait set and a valid context.
            // There are no other preconditions.
            rcl_wait_set_init(
                &mut rcl_wait_set,
                self.subscriptions,
                self.guard_conditions,
                self.timers,
                self.clients,
                self.services,
                self.events,
                &mut *rcl_context,
                rcutils_get_default_allocator(),
            )
            .ok()?;
            Ok(rcl_wait_set)
        }
    }

    pub(super) unsafe fn resize(
        &self,
        rcl_wait_set: &mut rcl_wait_set_t,
    ) -> Result<(), RclrsError> {
        unsafe {
            rcl_wait_set_resize(
                rcl_wait_set,
                self.subscriptions,
                self.guard_conditions,
                self.timers,
                self.clients,
                self.services,
                self.events,
            )
        }
        .ok()
    }
}

#[must_use = "If you do not give the Waiter to a WaitSet then it will never be useful"]
pub struct Waitable {
    pub(super) executable: Box<dyn RclExecutable + Send + Sync>,
    in_use: Arc<AtomicBool>,
    index_in_wait_set: Option<usize>,
}

impl std::fmt::Debug for Waitable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waitable")
            .field("primitive", &self.executable.handle())
            .field("index", &self.index_in_wait_set)
            .field("in_use", &self.in_use.load(Ordering::Acquire))
            .finish()
    }
}

impl Waitable {
    pub fn new(
        waitable: Box<dyn RclExecutable + Send + Sync>,
        guard_condition: Option<Arc<GuardCondition>>,
    ) -> (Self, WaitableLifecycle) {
        let in_use = Arc::new(AtomicBool::new(true));
        let waiter = Self {
            executable: waitable,
            in_use: Arc::clone(&in_use),
            index_in_wait_set: None,
        };

        let lifecycle = WaitableLifecycle { in_use, guard_condition };
        (waiter, lifecycle)
    }

    pub(super) fn in_wait_set(&self) -> bool {
        self.index_in_wait_set.is_some()
    }

    pub(super) fn in_use(&self) -> bool {
        self.in_use.load(Ordering::Relaxed)
    }

    pub(super) fn is_ready(&self, wait_set: &rcl_wait_set_t) -> bool {
        self.index_in_wait_set.is_some_and(|index| {
            let ptr_is_null = unsafe {
                // SAFETY: Each field in the wait set is an array of points.
                // The dereferencing that we do is equivalent to obtaining the
                // element of the array at the index-th position.
                match self.executable.kind() {
                    RclExecutableKind::Subscription => wait_set.subscriptions.add(index).is_null(),
                    RclExecutableKind::GuardCondition => wait_set.guard_conditions.add(index).is_null(),
                    RclExecutableKind::Service => wait_set.services.add(index).is_null(),
                    RclExecutableKind::Client => wait_set.clients.add(index).is_null(),
                    RclExecutableKind::Timer => wait_set.timers.add(index).is_null(),
                    RclExecutableKind::Event => wait_set.events.add(index).is_null(),
                }
            };
            !ptr_is_null
        })
    }

    pub(super) fn add_to_wait_set(
        &mut self,
        wait_set: &mut rcl_wait_set_t,
    ) -> Result<(), RclrsError> {
        dbg!(&self);

        let mut index = 0;
        unsafe {
            // SAFETY: The Executable is responsible for maintaining the lifecycle
            // of the handle, so it is guaranteed to be valid here.
            match self.executable.handle() {
                RclExecutableHandle::Subscription(handle) => {
                    rcl_wait_set_add_subscription(wait_set, &*handle, &mut index)
                }
                RclExecutableHandle::GuardCondition(handle) => {
                    handle.use_handle(|handle| {
                        rcl_wait_set_add_guard_condition(wait_set, &*handle, &mut index)
                    })
                }
                RclExecutableHandle::Service(handle) => {
                    rcl_wait_set_add_service(wait_set, &*handle, &mut index)
                }
                RclExecutableHandle::Client(handle) => {
                    rcl_wait_set_add_client(wait_set, &*handle, &mut index)
                }
                RclExecutableHandle::Timer(handle) => {
                    rcl_wait_set_add_timer(wait_set, &*handle, &mut index)
                }
                RclExecutableHandle::Event(handle) => {
                    rcl_wait_set_add_event(wait_set, &*handle, &mut index)
                }
            }
        }
        .ok()?;

        self.index_in_wait_set = Some(index);
        Ok(())
    }
}

#[must_use = "If you do not hold onto the WaiterLifecycle, then its Waiter will be immediately dropped"]
pub struct WaitableLifecycle {
    in_use: Arc<AtomicBool>,
    guard_condition: Option<Arc<GuardCondition>>,
}

impl Drop for WaitableLifecycle {
    fn drop(&mut self) {
        self.in_use.store(false, Ordering::Release);
        if let Some(guard_condition) = &self.guard_condition {
            guard_condition.trigger();
        }
    }
}
