use std::mem;
use std::io::prelude::*;
use super::{JitMemory, PAGE_SIZE};
use super::ast::*;
use msc::{MscsbFile, Cmd};
use std::io::Cursor;
use x86asm::{Instruction, InstructionWriter, Mnemonic, Mode, Operand, Reg};
use libc::{printf, c_void};

pub struct CompiledProgram {
    pub mem: Vec<JitMemory>,
    pub string_section: Vec<u8>,
    pub string_offsets: Vec<*const c_void>,
    pub entrypoint_index: usize,
}

pub trait Compilable {
    fn compile(&self) -> CompiledProgram;
}

impl Compilable for MscsbFile {
    fn compile(&self) -> CompiledProgram {
        let buffer = Cursor::new(Vec::new());
        let mut writer = InstructionWriter::new(buffer, Mode::Long);
        
        let mut string_writer = Cursor::new(Vec::new());
        let mut string_offsets: Vec<usize> = vec![];
        for string in self.strings.iter() {
            string_offsets.push(string_writer.get_ref().len());
            string_writer.write(string.as_bytes());
            string_writer.write(&[0u8]);
        }
        let string_section = string_writer.into_inner();
        let string_offsets = string_offsets.iter().map(
            |offset| unsafe {
                string_section.as_ptr().offset(*offset as isize) as *const c_void
            }
        ).collect::<Vec<*const c_void>>();

        let ast = self.scripts[0].as_ast();
        for node in ast.nodes.iter() {
            match node {
                Node::Printf { str_num, args } => {
                    let str_num = str_num.as_u32().unwrap();
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RDI),
                        Operand::Literal64(unsafe {
                            mem::transmute(string_offsets[str_num as usize])
                        })
                    ).unwrap();
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RAX),
                        Operand::Literal64(0)
                    ).unwrap();
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RCX),
                        Operand::Literal64(unsafe {
                            mem::transmute(libc::printf as *const c_void)
                        })
                    ).unwrap();
                    writer.write1(
                        Mnemonic::CALL,
                        Operand::Direct(Reg::RCX)
                    ).unwrap();
                }
                _ => {}
            }
        }
        writer.write0(Mnemonic::RET);

        let buffer = writer.get_inner_writer_ref().get_ref();
        let mut mem = JitMemory::new((buffer.len() + (PAGE_SIZE - 1)) / PAGE_SIZE);
        unsafe {
            &mem.as_slice()[..buffer.len()].copy_from_slice(&buffer[..]);
        }
        println!("\n\nEmitted asm:");
        println!("{}", buffer.iter().map(|b| format!("{:02X}", b)).collect::<Vec<String>>().join(" "));
        println!("\n\n\n");
        CompiledProgram {
            mem: vec![mem], entrypoint_index: 0,
            string_section, string_offsets
        }
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
            let ret = self.mem[self.entrypoint_index].run::<u64>();
            println!("Return value - {}", ret);
        }
    }
}

