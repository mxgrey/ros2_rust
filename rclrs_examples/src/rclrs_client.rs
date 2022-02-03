// Copyright 2022 DCS Corporation, All Rights Reserved.

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
// OPSEC #4584.use std::env;

use std::env;

use anyhow::{Error, Result};
use cstr_core::CString;

fn add(request: &rclrs_example_srvs::srv::AddTwoIntsRequest) -> rclrs_example_srvs::srv::AddTwoIntsResponse {
    rclrs_example_srvs::srv::AddTwoIntsResponse{
        sum : request.a + request.b,
    }
}

fn main() -> Result<(), Error> {
    let args: Vec<CString> = env::args()
        .filter_map(|arg| CString::new(arg).ok())
        .collect();
    let context = rclrs::Context::default(args);
    
    let mut node = context.create_node("minimal_service")?;
    
    let _client = 
        node.create_client::<rclrs_example_srvs::srv::AddTwoInts>(
            "topic",
            rclrs::QOS_PROFILE_DEFAULT
        )?;
    Ok(())
}
