use libc::{c_char};
use std::ffi::CString;

/// Extremely unsafe, sneeze and you'll have code execution
pub unsafe extern "C" fn msc_printf(fmt: *const c_char, args_ptr: *const u64, argsc: u64) {
    let args = std::slice::from_raw_parts(args_ptr, argsc as usize);
    let len = libc::strlen(fmt);
    let mut fmt = fmt as *const u8;
    let mut arg_i = 0;
    let end = fmt.add(len);
    while fmt < end {
        match *fmt as char {
            '%' => {
                let specifier_start = fmt;
                fmt = fmt.offset(1);
                if *fmt as char == '%' {
                    libc::putchar('%' as i32);
                }
                // Evaluate format specifier
                while let 0x30..=0x39 = *fmt {
                    // While in pad length of format specifier
                    fmt = fmt.offset(1);
                }
                // Parse decimal count
                if *fmt as char == '.' {
                    fmt = fmt.offset(1);
                    while let 0x30..=0x39 = *fmt {
                        fmt = fmt.offset(1);
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
                            libc::printf(cstr.as_ptr(), f64::from(val));
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
                libc::putchar(i32::from(*fmt));
            }
        }
        fmt = fmt.offset(1);
    }
}
