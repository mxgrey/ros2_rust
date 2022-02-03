@{
from rosidl_parser.definition import AbstractGenericString
from rosidl_parser.definition import AbstractNestedType
from rosidl_parser.definition import AbstractSequence
from rosidl_parser.definition import BasicType
from rosidl_parser.definition import Array
from itertools import chain

# Quick check to see if we have any special imports to do
has_string = False
for _subfolder, srv_spec in srv_specs:
     for member in chain(srv_spec.request_message.structure.members, srv_spec.response_message.structure.members):
        if isinstance(member.type, AbstractGenericString):
            has_string = True
            break
}@
@[if has_string]@
use libc::c_char;
@[end if]@
use libc::uintptr_t;
use rclrs_msg_utilities;
@[if has_string]@
use std::ffi::CString;
use std::ffi::CStr;
@[end if]@

@[for subfolder, srv_spec in srv_specs]@
@{
type_name = srv_spec.namespaced_type.name

request_members = srv_spec.request_message.structure.members
response_members = srv_spec.response_message.structure.members

base_c_function_prefix = gen_c_function_prefix(package_name, subfolder, type_name)
request_c_function_prefix = f"{base_c_function_prefix}_request"
request_struct_name = f"{type_name}Request"

response_c_function_prefix = f"{base_c_function_prefix}_response"
response_struct_name = f"{type_name}Response"
}@
@
@# Generates Rust structure for request/response type
@[def gen_message_struct(struct_name, package_name, member_iterable)]@
@{
from rosidl_generator_rs import get_rs_name, get_rs_type
}@
#[derive(Default)]
pub struct @(struct_name) {
@[    for member in member_iterable]@
    pub @(get_rs_name(member.name)): @(get_rs_type(member.type).replace(package_name, 'crate')),
@[    end for]@
}
@[end def]@
@
@# Generates Rust extern block for request/response type
@[def gen_message_extern(c_function_prefix, package_name, member_iterable)]@
@{
from rosidl_cmake import convert_camel_case_to_lower_case_underscore
from rosidl_parser.definition import AbstractGenericString, Array, BasicType
from rosidl_generator_rs import  get_rs_name, get_rs_type
}@
#[link(name = "@(package_name)__rosidl_typesupport_c_rsext")]
extern "C" {
    fn @(c_function_prefix)_get_type_support() -> uintptr_t;
    
    fn @(c_function_prefix)_get_native_message(
@[    for member in member_iterable]@
@[        if isinstance(member.type, AbstractGenericString)]@
        @(get_rs_name(member.name)): *const_c_char,
@[        elif isinstance(member.type, BasicType)]@
        @(get_rs_name(member.name)): @(get_rs_type(member.type)),
@[        end if]@
@[    end for]@
    ) -> uintptr_t;

    fn @(c_function_prefix)_destroy_native_message(message_handle: uintptr_t) -> ();

@[    for member in member_iterable]@
@[        if isinstance(member.type, Array)]@
@[        elif isinstance(member.type, AbstractGenericString)]@
    fn @(c_function_prefix)_@(member.name)_read_handle(message_handle: uintptr_t) -> *const c_char;
@[        elif isinstance(member.type, BasicType)]@
    fn @(c_function_prefix)_@(member.name)_read_handle(message_handle: uintptr_t) -> @(get_rs_type(member.type));
@[        end if]@
@[    end for]@
}
@[end def]@
@
@# Generates Rust impl block for request/response type
@[def gen_impl(struct_name, c_function_prefix, member_iterable)]@
@{
from rosidl_parser.definition import AbstractGenericString, AbstractSequence, Array, BasicType
from rosidl_generator_rs import get_rs_name
}@
impl @(struct_name) {
    fn get_native_message(&self) -> uintptr_t {
        unsafe { @(c_function_prefix)_get_native_message(
@[    for member in member_iterable]@
@[        if isinstance(member.type, Array)]@
@[        elif isinstance(member.type, AbstractGenericString)]@
                {let s = CString::new(self.@(get_rs_name(member.name)).clone()).unwrap();
                let p = s.as_ptr();
                std::mem::forget(s);
                p},
@[        elif isinstance(member.type, BasicType)]@
                self.@(get_rs_name(member.name)),
@[        end if]@
@[    end for]@
            )
        }
    }

    fn destroy_native_message(&self, message_handle: uintptr_t) -> () {
        unsafe {@(c_function_prefix)_destroy_native_message(message_handle);}
    }

    fn read_handle(&mut self, _message_handle: uintptr_t) -> () {
        unsafe {
            {
@[    for member in member_iterable]@
@[        if isinstance(member.type, Array)]@
@[        elif isinstance(member.type, AbstractGenericString)]@
                let ptr = @(c_function_prefix)_read_handle(_message_handle);
                self.@(get_rs_name(member.name)) = CStr::from_ptr(ptr).to_string_lossy().into_owned();
@[        elif isinstance(member.type, BasicType)]@
                self.@(get_rs_name(member.name)) = @(c_function_prefix)_read_handle(_message_handle);
@[        elif isinstance(member.type, AbstractSequence)]@
@[        end if]@
@[    end for]@
            }
        }
    }
}
@[end def]@
@
@# Generates Rust Message Trait implementation block for request/response type
@[def gen_message_trait_impl(struct_name)]@
impl rclrs_msg_utilities::traits::Message for @(struct_name) {
    fn get_native_message(&self) -> uintptr_t {
        return self.get_native_message();
    }

    fn destroy_native_message(&self, message_handle: uintptr_t) -> () {
        self.destroy_native_message(message_handle);
    }

    fn read_handle(&mut self, message_handle: uintptr_t) -> () {
        self.read_handle(message_handle);
    }
}
@[end def]@
@
@# Generates Rust MessageDefinition Trait implementation block for request/response type
@[def gen_message_definition_trait_impl(struct_name, c_function_prefix)]@
impl rclrs_msg_utilities::traits::MessageDefinition<@(struct_name)> for @(struct_name) {
    fn get_type_support() -> uintptr_t {
        unsafe { @(c_function_prefix)_get_type_support() }
    }

    fn static_get_native_message(message: &@(struct_name) -> uintptr_t {
        message.get_native_message()
    }

    fn static_destroy_native_message(message_handle: uintptr_t) -> () {
        unsafe { @(c_function_prefix)_destroy_native_message(message_handle) }
    }
}
@[end def]@
@
@# Generate infrastructure for Requests
@(gen_message_struct(request_struct_name, package_name, request_members))@

@(gen_message_extern(request_c_function_prefix, package_name, request_members))@

@(gen_impl(request_struct_name, request_c_function_prefix, request_members))@

@(gen_message_trait_impl(request_struct_name))@

@(gen_message_definition_trait_impl(request_struct_name, request_c_function_prefix))@

@# Generate infrastructure for Responses
@(gen_message_struct(response_struct_name, package_name, response_members))@

@(gen_message_extern(response_c_function_prefix, package_name, response_members))@

@(gen_impl(response_struct_name, response_c_function_prefix, response_members))@

@(gen_message_trait_impl(response_struct_name))@

@(gen_message_definition_trait_impl(response_struct_name, response_c_function_prefix))@

@[end for]
