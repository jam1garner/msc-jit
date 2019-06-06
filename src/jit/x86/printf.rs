use libc::{c_char, c_void};

pub extern "C" fn printf(fmt: u32, args: *const u64, argc: u32, strings: *const *const c_void) {
               
}
