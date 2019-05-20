use std::mem;
use std::io::prelude::*;
use super::{JitMemory, PAGE_SIZE};
use super::ast::*;
use msc::MscsbFile;
use std::io::Cursor;
use x86asm::{InstructionEncodingError, InstructionWriter, Mnemonic, Mode, Operand, Reg};
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

impl Compilable for MscsbFile {
    fn compile(&self) -> Option<CompiledProgram> {
        let buffer = Cursor::new(Vec::new());
        let mut writer = InstructionWriter::new(buffer, Mode::Long);
        
        let mut string_writer = Cursor::new(Vec::new());
        let mut string_offsets: Vec<usize> = vec![];
        for string in self.strings.iter() {
            string_offsets.push(string_writer.get_ref().len());
            string_writer.write(string.as_bytes()).ok()?;
            string_writer.write(&[0u8]).ok()?;
        }
        let string_section = string_writer.into_inner();
        let string_offsets = string_offsets.iter().map(
            |offset| unsafe {
                string_section.as_ptr().offset(*offset as isize) as *const c_void
            }
        ).collect::<Vec<*const c_void>>();

        let ast = self.scripts[0].as_ast();
        // Setup stack frame and whatnot
        writer.setup_stack_frame(0).ok()?;
        for node in ast.nodes.iter() {
            match node {
                Node::Printf { str_num, args: _ } => {
                    let str_num = str_num.as_u32().expect("Printf formatter not string literal");
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RDI),
                        Operand::Literal64(unsafe {
                            mem::transmute(string_offsets[str_num as usize])
                        })
                    ).ok()?;
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RAX),
                        Operand::Literal64(0)
                    ).ok()?;
                    writer.write2(
                        Mnemonic::MOV,
                        Operand::Direct(Reg::RCX),
                        Operand::Literal64(unsafe {
                            mem::transmute(libc::printf as *const c_void)
                        })
                    ).ok()?;
                    writer.write1(
                        Mnemonic::CALL,
                        Operand::Direct(Reg::RCX)
                    ).ok()?;
                }
                _ => {}
            }
        }
        writer.write_ret().ok()?;

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
    fn write_ret(&mut self) -> Result<(), InstructionEncodingError>;
    fn setup_stack_frame(&mut self, num_vars: u32) -> Result<(), InstructionEncodingError>;
    fn save_nonvolatile_regs(&mut self) -> Result<(), InstructionEncodingError>;
    fn restore_nonvolatile_regs(&mut self) -> Result<(), InstructionEncodingError>;
}

static NONVOLATILE_REGS: &[Reg] = &[Reg::RBX, Reg::RBP, Reg::RDI, Reg::RSI,
                                    Reg::R12, Reg::R13, Reg::R14, Reg::R15];

impl<T: Write> AsmWriterHelper for InstructionWriter<T> {
    fn write_ret(&mut self) -> Result<(), InstructionEncodingError> {
        self.write2(
            Mnemonic::MOV,
            Operand::Direct(Reg::RSP),
            Operand::Direct(Reg::RBP)
        )?;
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
        self.write2(
            Mnemonic::MOV,
            Operand::Direct(Reg::RBP),
            Operand::Direct(Reg::RSP)
        )?;
        if num_vars > 0 {
            self.write2(
                Mnemonic::SUB,
                Operand::Direct(Reg::RSP),
                Operand::Literal32(4 * num_vars)
            )?;
        }
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
