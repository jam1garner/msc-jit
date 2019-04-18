extern crate msc;
extern crate libc;
mod jit;

use jit::x86::*;

fn main() {
    let test = msc::MscsbFile::open("/home/jam/dev/msc/test.mscsb")
                    .unwrap();
    let mut test_compiled = test.compile();
    test_compiled.lock_all();
    test_compiled.run();
}
