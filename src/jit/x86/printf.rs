use libc::{c_char, c_void};
use std::ffi::CString;
use std::mem::transmute;

pub unsafe extern "C" fn msc_printf(fmt: *const c_char, args_ptr: *const u64, argsc: u64) {
    let args = std::slice::from_raw_parts(args_ptr, argsc as usize);
    let len = libc::strlen(fmt);
    let mut fmt = fmt as *const u8;
    let mut arg_i = 0;
    let end = fmt.offset(len as isize);
    while fmt < end {
        match *fmt as char {
            '%' => {
                let specifier_start = fmt;
                fmt = fmt.offset(1);
                if *fmt as char == '%' {
                    libc::putchar('%' as i32);
                }
                // Evaluate format specifier
                loop {
                    // While in pad length of format specifier
                    match *fmt {
                        0x30..=0x39 => { fmt = fmt.offset(1) }
                        _ => { break }
                    }
                }
                // Parse decimal count
                if *fmt as char == '.' {
                    fmt = fmt.offset(1);
                    loop {
                        match *fmt {
                            0x30..=0x39 => { fmt = fmt.offset(1) }
                            _ => { break }
                        }
                    }
                }
                match *fmt as char {
                    'c' | 'd' | 'i' | 'l' | 'o' | 'x' | 'p' | 'u' | 'X' => {
                        fmt = fmt.offset(1);
                        let s = std::slice::from_raw_parts(
                            specifier_start,
                            fmt as usize - specifier_start as usize
                        );
                        if let Ok(cstr) = CString::new(s) {
                            libc::printf(cstr.as_ptr(), args[argsc as usize - (arg_i as usize + 1)]);
                        }
                        arg_i += 1;
                    }
                    'f' | 'e' | 'g' => {
                        fmt = fmt.offset(1);
                        let s = std::slice::from_raw_parts(
                            specifier_start,
                            fmt as usize - specifier_start as usize
                        );
                        if let Ok(cstr) = CString::new(s) {
                            let val = *(&args[argsc as usize - (arg_i as usize + 1)] as *const u64 as *const f32);
                            libc::printf(cstr.as_ptr(), val as f64);
                        }
                        arg_i += 1;
                    }
                    _ => {
                        println!("oof");
                        fmt = fmt.offset(1);
                    }
                }
                continue;
            }
            _ => {
                libc::putchar(*fmt as i32);
            }
        }
        fmt = fmt.offset(1);
    }
    //libc::printf(fmt);
}
