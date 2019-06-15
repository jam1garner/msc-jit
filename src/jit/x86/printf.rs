use libc::{c_char, c_void};

pub unsafe extern "C" fn msc_printf(fmt: *const c_char, args: *const u64, argc: u32) {
    libc::printf(fmt);
}
