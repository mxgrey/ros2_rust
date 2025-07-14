use rosidl_runtime_rs::Action;

use futures::{
    StreamExt,
    channel::{oneshot, mpsc::{Sender, channel}},
    pin_mut,
};

use std::{
    future::Future,
    sync::Arc,
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
        callback: impl FnMut(ActionGoalHandle<A>) -> F,
    )
    where
        F: Future<Output = ActionGoalDecision<A>>,
    {

    }
}



pub enum ActionGoalDecision<A: Action> {
    Accepted(ActionExecution<A>),
    Rejected,
}

pub struct ActionGoalHandle<A: Action> {
    goal: Arc<A::Goal>,
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
    pub fn accept<F>(
        self: Arc<Self>,
        handle: ActionGoalHandle<A>,
        f: impl FnOnce(oneshot::Receiver<ActionQueueResult<A>>) -> F,
    ) -> ActionGoalDecision<A>
    where
        F: Future<Output = ActionExecution<A>>,
    {

    }
}

pub struct ActionExecutionHandle<A: Action> {
    goal: Arc<A::Goal>,
    feedback: Sender<A::Feedback>,
    canceled: watch::Receiver<bool>,
}

impl<A: Action> ActionExecutionHandle<A> {
    pub async fn recv_cancel(&self) -> () {
        self.canceled.clone().wait_for(|canceled| *canceled);
    }

    pub async fn until_canceled<F: Future>(&self, f: F) -> Result<F::Output, F> {
        let canceled = self.canceled.clone().wait_for(|canceled| *canceled);
        pin_mut!(f);
        pin_mut!(canceled);
        match futures::future::select(f, canceled).await {
            futures::future::Either::Left((result, _)) => Ok(result),
            futures::future::Either::Right((_, f)) => Err(f.into_inner()),
        }
    }
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

    use example_interfaces::action::{Fibonacci, Fibonacci_Result};

    fn test_create_action(node: &NodeState) {
        node.create_action(
            ActionOptions::default(),
            accept_fibonacci_action,
        );
    }

    async fn accept_fibonacci_action(
        handle: ActionGoalHandle<Fibonacci>
    ) -> ActionGoalDecision<Fibonacci> {
        handle.accept(run_fibonacci_action)
    }

    async fn run_fibonacci_action(
        handle: ActionExecutionHandle<Fibonacci>,
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
            match handle.until_canceled(receiver.next()).await {
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
        ready: oneshot::Receiver<ActionQueueResult<Fibonacci>>,
    ) -> ActionExecution<Fibonacci> {
        match ready.await.unwrap() {
            ActionQueueResult::Execute(execute) => {
                execute.begin(run_fibonacci_action)
            }
            ActionQueueResult::Cancel(cancellation) => {
                cancellation.canceled(Fibonacci_Result::default())
            }
        }
    }
}
