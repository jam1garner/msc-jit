extern crate msc;
extern crate libc;
extern crate x86asm;
mod jit;

use jit::x86::*;
use std::io::prelude::*;
//use std::io;

fn gdb(_address: u64) {
    std::fs::File::create("/tmp/msc-jit-temp.txt").unwrap()
        .write_all("layout regs\nb src/jit/mod.rs:50\ncommands\nsi\nend\n".as_bytes()).unwrap();
    println!("sudo gdb -p {} -x /tmp/msc-jit-temp.txt", std::process::id());
    //let stdin = io::stdin();
    //stdin.lock().lines().next().unwrap().ok();
    std::fs::remove_file("/tmp/msc-jit-temp.txt").ok();
}

fn main() {
    let test = msc::MscsbFile::open("/home/jam/dev/msc/msc-jit/printf.mscsb")
                    .unwrap();
    
    //println!("{:#?}", test.scripts[0].as_ast());
    let mut test_compiled = test.compile().expect("Failed to compile");
    test_compiled.lock_all();
    let address = test_compiled.get_entrypoint_address();
    gdb(address);
    test_compiled.run();
}
