use crate::{
    IntoNodeSubscriptionCallback, NodeHandle, RclrsError, Subscription,
    SubscriptionOptions, SubscriptionState, WorkerCommands
};

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use rosidl_runtime_rs::Message;

use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

/// This wraps together a message receiver with a subscription. The messages are
/// produced by the subscription, and this struct ties their lifetimes together.
///
/// Use [`Deref`] to access the API of [`UnboundedReceiver`] to await messages
/// from the channel.
pub struct SubscriptionReceiver<T: Message> {
    receiver: UnboundedReceiver<T>,
    // Ensure the subscription remains alive for the duration of this receiver.
    #[allow(unused)]
    subscription: Subscription<T>,
}

impl<T: Message> Deref for SubscriptionReceiver<T> {
    type Target = UnboundedReceiver<T>;
    fn deref(&self) -> &Self::Target {
        &self.receiver
    }
}

impl<T: Message> DerefMut for SubscriptionReceiver<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.receiver
    }
}

impl<T: Message> SubscriptionReceiver<T> {
    pub(crate) fn create<'a>(
        options: impl Into<SubscriptionOptions<'a>>,
        node_handle: &Arc<NodeHandle>,
        commands: &Arc<WorkerCommands>,
    ) -> Result<Self, RclrsError> {
        let (sender, receiver) = unbounded_channel();

        let callback = (
            move |msg: T| {
                let _ = sender.send(msg);
            }
        ).into_node_subscription_callback();

        let subscription = SubscriptionState::create(
            options,
            callback,
            node_handle,
            commands,
        )?;

        Ok(Self { receiver, subscription })
    }
}

#[cfg(test)]
mod tests {

    use crate::*;
    use example_interfaces::msg::Int32;

    #[test]
    fn test_subscription_receiver() {
        let mut executor = Context::default().create_basic_executor();

        let node = executor
            .create_node(&format!("test_subscription_receiver_{}", line!()))
            .unwrap();

        let mut receiver = node.create_subscription_receiver::<Int32>("receiver_test_topic").unwrap();
        let publisher = node.create_publisher::<Int32>("receiver_test_topic").unwrap();

        for data in 0..10 {
            publisher.publish(Int32 { data }).unwrap();
        }

        let promise = executor.commands().run(async move {
            for expected_data in 0..10 {
                let msg = receiver.recv().await.unwrap();
                assert_eq!(msg.data, expected_data);
            }

            // We are not expecting any more messages, so let this task end and
            // the promise will resolve so the executor will stop.
        });

        executor.spin(SpinOptions::default().until_promise_resolved(promise));
    }
}
