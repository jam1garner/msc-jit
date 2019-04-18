use super::JitMemory;
use std::io::prelude::*;
use msc::{MscsbFile, Script, Command, Cmd};
use std::io::Cursor;

mod assembler;

pub struct CompiledProgram {
    pub mem: Vec<JitMemory>,
    pub entrypoint_index: usize,
}

pub trait Compilable {
    fn compile(&self) -> CompiledProgram;
}

impl Compilable for MscsbFile {
    fn compile(&self) -> CompiledProgram {
        let mut mem = JitMemory::new(1);
        let buffer = unsafe {mem.as_slice()};
        let mut asm_buffer = Cursor::new(buffer);
        
        let mut current_val: &Cmd = &Cmd::Nop;
        for command in self.scripts[0].commands.iter() {
            match command.cmd {
                Cmd::Begin { var_count, arg_count } => {
                    // TODO: literally all of this
                }
                Cmd::PushInt { val } => {
                    current_val = &command.cmd;
                }
                Cmd::PushShort { val } => {
                    current_val = &command.cmd;
                }
                Cmd::Return6 => {
                    match current_val {
                        Cmd::PushInt { val } => {
                            asm_buffer.write(&[0xB8, (val & 0xFF) as u8, ((val & 0xFF00) >> 8) as u8, ((val & 0xFF0000) >> 0x10) as u8, ((val & 0xFF000000) >> 0x18) as u8]);
                        }
                        Cmd::PushShort { val } => {
                            asm_buffer.write(&[0xB8, (val & 0xFF) as u8, ((val & 0xFF00) >> 8) as u8, 0, 0]);
                        }
                        _ => {}
                    }
                }
                Cmd::End => {
                    // TODO: all of this too
                }
                _ => { panic!("Opcode {:?} not support", command.cmd) }
            }
        }

        CompiledProgram { mem: vec![mem], entrypoint_index: 0 }
    }
}



impl CompiledProgram {
    pub fn lock_all(&mut self) {
        for jit_mem in self.mem.iter_mut() {
            let ret = unsafe { jit_mem.lock() };
            if ret != 0 {
                panic!("Error: lock_all lock returned {}", ret);
            }
        }
    }

    pub fn run(&self) {
        if self.mem.len() <= self.entrypoint_index {
            panic!("Error: entrypoint_index '{}' out of bounds (< {})",
                   self.entrypoint_index, self.mem.len());
        }
        unsafe {
            let ret = self.mem[self.entrypoint_index].run::<u32>();
            println!("Return value - {}", ret);
        }
    }
}

