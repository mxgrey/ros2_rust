use rosidl_runtime_rs::Action;

use futures::channel::{oneshot, mpsc::UnboundedSender};

use std::{
    future::Future,
    sync::Arc
};

pub enum ActionGoalDecision<A: Action> {
    Accepted(ActionExecution<A>),
    Rejected,
}

pub struct ActionGoalHandle<A: Action> {
    goal: Arc<A::Goal>,
    // TODO: Auto-reject the goal if this is dropped without a decision being made.
    decision_made: bool,
}

impl<A: Action> ActionGoalHandle<A> {
    pub fn accept<F>(
        self,
        execute: impl FnOnce(ActionExecutionHandle<A>) -> F,
    ) -> ActionGoalDecision<A>
    where
        F: Future<Output = ActionResult<A>>
    {

    }
}

pub struct ActionAcceptedQueue<A: Action> {
    _ignore: std::marker::PhantomData<fn(A)>,
}

impl<A: Action> ActionAcceptedQueue<A> {
    /// Accept the action, but put it in a queue
    pub fn accept(
        self: Arc<Self>,
        handle: ActionGoalHandle<A>,
        f: impl FnOnce(oneshot::Receiver<ActionQueueResult<A>>) -> ActionExecution<A>,
    ) -> ActionGoalDecision<A> {

    }
}

pub struct ActionExecutionHandle<A: Action> {
    feedback: UnboundedSender<A::Feedback>,

}

pub struct ActionExecution<A: Action> {
    result: oneshot::Receiver<ActionResult<A>>,
}

pub enum ActionQueueResult<A: Action> {
    Execute(ExecuteAction<A>),
    Cancel(ExecuteAction<A>)
}

pub enum ActionResult<A: Action> {
    Succeeded(A::Result),
    Canceled(A::Result),
    Aborted,
}

pub struct ExecuteAction<A: Action> {
    _ignore: std::marker::PhantomData<A>,
}

impl<A: Action> ExecuteAction<A> {
    pub fn begin<F>(
        self,
        f: impl FnOnce(ActionExecutionHandle<A>) -> F,
    ) -> ActionExecution<A>
    where
        F: Future<Output = ActionResult<A>>,
    {

    }
}
