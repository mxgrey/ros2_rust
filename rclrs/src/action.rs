use rosidl_runtime_rs::Action;

use futures::{
    StreamExt,
    channel::{oneshot, mpsc::{Sender, channel}},
    pin_mut,
};

use std::{
    future::Future,
    sync::Arc,
    pin::pin,
};

use tokio::sync::watch;

use crate::NodeState;

pub struct ActionOptions {
    pub feedback_channel_bound: usize,
}

impl Default for ActionOptions {
    fn default() -> Self {
        Self {
            feedback_channel_bound: 16,
        }
    }
}

impl NodeState {
    pub fn create_action<A: Action, F>(
        &self,
        options: ActionOptions,
        callback: impl FnMut(GoalHandle<A>) -> F,
    )
    where
        F: Future<Output = GoalDecision<A>>,
    {

    }
}



pub enum GoalDecision<A: Action> {
    Accept(ActionExecution<A>),
    Reject,
}

pub struct GoalHandle<A: Action> {
    goal: Arc<A::Goal>,
}

impl<A: Action> GoalHandle<A> {
    pub fn accept_and_execute<F>(
        self,
        execute: impl FnOnce(ActionHandle<A>) -> F,
    ) -> GoalDecision<A>
    where
        F: Future<Output = ActionResult<A>>
    {

    }

    pub fn accept_and_defer(
        self,
        defer: impl FnOnce(AcceptedAction<A>) -> F,
    ) -> GoalDecision<A>
    where
        F: Future<Output = ActionExecution<A>>,
    {

    }
}

pub struct ActionQueue<A: Action> {
    _ignore: std::marker::PhantomData<fn(A)>,
}

impl<A: Action> ActionQueue<A> {
    /// Accept the action, but put it in a queue
    pub fn accept<F>(
        self: Arc<Self>,
        handle: GoalHandle<A>,
        f: impl FnOnce(ActionQueueHandle<A>) -> F,
    ) -> GoalDecision<A>
    where
        F: Future<Output = ActionExecution<A>>,
    {

    }
}

pub struct ActionQueueHandle<A: Action> {
    ready: oneshot::Receiver<ActionQueueResult<A>>,
}

pub type UntilReadyResult<A, F> = Result<
    (<F as Future>::Output, ActionQueueHandle<A>),
    (ActionQueueResult<A>, F)
>;

impl<A: Action> ActionQueueHandle<A> {
    pub async fn ready(self) -> ActionQueueResult<A> {
        self.ready.await
        .unwrap_or_else(|_| ActionQueueResult::Cancel(CancelingAction { _ignore: Default::default() }))
    }

    pub fn until_ready<F>(self, f: F) -> UntilReadyResult<A, F>
    where
        F: Future,
    {

    }

    pub fn abort(self) -> ActionExecution<A> {

    }
}

pub struct ActionHandle<A: Action> {
    goal: Arc<A::Goal>,
    feedback: Sender<A::Feedback>,
    cancellation: CancellationWatcher<A>,
}

pub type UntilCancelledResult<A, F> = Result<
    <F as Future>::Output,
    (Cancellation<A>, F),
>;

impl<A: Action> ActionHandle<A> {
    pub async fn recv_cancel(&self) -> Cancellation<A> {
        self.cancellation.recv_cancel().await
    }

    pub async fn until_cancelled<F: Future + Unpin>(&self, f: F) -> UntilCancelledResult<A, F> {
        self.cancellation.until_cancelled(f).await
    }
}

#[derive(Clone)]
pub struct CancellationWatcher<A: Action> {
    watcher: watch::Receiver<bool>,
    _ignore: std::marker::PhantomData<A>,
}

impl<A: Action> CancellationWatcher<A> {
    pub async fn recv_cancel(&self) -> Cancellation<A> {
        self.watcher.clone().wait_for(|canceled| *canceled);
        Cancellation { _ignore: Default::default() }
    }

    pub async fn until_cancelled<F: Future + Unpin>(
        &self,
        f: F,
    ) -> UntilCancelledResult<A, F> {
        let mut watcher = self.watcher.clone();
        let cancelled = watcher.wait_for(|canceled| *canceled);
        pin_mut!(cancelled);
        match futures::future::select(f, cancelled).await {
            futures::future::Either::Left((result, _)) => Ok(result),
            futures::future::Either::Right((_, f)) => Err((
                Cancellation { _ignore: Default::default() },
                f
            )),
        }
    }
}

pub struct Cancellation<A> {
    _ignore: std::marker::PhantomData<A>,
}

impl<A: Action> Cancellation<A> {
    pub fn canceled(result: A::Result) -> ActionResult<A> {

    }
}

pub struct ActionExecution<A: Action> {
    result: oneshot::Receiver<ActionResult<A>>,
}

pub enum ActionQueueResult<A: Action> {
    Execute(AcceptedAction<A>),
    Cancel(CancelingAction<A>),
}

pub struct ActionResult<A: Action> {
    inner: InnerActionResult<A>,
}

enum InnerActionResult<A: Action> {
    Succeeded(A::Result),
    Canceled(A::Result),
    Aborted,
}

pub struct AcceptedAction<A: Action> {
    _ignore: std::marker::PhantomData<A>,
}

impl<A: Action> AcceptedAction<A> {
    pub fn execute<F>(
        self,
        f: impl FnOnce(ActionHandle<A>) -> F,
    ) -> ActionExecution<A>
    where
        F: Future<Output = ActionResult<A>>,
    {

    }
}

pub struct CancelingAction<A: Action> {
    _ignore: std::marker::PhantomData<fn(A)>,
}

impl<A: Action> CancelingAction<A> {
    /// Enter the Canceling state and do whatever work is needed to respond to
    /// the cancellation request. This still allows you to respond with Succeed
    /// or Abort instead of Canceled, but the recommended final transition is
    /// Canceled using [`Cancellation::canceled`].
    pub fn begin_canceling<F>(
        self,
        f: impl FnOnce(ActionHandle<A>, Cancellation<A>),
    ) -> ActionExecution<A> {

    }

    /// Immediately report the action as canceled. This can be used if no work
    /// needs to be done to cancel the action.
    pub fn canceled(self, result: A::Result) -> ActionExecution<A> {

    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use futures::{
        StreamExt,
        channel::{
            oneshot,
            mpsc::channel,
        }
    };

    use example_interfaces::action::{Fibonacci, Fibonacci_Result, Fibonacci_Feedback};

    fn test_create_action(node: &NodeState) {
        node.create_action(
            ActionOptions::default(),
            accept_fibonacci_action,
        );
    }

    async fn accept_fibonacci_action(
        handle: GoalHandle<Fibonacci>
    ) -> GoalDecision<Fibonacci> {
        handle.accept_and_execute(run_fibonacci_action)
    }

    async fn run_fibonacci_action(
        handle: ActionHandle<Fibonacci>,
    ) -> ActionResult<Fibonacci> {
        let order = handle.goal.order;
        let mut sequence = Vec::new();
        let (mut sender, mut receiver) = channel(16);
        std::thread::spawn(move || {
            let mut i_prev2 = 0;
            let mut i_prev = 1;
            for _ in 2..=order {
                std::thread::sleep(std::time::Duration::from_secs(1));

                let next_value = i_prev2 + i_prev;
                sender.try_send(next_value);

                i_prev2 = i_prev;
                i_prev = next_value;
            }
        });

        loop {
            match handle.until_cancelled(receiver.next()).await {
                Ok(Some(next)) => {
                    sequence.push(next);

                }
                Ok(None) => {
                    // The sequence has finished
                    break;
                }
                Err(_) => return ActionResult::Canceled(Fibonacci_Result { sequence }),
            }
        }

        ActionResult::Succeeded(Fibonacci_Result { sequence })
    }

    async fn wait_to_run_fibonacci(
        handle: ActionQueueHandle<Fibonacci>,
    ) -> ActionExecution<Fibonacci> {
        match handle.ready().await {
            ActionQueueResult::Execute(accepted) => {
                accepted.execute(run_fibonacci_action)
            }
            ActionQueueResult::Cancel(cancellation) => {
                cancellation.canceled(Fibonacci_Result::default())
            }
        }
    }
}
