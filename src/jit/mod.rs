#![allow(dead_code)]
use std::mem;
pub mod x86;

extern {
    fn memset(s: *mut libc::c_void, c: libc::uint32_t, n: libc::size_t) -> *mut libc::c_void;
}

pub struct JitMemory {
    pub contents : *mut u8,
    pub locked: bool,
    _contents: *mut libc::c_void,
    size: usize,
}

const PAGE_SIZE: usize = 4096;

impl<'a> JitMemory {
    pub fn new(num_pages: usize) -> JitMemory {
        let contents : *mut u8;
        unsafe {
            let size = num_pages * PAGE_SIZE;
            let mut _contents : *mut libc::c_void = mem::uninitialized(); // avoid uninitalized warning
            libc::posix_memalign(&mut _contents, PAGE_SIZE, size);
            libc::mprotect(_contents, size, libc::PROT_READ | libc::PROT_WRITE);

            memset(_contents, 0xc3, size);  // for now, prepopulate with 'RET'

            contents = mem::transmute(_contents);
            JitMemory { contents, _contents, size, locked: false }
        }
    }

    pub unsafe fn lock(&mut self) -> i32 {
        self.locked = false;
        libc::mprotect(self._contents, self.size, libc::PROT_EXEC | libc::PROT_READ)
    }

    pub unsafe fn unlock(&mut self) -> i32 {
        self.locked = false;
        libc::mprotect(self._contents, self.size, libc::PROT_WRITE | libc::PROT_READ)
    }

    pub unsafe fn run<T>(&self) -> T {
        if self.locked {
            panic!("Cannot run locked JitMemory");
        }
        let jit_func: (extern "C" fn() -> T) = mem::transmute(self._contents);
        jit_func()
    }

    pub unsafe fn as_slice(&mut self) -> &'a mut [u8] {
        std::slice::from_raw_parts_mut(self.contents, self.size)
    }
}
