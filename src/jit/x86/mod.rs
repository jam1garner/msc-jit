use super::JitMemory;
use msc::{MscsbFile, Cmd};
use std::io::Cursor;
use x86asm::{Instruction, InstructionWriter, Mnemonic, Mode, Operand, Reg};

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
        let buffer = Cursor::new(buffer);
        let mut writer = InstructionWriter::new(buffer, Mode::Long);
        
        let mut current_val: &Cmd = &Cmd::Nop;
        for command in self.scripts[0].commands.iter() {
            if command.push_bit {
                current_val = &command.cmd;
            }
            match command.cmd {
                Cmd::Begin { var_count: _, arg_count: _ } => {
                    // TODO: literally all of this
                }
                Cmd::Return6 => {
                    match current_val {
                        Cmd::PushInt { val } => {
                            writer.write(
                                &Instruction::new2(
                                    Mnemonic::MOV,
                                    Operand::Direct(Reg::EAX),
                                    Operand::Literal32(*val)
                                )
                            ).unwrap();
                            // asm_buffer.write(&[0xB8, (val & 0xFF) as u8, ((val & 0xFF00) >> 8) as u8, ((val & 0xFF0000) >> 0x10) as u8, ((val & 0xFF000000) >> 0x18) as u8]);
                        }
                        Cmd::PushShort { val } => {
                            writer.write(
                                &Instruction::new2(
                                    Mnemonic::MOV,
                                    Operand::Direct(Reg::EAX),
                                    Operand::Literal32(*val as u32)
                                )
                            ).unwrap();
                            // asm_buffer.write(&[0xB8, (val & 0xFF) as u8, ((val & 0xFF00) >> 8) as u8, 0, 0]);
                        }
                        _ => {}
                    }
                }
                Cmd::End => {
                    // TODO: all of this too
                }
                _ => {}
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

