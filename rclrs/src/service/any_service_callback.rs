use rosidl_runtime_rs::{Service, Message};

use crate::{
    error::ToResult,
    rcl_bindings::{
        rmw_request_id_t, rmw_service_info_t, rcl_take_request, rcl_take_request_with_info,
    },
    service::ServiceResponseSender,
    RequestId, ServiceInfo, ServiceHandle,
    RclrsError, RclReturnCode,
};

use futures::future::BoxFuture;

use std::sync::Arc;

/// An enum capturing the various possible function signatures for service callbacks.
pub enum AnyServiceCallback<T>
where
    T: Service,
{
    /// A callback that only takes in the request value
    OnlyRequest(Box<dyn FnMut(T::Request) -> BoxFuture<'static, T::Response> + Send>),
    /// A callback that takes in the request value and the ID of the request
    WithId(Box<dyn FnMut(T::Request, RequestId) -> BoxFuture<'static, T::Response> + Send>),
    /// A callback that takes in the request value and all available
    WithInfo(Box<dyn FnMut(T::Request, ServiceInfo) -> BoxFuture<'static, T::Response> + Send>),
}

impl<T: Service> AnyServiceCallback<T> {
    pub(super) fn execute(
        &mut self,
        response_sender: Arc<ServiceResponseSender<T>>,
    ) -> Result<(), RclrsError> {
        let evaluate = || {
            dbg!();
            let commands = Arc::clone(&response_sender.commands);
            match self {
                AnyServiceCallback::OnlyRequest(cb) => {
                    dbg!();
                    let (msg, rmw_request_id) = Self::take_request(&response_sender.handle)?;
                    let response = cb(msg);
                    dbg!();
                    let _ = commands.run(async move {
                        response_sender.send(rmw_request_id, response.await);
                    });
                }
                AnyServiceCallback::WithId(cb) => {
                    dbg!();
                    let (msg, rmw_request_id) = Self::take_request(&response_sender.handle)?;
                    let request_id = RequestId::from_rmw_request_id(&rmw_request_id);
                    let response = cb(msg, request_id);
                    dbg!();
                    let _ = commands.run(async move {
                        response_sender.send(rmw_request_id, response.await);
                    });
                }
                AnyServiceCallback::WithInfo(cb) => {
                    dbg!();
                    let (msg, rmw_service_info) = Self::take_request_with_info(&response_sender.handle)?;
                    let rmw_request_id = rmw_request_id_t {
                        writer_guid: rmw_service_info.request_id.writer_guid,
                        sequence_number: rmw_service_info.request_id.sequence_number,
                    };
                    let service_info = ServiceInfo::from_rmw_service_info(&rmw_service_info);
                    let response = cb(msg, service_info);
                    dbg!();
                    let _ = commands.run(async move {
                        response_sender.send(rmw_request_id, response.await);
                    });
                }
            }

            Ok(())
        };

        dbg!();
        match evaluate() {
            Err(RclrsError::RclError {
                code: RclReturnCode::ServiceTakeFailed,
                ..
            }) => {
                // Spurious wakeup - this may happen even when a waitlist indicated that this
                // subscription was ready, so it shouldn't be an error.
                dbg!();
                println!("Spurious wakeup for service request");
                Ok(())
            }
            other => other,
        }
    }

    /// Fetches a new request.
    ///
    /// When there is no new message, this will return a
    /// [`ServiceTakeFailed`][1].
    ///
    /// [1]: crate::RclrsError
    //
    // ```text
    // +---------------------+
    // | rclrs::take_request |
    // +----------+----------+
    //            |
    //            |
    // +----------v----------+
    // |  rcl_take_request   |
    // +----------+----------+
    //            |
    //            |
    // +----------v----------+
    // |      rmw_take       |
    // +---------------------+
    // ```
    fn take_request(handle: &ServiceHandle) -> Result<(T::Request, rmw_request_id_t), RclrsError> {
        let mut request_id_out = RequestId::zero_initialized_rmw();
        type RmwMsg<T> = <<T as Service>::Request as Message>::RmwMsg;
        let mut request_out = RmwMsg::<T>::default();
        let handle = &*handle.lock();
        unsafe {
            // SAFETY: The three pointers are valid and initialized
            rcl_take_request(
                handle,
                &mut request_id_out,
                &mut request_out as *mut RmwMsg<T> as *mut _,
            )
        }
        .ok()?;
        println!("^^^^^^^^^^ service request arrived: {request_id_out:?} ^^^^^^^^^^^^^^");
        Ok((T::Request::from_rmw_message(request_out), request_id_out))
    }

    /// Same as [`Self::take_request`] but includes additional info about the service
    fn take_request_with_info(handle: &ServiceHandle) -> Result<(T::Request, rmw_service_info_t), RclrsError> {
        let mut service_info_out = ServiceInfo::zero_initialized_rmw();
        type RmwMsg<T> = <<T as Service>::Request as Message>::RmwMsg;
        let mut request_out = RmwMsg::<T>::default();
        let handle = &*handle.lock();
        unsafe {
            // SAFETY: The three pointers are valid and initialized
            rcl_take_request_with_info(
                handle,
                &mut service_info_out,
                &mut request_out as *mut RmwMsg<T> as *mut _,
            )
        }
        .ok()?;
        println!("^^^^^^^^^^^^ service request arrived: {service_info_out:?} ^^^^^^^^^^^^^");
        Ok((T::Request::from_rmw_message(request_out), service_info_out))
    }
}
