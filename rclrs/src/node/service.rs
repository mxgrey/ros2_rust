// Copyright 2021 DCS Corporation, All Rights Reserved.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at

//     http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// DISTRIBUTION A. Approved for public release; distribution unlimited.
// OPSEC #4584.

use crate::error::{RclReturnCode, ToResult};
use crate::qos::QoSProfile;
use crate::rcl_bindings::*;
use crate::rcl_bindings::rmw_service_info_t;
use crate::{Node, NodeHandle};
use alloc::boxed::Box;
use alloc::sync::Arc;
use cstr_core::CString;
use rclrs_msg_utilities::traits::{MessageDefinition, Message};
use core::borrow::Borrow;
use core::marker::PhantomData;

#[cfg(not(feature = "std"))]
use spin::{Mutex, MutexGuard};

#[cfg(feature = "std")]
use parking_lot::{Mutex, MutexGuard};

pub struct ServiceHandle {
    handle: Mutex<rcl_service_t>,
    node_handle: Arc<NodeHandle>,
}

impl ServiceHandle {
    fn node_handle(&self) -> &NodeHandle {
        self.node_handle.borrow()
    }

    fn get_mut(&mut self) -> &mut rcl_service_t {
        self.handle.get_mut()
    }

    fn lock(&self) -> MutexGuard<rcl_service_t> {
        self.handle.lock()
    }

    fn try_lock(&self) -> Option<MutexGuard<rcl_service_t>> {
        self.handle.try_lock()
    }
}

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        let handle = self.handle.get_mut();
        let node_handle = &mut *self.node_handle.lock();
        unsafe {
            rcl_service_fini(handle as *mut _, node_handle as *mut _);
        }
    }
}

pub trait ServiceBase {
    fn handle(&self) -> &ServiceHandle;

    fn create_message(&self) -> Box<dyn Message>;

    fn callback_fn(&self, message: Box<dyn Message>) -> Result<(), RclReturnCode>;

}

pub struct Service<T>
where
    T: MessageDefinition<T>,
{
    pub handle: Arc<ServiceHandle>,
    // The callback's lifetime should last as long as we need it to
    pub callback: Mutex<Box<dyn FnMut(&T) + 'static>>,
    message: PhantomData<T>,
}

impl<T> Service<T>
where
    T: MessageDefinition<T>,
{
    /// Creates and initializes a non-action-based service.
    /// 
    /// Underlying _RCL_ information:
    /// 
    /// |Attribute|Adherence|
    /// |---------|---------|
    /// |Allocates Memory|Yes|
    /// |Thread-Safe|No|
    /// |Uses Atomics|No|
    /// |Lock-Free|Yes|
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
        let mut service_handle = unsafe { rcl_get_zero_initialized_service() };
        let type_support = T::get_type_support() as *const rosidl_service_type_support_t;
        let topic_c_string = CString::new(topic).unwrap();  // If the topic name is unrepresentable as a c-string, RCL will be unable to use it
        let node_handle = &mut *node.handle.lock();

        unsafe {
            let mut service_options = rcl_service_get_default_options();
            service_options.qos = qos.into();

            rcl_service_init(
                &mut service_handle as *mut _,
                node_handle as *mut _,
                type_support,
                topic_c_string.as_ptr(),
                &service_options as *const _,
            )
            .ok()?;
        }
        
        let handle = Arc::new(ServiceHandle {
            handle: Mutex::new(service_handle),
            node_handle: node.handle.clone(),
        });

        Ok(Self {
            handle,
            callback: Mutex::new(Box::new(callback)),
            message: PhantomData,
        })
    }

    pub fn take_request_with_info(&self,  request_header: &mut rmw_service_info_t, request: &mut T) -> Result<(), RclReturnCode> {
        let handle = &mut *self.handle.lock();
        let request_handle = request.get_native_message();
        let ret = unsafe {
            rcl_take_request_with_info(
                handle as *const _,
                request_header as *mut _,
                request_handle as *mut _)
        };
        request.read_handle(request_handle);
        request.destroy_native_message(request_handle);
        ret.ok()
    }

    pub fn take_request(&self, request: &mut T) -> Result<(), RclReturnCode> {
        let handle = &mut *self.handle.lock();
        let request_handle = request.get_native_message();
        let ret = unsafe {
            rcl_take_request(
                handle as *const _,
                core::ptr::null_mut(),
                request_handle as *mut _,
            )
        };
        request.read_handle(request_handle);
        request.destroy_native_message(request_handle);
        ret.ok()
    }

    fn callback_ext(
        &self,
        message: Box<dyn Message>,
    ) -> Result<(), RclReturnCode> {
        let msg = message
            .downcast_ref()
            .ok_or(RclReturnCode::Error)?;
        (&mut *self.callback.lock())(msg);
        Ok(())
    }

}

impl<T> ServiceBase for Service<T>
where
    T: MessageDefinition<T> + core::default::Default,
{
    fn handle(&self) -> &ServiceHandle {
        self.handle.borrow()
    }

    fn create_message(&self) -> Box<dyn Message> {
        Box::new(T::default())
    }

    fn callback_fn(&self, message: Box<dyn Message>) -> Result<(), RclReturnCode> {
        self.callback_ext(message)
    }
}
