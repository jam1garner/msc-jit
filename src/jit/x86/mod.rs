use std::io::prelude::*;
use super::{JitMemory, PAGE_SIZE};
use msc::{MscsbFile, Cmd, Script};
use std::io::Cursor;
use x86asm::{OperandSize, RegScale, InstructionEncodingError, InstructionWriter, Mnemonic, Mode, Operand, Reg};
use libc::c_void;

mod asm_helper;
use asm_helper::*;

pub struct CompiledProgram {
    pub mem: Vec<JitMemory>,
    pub string_section: Vec<u8>,
    pub string_offsets: Vec<*const c_void>,
    pub entrypoint_index: usize,
}

pub trait Compilable {
    fn compile(&self) -> Option<CompiledProgram>;
}

fn get_var_info(script: &Script) -> Option<(u16, u16)> {
    if let Some(cmd) = script.iter().nth(0) {
        if let Cmd::Begin { arg_count, var_count } = cmd.cmd {
            Some((arg_count, var_count))
        } else {
            None
        }
    } else {
        None
    }
}

impl Compilable for MscsbFile {
    fn compile(&self) -> Option<CompiledProgram> {
        let buffer = Cursor::new(Vec::new());
        let mut writer = InstructionWriter::new(buffer, Mode::Long);
        
        let mut string_writer = Cursor::new(Vec::new());
        let mut string_offsets: Vec<usize> = vec![];
        for string in self.strings.iter() {
            string_offsets.push(string_writer.get_ref().len());
            string_writer.write(string.as_bytes()).unwrap();
            string_writer.write(&[0u8]).unwrap();
        }
        let string_section = string_writer.into_inner();
        let string_offsets = string_offsets.iter().map(
            |offset| unsafe {
                string_section.as_ptr().offset(*offset as isize) as *const c_void
            }
        ).collect::<Vec<*const c_void>>();

        // Setup stack frame and whatnot
        if let Some((_, var_count)) = get_var_info(&self.scripts[0]) {
            writer.setup_stack_frame(var_count as u32).unwrap();
            for cmd in self.scripts[0].iter().skip(1) {
                match cmd.cmd {
                    Cmd::Begin { arg_count: _, var_count: _ } => {
                        panic!("Begin not allowed after first command of script");
                    }
                    Cmd::PushShort { val } => {
                        if cmd.push_bit {
                            writer.push(val as u32).unwrap();
                        }
                    }
                    Cmd::PushInt { val } => {
                        if cmd.push_bit {
                            writer.push(val).unwrap();
                        }
                    }
                    Cmd::MultI | Cmd::DivI => {
                        writer.pop(Reg::RCX).unwrap();
                        writer.pop(Reg::RAX).unwrap();
                        if cmd.push_bit {
                            writer.write1(
                                match cmd.cmd {
                                    Cmd::MultI => Mnemonic::MUL,
                                    Cmd::DivI => Mnemonic::DIV,
                                    _ => { unreachable!() }
                                },
                                Operand::Direct(Reg::ECX)
                            ).unwrap();
                            writer.push(Reg::RAX).unwrap();
                        }
                    }
                    Cmd::AddI | Cmd::SubI => {
                        writer.pop(Reg::RCX).unwrap();
                        writer.pop(Reg::RAX).unwrap();
                        if cmd.push_bit {
                            writer.write2(
                                match cmd.cmd {
                                    Cmd::AddI => Mnemonic::ADD,
                                    Cmd::SubI => Mnemonic::SUB,
                                    _ => { unreachable!() }
                                },
                                Operand::Direct(Reg::EAX),
                                Operand::Direct(Reg::ECX)
                            ).unwrap();
                            writer.push(Reg::RAX).unwrap();
                        }
                    }
                    Cmd::NegI => {
                        if cmd.push_bit {
                            writer.write1(
                                Mnemonic::NEG,
                                Operand::Indirect(Reg::RSP, Some(OperandSize::Dword), None)
                            ).unwrap();
                        } else {
                            writer.pop(Reg::RAX).unwrap();
                        }
                    }
                    Cmd::PrintF { arg_count } => {
                        writer.pop(Reg::RAX).unwrap();
                        writer.push(Reg::RDI).unwrap();
                        writer.mov(Reg::RDI,string_offsets.as_ptr() as u64).unwrap();
                        writer.mov(
                            Reg::RDI,
                            (Reg::RDI, Reg::RAX, RegScale::Eight, OperandSize::Qword)
                        ).unwrap();
                        writer.mov(Reg::AX, 0u16).unwrap();
                        writer.mov(Reg::RCX, libc::printf as u64).unwrap();
                        writer.call(Reg::RCX).unwrap();
                        writer.pop(Reg::RDI).unwrap();
                    }
                    Cmd::Return6 | Cmd::Return8 => {
                        writer.pop(Reg::RAX).unwrap();
                        writer.write_ret(var_count as u32).unwrap();
                    }
                    Cmd::Return7 | Cmd::Return9 => {
                        writer.write_ret(var_count as u32).unwrap();
                    }
                    Cmd::Nop | Cmd::End => {}
                    _ => {
                        println!("{:?} not recognized", cmd);
                    }
                }
            }
            writer.write_ret(var_count as u32).unwrap();
        } else {
            writer.write0(
                Mnemonic::RET
            ).unwrap();
        }
        let buffer = writer.get_inner_writer_ref().get_ref();
        let mut mem = JitMemory::new((buffer.len() + (PAGE_SIZE - 1)) / PAGE_SIZE);
        unsafe {
            &mem.as_slice()[..buffer.len()].copy_from_slice(&buffer[..]);
        }
        println!("\n\nEmitted asm:");
        println!("{}", buffer.iter().map(|b| format!("{:02X}", b)).collect::<Vec<String>>().join(" "));
        println!("\n\n\n");
        Some(CompiledProgram {
            mem: vec![mem], entrypoint_index: 0,
            string_section, string_offsets
        })
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
            println!("Return value - 0x{:X}", ret);
        }
    }
}

