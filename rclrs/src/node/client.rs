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
use core::borrow::Borrow;
use core::marker::PhantomData;
use cstr_core::CString;
use rclrs_msg_utilities::traits::{Message, ServiceType};
use hashbrown::HashMap;

#[cfg(not(feature = "std"))]
use spin::{Mutex, MutexGuard};

#[cfg(feature = "std")]
use parking_lot::{Mutex, MutexGuard};

#[cfg(feature = "std")]
use std::time::SystemTime;

pub(crate) struct ClientHandle {
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

pub(crate) trait ClientBase {
    fn handle(&self) -> &ClientHandle;
}

pub struct Client<T>
where
    T: ServiceType,
{
    pub(crate) handle: Arc<ClientHandle>,
    message: PhantomData<T>,
    #[cfg(feature = "std")]
    pending_requests: HashMap<i64, (SystemTime, Box<dyn FnOnce(T::Response) + Send + Sync>)>,
    #[cfg(not(feature = "std"))]
    pending_requests: HashMap<i64, Box<dyn FnOnce(T::Response) + Send + Sync>>,
}

impl<ST> Client<ST>
where
    ST: ServiceType,
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
    pub fn new(node: &Node, topic: &str, qos: QoSProfile) -> Result<Self, RclReturnCode> {
        let mut client_handle = unsafe { rcl_get_zero_initialized_client() };
        let type_support = ST::get_type_support() as *const rosidl_service_type_support_t;
        let topic_c_string = CString::new(topic).unwrap(); // If the topic name is unrepresentable as a c-string, RCL will be unable to use it
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
            pending_requests: HashMap::new(),
        })
    }

    /// Add a request to the requests collection.
    /// 
    /// In systems with [`std`] enabled, this also stores a timestamp as to when the callback was stored.
    /// If [`std`] is not enabled, only the callback is stored, and time-related pruning functions are disabled.
    /// 
    /// # Parameters
    /// * `request_id` - The request ID returned by [`Client::send_request`]
    /// * `callback` - The callback to execute on the returned response (when it comes)
    fn add_request(
        &mut self,
        request_id: &i64,
        callback: Box<dyn FnOnce(ST::Response) + Send + Sync>,
    ) {
        #[cfg(not(feature = "std"))]
        self.pending_requests.insert(*request_id, callback);
        
        #[cfg(feature = "std")]
        self.pending_requests.insert(*request_id, (SystemTime::now(), callback));
    }

    /// Clean up a pending request.
    ///
    /// This notifies the client that we have waited long enough for a response from the server
    /// to come; we have given up, and are not waiting for a response anymore.
    ///
    /// Not calling this will make the client start using more memory for each request
    /// that never got a reply from the server.
    ///
    /// # Parameters
    /// * `request_id` - The request ID returned by [`Client::send_request()`]
    /// * returns - `true` when a pending request was removed, `false` if not (e.g. a response was recieved)
    fn remove_pending_request(&mut self, request_id: &i64) -> bool {
        self.pending_requests.remove(request_id).is_some()
    }

    /// Clean all pending requests.
    ///
    /// # Parameters
    /// * returns - The number of pending requests that were removed.
    fn prune_pending_requests(&mut self) -> usize {
        let old_size = self.pending_requests.len();
        self.pending_requests.clear();
        old_size
    }

    /// Clean all pending requests older than a [`time_point`].
    ///
    /// # Parameters
    /// * `time_point` - Requests that were sent before this point are going to be removed.
    /// * returns - The number of pending requests that were removed.
    #[cfg(feature = "std")]
    fn prune_requests_older_than(&mut self, time_point: &SystemTime) -> usize {
        let old_size = self.pending_requests.len();
        self.pending_requests
            .retain(|_, (tp, _)| *tp > *time_point);
        old_size - self.pending_requests.len()
    }

    /// Grab and remove callback from requests_collection corresponding to passed id.
    /// 
    /// # Parameters
    /// * `request_id` - The request ID returned by [`Client::send_request`].
    /// * returns - The callback (if it exists) or [`None`]
    fn get_and_erase_pending_request(
        &mut self,
        request_id: &i64,
    ) -> Option<Box<dyn FnOnce(ST::Response) + Send + Sync>> {
        self.pending_requests
            .remove(request_id)
            .map(|(_, cb)| cb)
    }
    fn service_is_ready(&self) -> Result<bool, RclReturnCode> {
        let node_handle = &*self.handle.node_handle.lock();
        let client_handle = &*self.handle.handle.lock();
        let mut is_ready = false;
        unsafe {
            rcl_service_server_is_available(
                node_handle as *const _,
                client_handle as *const _,
                &mut is_ready as *mut _,
            )
        }
        .ok()?;
        Ok(is_ready)
    }

    fn take_response(&self, response: &mut ST::Response) -> Result<(), RclReturnCode> {
        let handle = &*self.handle.lock();
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

    /// Send a request to the service server, and schedule a callback in the executor.
    ///
    /// If the callback is never called (because we never got a reply for the service server),
    /// [`remove_pending_request`] has to be called with the returned request ID, or [`prune_pending_requests()`].
    /// Not doing so will make the [`Client`] instance use more memory each time a response is not
    /// recieved from the service server.
    /// In this case, it's convenient to setup a timer to clean up the pending requests.
    ///
    /// # Parameters
    /// * `request` - The request to be sent
    /// * `callback` - The callback that will be called when we get a response for this request.
    /// * returns - The request ID representing the request just sent
    pub fn send_request(
        &mut self,
        request: ST::Request,
        callback: Box<dyn FnOnce(ST::Response) + Send + Sync>,
    ) -> Result<i64, RclReturnCode> {
        let request_handle = request.get_native_message();
        let sequence_number = core::ptr::null_mut();
        let ret = unsafe {
            let handle = &*self.handle.lock();
            rcl_send_request(
                handle as *const _,
                request_handle as *const _,
                sequence_number,
            )
            .ok()
            .map(|_| *sequence_number)
        }?;
        self.add_request(&ret, callback);
        Ok(ret)
    }
}

impl<ST> ClientBase for Client<ST>
where
    ST: ServiceType + core::default::Default,
{
    fn handle(&self) -> &ClientHandle {
        self.handle.borrow()
    }
}
