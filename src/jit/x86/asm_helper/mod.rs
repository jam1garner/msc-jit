use std::io::prelude::*;
use x86asm::{OperandSize, RegScale, InstructionEncodingError, InstructionWriter, Mnemonic, Mode, Operand, Reg};

mod into_operand;
pub use into_operand::*;

pub trait AsmWriterHelper {
    fn write_ret(&mut self, num_vars: u32) -> Result<(), InstructionEncodingError>;
    fn setup_stack_frame(&mut self, num_vars: u32) -> Result<(), InstructionEncodingError>;
    fn save_nonvolatile_regs(&mut self) -> Result<(), InstructionEncodingError>;
    fn restore_nonvolatile_regs(&mut self) -> Result<(), InstructionEncodingError>;
    fn pop(&mut self, reg: Reg) -> Result<(), InstructionEncodingError>;
    fn push<I: IntoOperand>(&mut self, operand: I) -> Result<(), InstructionEncodingError>;
    fn mov<I: IntoOperand, I2: IntoOperand>(&mut self, op1: I, op2: I2) -> Result<(), InstructionEncodingError>;
    fn call<I: IntoOperand>(&mut self, op1: I) -> Result<(), InstructionEncodingError>;
    fn get_global(&mut self, globals: *const u32, reg: Reg, global_num: u16) -> Result<(), InstructionEncodingError>;
    fn set_global(&mut self, globals: *const u32, reg: Reg, global_num: u16) -> Result<(), InstructionEncodingError>;
    fn copy_to_fpu(&mut self, count: usize) -> Result<(), InstructionEncodingError>;
    fn copy_to_fpu_rev(&mut self, count: usize) -> Result<(), InstructionEncodingError>;
    fn get_global_float(&mut self, globals: *const u32, global_num: u16) -> Result<(), InstructionEncodingError>;
    fn set_global_float(&mut self, globals: *const u32, global_num: u16) -> Result<(), InstructionEncodingError>;
    fn fstsw_ax(&mut self) -> Result<(), std::io::Error>;
    fn sahf(&mut self) -> Result<(), std::io::Error>;
    fn fcompp(&mut self) -> Result<(), std::io::Error>;
}

static NONVOLATILE_REGS: &[Reg] = &[Reg::RBX, Reg::RBP, Reg::RDI, Reg::RSI,
                                    Reg::R12, Reg::R13, Reg::R14, Reg::R15];

impl<T: Write + Seek> AsmWriterHelper for InstructionWriter<T> {
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

    fn pop(&mut self, reg: Reg) -> Result<(), InstructionEncodingError> {
        self.write1(
            Mnemonic::POP,
            Operand::Direct(reg)
        )?;
        Ok(())
    }
    
    fn push<I: IntoOperand>(&mut self, operand: I) -> Result<(), InstructionEncodingError> {
        self.write1(
            Mnemonic::PUSH,
            operand.into_op()
        )?;
        Ok(())
    }

    fn mov<I: IntoOperand, I2: IntoOperand>(&mut self, op1: I, op2: I2) -> Result<(), InstructionEncodingError> {
        self.write2(
            Mnemonic::MOV,
            op1.into_op(),
            op2.into_op()
        )?;
        Ok(())
    }

    fn call<I: IntoOperand>(&mut self, op1: I) -> Result<(), InstructionEncodingError> {
        self.write1(
            Mnemonic::CALL,
            op1.into_op()
        )?;
        Ok(())
    }
    
    fn get_global(&mut self, globals: *const u32, reg: Reg, global_num: u16) -> Result<(), InstructionEncodingError> {
        self.mov(Reg::RDX, globals as u64)?;
        self.mov(
            reg,
            (Reg::RDX, global_num as u64 * 4, OperandSize::Dword)
        )?;
        Ok(())
    }
    
    fn set_global(&mut self, globals: *const u32, reg: Reg, global_num: u16) -> Result<(), InstructionEncodingError> {
        self.mov(Reg::RDX, globals as u64)?;
        self.mov(
            (Reg::RDX, global_num as u64 * 4, OperandSize::Dword),
            reg
        )?;
        Ok(())
    }

    fn get_global_float(&mut self, globals: *const u32, global_num: u16) -> Result<(), InstructionEncodingError> {
        self.mov(Reg::RDX, globals as u64)?;
        self.write1(
            Mnemonic::FLD,
            (Reg::RDX, global_num as u64 * 4, OperandSize::Dword).into_op()
        )?;
        Ok(())
    }

    fn set_global_float(&mut self, globals: *const u32, global_num: u16) -> Result<(), InstructionEncodingError> {
        self.mov(Reg::RDX, globals as u64)?;
        self.write1(
            Mnemonic::FSTP,
            (Reg::RDX, global_num as u64 * 4, OperandSize::Dword).into_op()
        ).unwrap();
        Ok(())
    }

    fn copy_to_fpu(&mut self, count: usize) -> Result<(), InstructionEncodingError> {
        for i in 1..count+1 {
            self.write1(
                Mnemonic::FLD,
                (Reg::RSP, ((count - i) * 8) as u64, OperandSize::Dword).into_op()
            )?;
        }
        Ok(())
    }

    fn copy_to_fpu_rev(&mut self, count: usize) -> Result<(), InstructionEncodingError> {
        for i in (1..count+1).rev() {
            self.write1(
                Mnemonic::FLD,
                (Reg::RSP, ((count - i) * 8) as u64, OperandSize::Dword).into_op()
            )?;
        }
        Ok(())
    }

    fn fstsw_ax(&mut self) -> Result<(), std::io::Error> {
        self.write_bytes(b"\x9B\xDF\xE0")?;
        Ok(())
    }

    fn sahf(&mut self) -> Result<(), std::io::Error> {
        self.write_bytes(b"\x9e")?;
        Ok(())
    }

    fn fcompp(&mut self) -> Result<(), std::io::Error> {
        self.write_bytes(b"\xde\xd9")?;
        Ok(())
    }
}

