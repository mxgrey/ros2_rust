use crate::error::ToResult;
use crate::qos::QoSProfile;
use crate::{RclReturnCode, SubscriberErrorCode, rcl_bindings::*, to_rcl_result};
use crate::{Node, NodeHandle};
use rclrs_msg_utilities::traits::{Message, MessageDefinition};
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::borrow::Borrow;
use core::marker::PhantomData;
use cstr_core::CString;

#[cfg(not(feature = "std"))]
use spin::{Mutex, MutexGuard};

#[cfg(feature = "std")]
use parking_lot::{Mutex, MutexGuard};

pub struct SubscriptionHandle {
    handle: Mutex<rcl_subscription_t>,
    node_handle: Arc<NodeHandle>,
}

impl SubscriptionHandle {
    fn node_handle(&self) -> &NodeHandle {
        self.node_handle.borrow()
    }

    pub fn get_mut(&mut self) -> &mut rcl_subscription_t {
        self.handle.get_mut()
    }

    pub fn lock(&self) -> MutexGuard<rcl_subscription_t> {
        self.handle.lock()
    }

    pub fn try_lock(&self) -> Option<MutexGuard<rcl_subscription_t>> {
        self.handle.try_lock()
    }
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        let handle = self.handle.get_mut();
        let node_handle = &mut *self.node_handle.lock();
        unsafe {
            rcl_subscription_fini(handle as *mut _, node_handle as *mut _);
        }
    }
}

/// Trait to be implemented by concrete Subscriber structs
/// See [`Subscription<T>`] for an example
pub trait SubscriptionBase {
    fn handle(&self) -> &SubscriptionHandle;
    fn create_message(&self) -> Box<dyn Message>;
    fn callback_fn(&self, message: Box<dyn Message>) -> ();

    /// Ask RMW for the data
    ///
    /// +-------------+
    /// | rclrs::take |
    /// +------+------+
    ///        |
    ///        |
    /// +------v------+
    /// |  rcl_take   |
    /// +------+------+
    ///        |
    ///        |
    /// +------v------+
    /// |  rmw_take   |
    /// +-------------+
    fn take(&self, message: &mut dyn Message) -> Result<bool, RclReturnCode> {
        let handle = &mut *self.handle().lock();
        let message_handle = message.get_native_message();

        let result = unsafe {
            rcl_take(
                handle as *const _,
                message_handle as *mut _,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };

        let result = match to_rcl_result(result) {
            Ok(()) => {
                message.read_handle(message_handle);
                Ok(true)
            }
            Err(RclReturnCode::SubscriberError(SubscriberErrorCode::SubscriptionTakeFailed)) => {
                Ok(false)
            }
            Err(error) => Err(error.into()),
        };

        message.destroy_native_message(message_handle);

        result
    }
}

/// Main class responsible for subscribing to topics and receiving data over IPC in ROS
pub struct Subscription<T>
where
    T: Message,
{
    pub handle: Arc<SubscriptionHandle>,
    // The callback's lifetime should last as long as we need it to
    pub callback: Mutex<Box<dyn FnMut(&T) + 'static>>,
    message: PhantomData<T>,
}

impl<T> Subscription<T>
where
    T: Message,
{
    pub fn new<F>(
        node: &Node,
        topic: &str,
        qos: QoSProfile,
        callback: F,
    ) -> Result<Self, RclReturnCode>
    where
        T: MessageDefinition<T>,
        F: FnMut(&T) + Sized + 'static,
    {
        let mut subscription_handle = unsafe { rcl_get_zero_initialized_subscription() };
        let type_support = T::get_type_support() as *const rosidl_message_type_support_t;
        let topic_c_string = CString::new(topic).unwrap();
        let node_handle = &mut *node.handle.lock();

        unsafe {
            let mut subscription_options = rcl_subscription_get_default_options();
            subscription_options.qos = qos.into();
            rcl_subscription_init(
                &mut subscription_handle as *mut _,
                node_handle as *mut _,
                type_support,
                topic_c_string.as_ptr(),
                &subscription_options as *const _,
            )
            .ok()?;
        }

        let handle = Arc::new(SubscriptionHandle {
            handle: Mutex::new(subscription_handle),
            node_handle: node.handle.clone(),
        });

        Ok(Self {
            handle,
            callback: Mutex::new(Box::new(callback)),
            message: PhantomData,
        })
    }

    pub fn take(&self, message: &mut T) -> Result<(), RclReturnCode> {
        let handle = &mut *self.handle.lock();
        let message_handle = message.get_native_message();
        let ret = unsafe {
            rcl_take(
                handle as *const _,
                message_handle as *mut _,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };
        message.read_handle(message_handle);
        message.destroy_native_message(message_handle);
        ret.ok()
    }

    fn callback_ext(
        &self,
        message: Box<dyn Message>,
    ) -> Result<(), RclReturnCode> {
        let msg = message
            .downcast_ref()
            .unwrap();
        (&mut *self.callback.lock())(msg);
        Ok(())
    }
}

impl<T> SubscriptionBase for Subscription<T>
where
    T: MessageDefinition<T> + core::default::Default,
{
    fn handle(&self) -> &SubscriptionHandle {
        self.handle.borrow()
    }

    fn create_message(&self) -> Box<dyn Message> {
        Box::new(T::default())
    }

    fn callback_fn(&self, message: Box<dyn Message>) {
        self.callback_ext(message);
    }
}
