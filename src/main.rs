extern crate msc;
extern crate libc;
extern crate x86asm;
mod jit;

use jit::ast::*;
use jit::x86::*;

fn main() {
    let test = msc::MscsbFile::open("/home/jam/dev/msc/msc-jit/test.mscsb")
                    .unwrap();
    
    println!("{:#?}", test.scripts[0].as_ast());
    let mut test_compiled = test.compile();
    test_compiled.lock_all();
    test_compiled.run();
}
