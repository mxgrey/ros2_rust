#include "rosidl_runtime_c/string_functions.h"
#include "rosidl_runtime_c/u16string_functions.h"
#include "rosidl_runtime_c/message_type_support_struct.h"

@{
from rosidl_parser.definition import AbstractGenericString
from rosidl_parser.definition import AbstractWString
from rosidl_parser.definition import AbstractNestedType
from rosidl_parser.definition import Array
from rosidl_parser.definition import BasicType
from itertools import chain
import sys
}@

@[for subfolder, srv_spec in srv_specs]@
@{
type_name = srv_spec.namespaced_type.name
request_members = srv_spec.request_message.structure.members
request_c_fields = []
for member in request_members:
    if type(member.type) is Array:
        pass
    else:
        if isinstance(member.type, BasicType) or isinstance(member.type, AbstractGenericString):
            request_c_fields.append(f"{get_c_type(member.type)} {member.name}")
        else:
            pass

response_members = srv_spec.response_message.structure.members
response_c_fields = []
for member in response_members:
    if type(member.type) is Array:
        pass
    else:
        if isinstance(member.type, BasicType) or isinstance(member.type, AbstractGenericString):
            response_c_fields.append(f"{get_c_type(member.type)} {member.name}")
        else:
            pass

msg_normalized_type = get_rs_type(srv_spec.namespaced_type).replace('::', '__')
base_c_function_prefix = gen_c_function_prefix(package_name, subfolder, type_name)
request_c_function_prefix = f"{base_c_function_prefix}_request"
request_service_type = f"{msg_normalized_type}_Request"
response_c_function_prefix = f"{base_c_function_prefix}_response"
response_service_type = f"{msg_normalized_type}_Response"
}@

#include "@(package_name)/@(subfolder)/@(convert_camel_case_to_lower_case_underscore(type_name)).h"

@# Generate _get_type_support()
uintptr_t @(base_c_function_prefix)_get_type_support() {
    return (uintptr_t)ROSIDL_GET_SRV_TYPE_SUPPORT(@(package_name), @(subfolder), @(srv_spec.namespaced_type.name));
}

@# Generate _get_native_message() for each of request and response messages
@# @[def gen_get_native_message(is_request, package_name, subfolder, c_fields, type_name, normalized_type, member_iterable)]@
@[def gen_get_native_message(c_function_prefix, service_type, c_fields, member_iterable)]@
@{
from rosidl_parser.definition import AbstractGenericString, AbstractWString, Array, BasicType
}@
uintptr_t @(c_function_prefix)_get_native_message(
  @(', '.join(c_fields))) {
    @(service_type) *ros_message = @(service_type)__create();
@[    for member in member_iterable]@
@[        if isinstance(member.type, Array)]@
@[        elif isinstance(member.type, AbstractGenericString)]@
@[            if isinstance(member.type, AbstractWString)]@
    rosidl_runtime_c__U16String__assign(&ros_message->@(member.name)), @(member.name));
@[            else]@
    rosidl_runtime_c__String__assign(&(ros_message->@(member.name)), @(member.name));
@[            end if]@
@[        elif isinstance(member.type, BasicType)]@
    ros_message->@(member.name) = @(member.name);  
@[        end if]@
@[    end for]@
    return (uintptr_t)ros_message;
}
@[end def]@
@
@# Generate _read_handle() for each of request and response message members
@[def gen_read_handle(c_function_prefix, service_type, member_iterable)]@
@{
from rosidl_cmake import convert_camel_case_to_lower_case_underscore
from rosidl_parser.definition import AbstractGenericString
from rosidl_parser.definition import AbstractNestedType
from rosidl_parser.definition import Array
from rosidl_parser.definition import BasicType
from rosidl_generator_rs import get_c_type
}@
@[    for member in member_iterable]@
@(get_c_type(member.type)) @(c_function_prefix)_@(member.name)_read_handle(uintptr_t message_handle) {
@[        if isinstance(member.type, Array)]@
    (void)message_handle;
    return 0;
@[        elif isinstance(member.type, AbstractGenericString)]@
    @(service_type) * ros_message = (@(service_type) *)message_handle;
    return ros_message->@(member.name).data;
@[        elif isinstance(member.type, BasicType)]@
    @(service_type) * ros_message = (@(service_type) *)message_handle;
    return ros_message->@(member.name);
@[        elif isinstance(member.type, AbstractNestedType)]@
    @(service_type) * ros_message = (@(service_type) *)message_handle;
    return (@(get_c_type(member.type)))&ros_message->@(member.name);
@[        else]@
    (void)message_handle;
    return 0;
@[        end if]@
}
@[    end for]@
@[end def]@
@# Request case
@(gen_get_native_message(request_c_function_prefix, request_service_type, request_c_fields, request_members))@

void @(request_c_function_prefix)_destroy_native_message(void * raw_ros_message) {
      @(request_service_type) * ros_message = raw_ros_message;
      @(request_service_type)__destroy(ros_message);
}

@(gen_read_handle(request_c_function_prefix, request_service_type, request_members))@

@# Response case
@(gen_get_native_message(response_c_function_prefix, response_service_type, response_c_fields, response_members))@

void @(response_c_function_prefix)_destroy_native_message(void * raw_ros_message) {
      @(response_service_type) * ros_message = raw_ros_message;
      @(response_service_type)__destroy(ros_message);
}

@(gen_read_handle(response_c_function_prefix, response_service_type, response_members))@
@[end for]@
