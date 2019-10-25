use std::io::prelude::*;
use x86asm::{OperandSize, InstructionEncodingError, InstructionWriter, Mnemonic, Operand, Reg};

mod into_operand;
pub use into_operand::*;

use Mnemonic::*;
use Reg::*;
use Operand::*;
use OperandSize::*;

type Result<T> = std::result::Result<T, InstructionEncodingError>;
type IoResult<T> = std::result::Result<T, std::io::Error>;

pub trait AsmWriterHelper {
    fn write_ret(&mut self, num_vars: u32) -> Result<()>;
    fn setup_stack_frame(&mut self, num_vars: u32) -> Result<()>;
    fn save_nonvolatile_regs(&mut self) -> Result<()>;
    fn restore_nonvolatile_regs(&mut self) -> Result<()>;
    fn pop(&mut self, reg: Reg) -> Result<()>;
    fn push<I: IntoOperand>(&mut self, operand: I) -> Result<()>;
    fn mov<I: IntoOperand, I2: IntoOperand>(&mut self, op1: I, op2: I2) -> Result<()>;
    fn call<I: IntoOperand>(&mut self, op1: I) -> Result<()>;
    fn get_global(&mut self, globals: *const u32, reg: Reg, global_num: u16) -> Result<()>;
    fn set_global(&mut self, globals: *const u32, reg: Reg, global_num: u16) -> Result<()>;
    fn copy_to_fpu(&mut self, count: usize) -> Result<()>;
    fn copy_to_fpu_rev(&mut self, count: usize) -> Result<()>;
    fn get_global_float(&mut self, globals: *const u32, global_num: u16) -> Result<()>;
    fn set_global_float(&mut self, globals: *const u32, global_num: u16) -> Result<()>;
    fn fstsw_ax(&mut self) -> IoResult<()>;
    fn sahf(&mut self) -> IoResult<()>;
    fn fcompp(&mut self) -> IoResult<()>;
    fn mov_rax_0(&mut self) -> IoResult<()>;
}

static NONVOLATILE_REGS: &[Reg] = &[RBX, RBP, RDI, RSI, R12, R13, R14, R15];

impl<T: Write + Seek> AsmWriterHelper for InstructionWriter<T> {
    fn write_ret(&mut self, num_vars: u32) -> Result<()> {
        let num_vars = num_vars + ((4 - (num_vars % 4)) % 4);
        self.write2(
            MOV,
            Direct(RSP),
            Direct(RBP)
        )?;
        if num_vars > 0 {
            self.write2(
                ADD,
                Direct(RSP),
                Literal32(4 * num_vars)
            )?;
        }
        self.write1(
            POP,
            Direct(RBP)
        )?;
        self.write0(
            RET
        )?;
        Ok(())
    }

    fn setup_stack_frame(&mut self, num_vars: u32) -> Result<()> {
        let num_vars = num_vars + ((4 - (num_vars % 4)) % 4);
        self.write1(
            PUSH,
            Direct(RBP)
        )?;
        if num_vars > 0 {
            self.write2(
                SUB,
                Direct(RSP),
                Literal32(4 * num_vars)
            )?;
        }
        self.write2(
            MOV,
            Direct(RBP),
            Direct(RSP)
        )?;
        Ok(())
    }

    fn save_nonvolatile_regs(&mut self) -> Result<()> {
        for reg in NONVOLATILE_REGS {
            self.write1(PUSH, Direct(*reg))?;
        }
        Ok(())
    }

    fn restore_nonvolatile_regs(&mut self) -> Result<()> {
        for reg in NONVOLATILE_REGS.iter().rev() {
            self.write1(POP, Direct(*reg))?;
        }
        Ok(())
    }

    fn pop(&mut self, reg: Reg) -> Result<()> {
        self.write1(
            POP,
            Direct(reg)
        )?;
        Ok(())
    }
    
    fn push<I: IntoOperand>(&mut self, operand: I) -> Result<()> {
        self.write1(
            PUSH,
            operand.into_op()
        )?;
        Ok(())
    }

    fn mov<I: IntoOperand, I2: IntoOperand>(&mut self, op1: I, op2: I2) -> Result<()> {
        self.write2(
            MOV,
            op1.into_op(),
            op2.into_op()
        )?;
        Ok(())
    }

    fn call<I: IntoOperand>(&mut self, op1: I) -> Result<()> {
        self.write1(
            CALL,
            op1.into_op()
        )?;
        Ok(())
    }
    
    fn get_global(&mut self, globals: *const u32, reg: Reg, global_num: u16) -> Result<()> {
        self.mov(RDX, globals as u64)?;
        self.mov(
            reg,
            (RDX, global_num as u64 * 4, Dword)
        )?;
        Ok(())
    }
    
    fn set_global(&mut self, globals: *const u32, reg: Reg, global_num: u16) -> Result<()> {
        self.mov(RDX, globals as u64)?;
        self.mov(
            (RDX, global_num as u64 * 4, Dword),
            reg
        )?;
        Ok(())
    }

    fn get_global_float(&mut self, globals: *const u32, global_num: u16) -> Result<()> {
        self.mov(RDX, globals as u64)?;
        self.write1(
            FLD,
            (RDX, global_num as u64 * 4, Dword).into_op()
        )?;
        Ok(())
    }

    fn set_global_float(&mut self, globals: *const u32, global_num: u16) -> Result<()> {
        self.mov(RDX, globals as u64)?;
        self.write1(
            FSTP,
            (RDX, global_num as u64 * 4, Dword).into_op()
        ).unwrap();
        Ok(())
    }

    fn copy_to_fpu(&mut self, count: usize) -> Result<()> {
        for i in 1..=count {
            self.write1(
                FLD,
                (RSP, ((count - i) * 8) as u64, Dword).into_op()
            )?;
        }
        Ok(())
    }

    fn copy_to_fpu_rev(&mut self, count: usize) -> Result<()> {
        for i in (1..=count).rev() {
            self.write1(
                FLD,
                (RSP, ((count - i) * 8) as u64, Dword).into_op()
            )?;
        }
        Ok(())
    }

    fn fstsw_ax(&mut self) -> IoResult<()> {
        self.write_bytes(b"\x9B\xDF\xE0")?;
        Ok(())
    }

    fn sahf(&mut self) -> IoResult<()> {
        self.write_bytes(b"\x9e")?;
        Ok(())
    }

    fn fcompp(&mut self) -> IoResult<()> {
        self.write_bytes(b"\xde\xd9")?;
        Ok(())
    }

    fn mov_rax_0(&mut self) -> IoResult<()> {
        self.write_bytes(b"\x48\xb8\xf8\xff\xff\xff\xff\xff\xff\xf8")?;
        Ok(())
    }
}

