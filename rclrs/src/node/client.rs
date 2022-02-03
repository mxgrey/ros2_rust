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

pub struct ClientHandle {
    handle: Mutex<rcl_client_t>,
    node_handle: Arc<NodeHandle>,
}

impl ClientHandle {
    fn node_handle(&self) -> &NodeHandle {
        self.node_handle.borrow()
    }

    fn get_mut(&mut self) -> &mut rcl_client_t {
        self.handle.get_mut()
    }

    fn lock(&self) -> MutexGuard<rcl_client_t> {
        self.handle.lock()
    }

    fn try_lock(&self) -> Option<MutexGuard<rcl_client_t>> {
        self.handle.try_lock()
    }
}

impl Drop for ClientHandle {
    fn drop(&mut self) {
        let handle = self.handle.get_mut();
        let node_handle = &mut *self.node_handle.lock();
        unsafe {
            rcl_client_fini(handle as *mut _, node_handle as *mut _);
        }
    }
}

pub trait ClientBase {
    fn handle(&self) -> &ClientHandle;

    fn create_message(&self) -> Box<dyn Message>;

    fn send_request(&self, request: Box<dyn Message>) -> Result<i64, RclReturnCode>;
}

pub struct Client<T>
where
    T: MessageDefinition<T>
{
    pub handle: Arc<ClientHandle>,
    message: PhantomData<T>,
}

impl<T> Client<T>
where
    T: MessageDefinition<T>,
{
    /// Creates and initializes a non-action-based client.
    /// 
    /// Underlying _RCL_ information:
    /// |Attribute|Adherence|
    /// |---------|---------|
    /// |Allocates Memory|Yes|
    /// |Thread-Safe|No|
    /// |Uses Atomics|No|
    /// |Lock-Free|Yes|
    pub fn new(
        node: &Node,
        topic: &str,
        qos: QoSProfile
    ) -> Result<Self, RclReturnCode>
    {
        let mut client_handle = unsafe { rcl_get_zero_initialized_client() };
        let type_support = T::get_type_support() as *const rosidl_service_type_support_t;
        let topic_c_string = CString::new(topic).unwrap();  // If the topic name is unrepresentable as a c-string, RCL will be unable to use it
        let node_handle = &mut *node.handle.lock();

        unsafe {
            let mut client_options = rcl_client_get_default_options();
            client_options.qos = qos.into();

            rcl_client_init(
                &mut client_handle as *mut _,
                node_handle as *mut _,
                type_support,
                topic_c_string.as_ptr(),
                &client_options as *const _,
            )
            .ok()?;
        }
        let handle = Arc::new(ClientHandle {
            handle: Mutex::new(client_handle),
            node_handle: node.handle.clone(),
        });

        Ok(Self {
            handle,
            message: PhantomData,
        })
    }

    fn service_is_ready(&self) -> Result<bool, RclReturnCode> {
        let node_handle = & *self.handle.node_handle.lock();
        let client_handle = & *self.handle.handle.lock();
        let mut is_ready = false;
        unsafe { rcl_service_server_is_available(
            node_handle as *const _,
            client_handle as *const _,
            &mut is_ready as *mut _,
        )}.ok()?;
        Ok(is_ready)
    }

    fn take_response(&self, response: &mut T) -> Result<(), RclReturnCode> {
        let handle = & *self.handle.lock();
        let response_handle = response.get_native_message();
        let ret = unsafe {
            rcl_take_response(
                handle as *const _,
                core::ptr::null_mut(),
                response_handle as *mut _,
            )
        };
        response.read_handle(response_handle);
        response.destroy_native_message(response_handle);
        ret.ok()
    }

    fn send_request(&self, request: Box<dyn Message>) -> Result<i64, RclReturnCode> {
        let handle = & *self.handle.lock();
        let request_handle = request.get_native_message();
        let sequence_number = core::ptr::null_mut();
        unsafe {
            rcl_send_request(
                handle as *const _, 
                request_handle as *const _,
                sequence_number)
            .ok().
            map(|_| *sequence_number)
        }
    }

}

impl<T> ClientBase for Client<T>
where
    T: MessageDefinition<T> + core::default::Default,
{
    fn handle(&self) -> &ClientHandle {
        self.handle.borrow()
    }

    fn create_message(&self) -> Box<dyn Message> {
        Box::new(T::default())
    }

    fn send_request(&self, request: Box<dyn Message>) -> Result<i64, RclReturnCode> {
        self.send_request(request)
    }

}
