use std::mem;
use std::io::prelude::*;
use super::{JitMemory, PAGE_SIZE};
use super::ast::*;
use msc::{MscsbFile, Cmd, Script};
use std::io::Cursor;
use x86asm::{OperandSize, RegScale, InstructionEncodingError, InstructionWriter, Mnemonic, Mode, Operand, Reg};
use libc::c_void;

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
                            writer.write1(
                                Mnemonic::PUSH,
                                Operand::Literal32(val as u32)
                            ).unwrap();
                        }
                    }
                    Cmd::PushInt { val } => {
                        if cmd.push_bit {
                            writer.write1(
                                Mnemonic::PUSH,
                                Operand::Literal32(val)
                            ).unwrap();
                        }
                    }
                    Cmd::PrintF { arg_count } => {
                        writer.write1(
                            Mnemonic::POP,
                            Operand::Direct(Reg::RAX)
                        ).unwrap();
                        writer.write1(
                            Mnemonic::PUSH,
                            Operand::Direct(Reg::RDI)
                        ).unwrap();
                        writer.write2(
                            Mnemonic::MOV,
                            Operand::Direct(Reg::RDI),
                            Operand::Literal64(string_offsets.as_ptr() as u64)
                        ).unwrap();
                        writer.write2(
                            Mnemonic::MOV,
                            Operand::Direct(Reg::RDI),
                            Operand::IndirectScaledIndexed(
                                Reg::RDI,
                                Reg::RAX,
                                RegScale::Eight,
                                Some(OperandSize::Qword),
                                None
                            )
                        ).unwrap();
                        writer.write2(
                            Mnemonic::MOV,
                            Operand::Direct(Reg::AX),
                            Operand::Literal16(0)
                        ).unwrap();
                        writer.write2(
                            Mnemonic::MOV,
                            Operand::Direct(Reg::RCX),
                            Operand::Literal64(
                                libc::printf as u64
                            )
                        ).unwrap();
                        writer.write1(
                            Mnemonic::CALL,
                            Operand::Direct(Reg::RCX)
                        ).unwrap();
                        writer.write1(
                            Mnemonic::POP,
                            Operand::Direct(Reg::RDI)
                        ).unwrap();
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
        /*
        let ast = self.scripts[0].as_ast();
        for node in ast.nodes.iter() {
            match node {
                Node::Printf { str_num, args: _ } => {
                    let str_num = str_num.as_u32().expect("Printf formatter not string literal");
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RDI),
                        Operand::Literal64(
                            string_offsets[str_num as usize] as u64
                        )
                    ).unwrap();
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RAX),
                        Operand::Literal64(0)
                    ).unwrap();
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RCX),
                        Operand::Literal64(
                            libc::printf as u64
                        )
                    ).unwrap();
                    writer.write1(
                        Mnemonic::CALL,
                        Operand::Direct(Reg::RCX)
                    ).unwrap();
                }
                _ => {}
            }
        }*/
        
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
            println!("Return value - {}", ret);
        }
    }
}

trait AsmWriterHelper {
    fn write_ret(&mut self, num_vars: u32) -> Result<(), InstructionEncodingError>;
    fn setup_stack_frame(&mut self, num_vars: u32) -> Result<(), InstructionEncodingError>;
    fn save_nonvolatile_regs(&mut self) -> Result<(), InstructionEncodingError>;
    fn restore_nonvolatile_regs(&mut self) -> Result<(), InstructionEncodingError>;
}

static NONVOLATILE_REGS: &[Reg] = &[Reg::RBX, Reg::RBP, Reg::RDI, Reg::RSI,
                                    Reg::R12, Reg::R13, Reg::R14, Reg::R15];

impl<T: Write> AsmWriterHelper for InstructionWriter<T> {
    fn write_ret(&mut self, num_vars: u32) -> Result<(), InstructionEncodingError> {
        self.write2(
            Mnemonic::MOV,
            Operand::Direct(Reg::RSP),
            Operand::Direct(Reg::RBP)
        )?;
        if num_vars > 0 {
            self.write2(
                Mnemonic::ADD,
                Operand::Direct(Reg::RSP),
                Operand::Literal32(4 * num_vars)
            )?;
        }
        self.write1(
            Mnemonic::POP,
            Operand::Direct(Reg::RBP)
        )?;
        self.write0(
            Mnemonic::RET
        )?;
        Ok(())
    }

    fn setup_stack_frame(&mut self, num_vars: u32) -> Result<(), InstructionEncodingError> {
        self.write1(
            Mnemonic::PUSH,
            Operand::Direct(Reg::RBP)
        )?;
        if num_vars > 0 {
            self.write2(
                Mnemonic::SUB,
                Operand::Direct(Reg::RSP),
                Operand::Literal32(4 * num_vars)
            )?;
        }
        self.write2(
            Mnemonic::MOV,
            Operand::Direct(Reg::RBP),
            Operand::Direct(Reg::RSP)
        )?;
        Ok(())
    }

    fn save_nonvolatile_regs(&mut self) -> Result<(), InstructionEncodingError> {
        for reg in NONVOLATILE_REGS {
            self.write1(Mnemonic::PUSH, Operand::Direct(*reg))?;
        }
        Ok(())
    }

    fn restore_nonvolatile_regs(&mut self) -> Result<(), InstructionEncodingError> {
        for reg in NONVOLATILE_REGS.iter().rev() {
            self.write1(Mnemonic::POP, Operand::Direct(*reg))?;
        }
        Ok(())
    }
}
