use x86asm::{OperandSize, RegScale, InstructionEncodingError, InstructionWriter, Mnemonic, Mode, Operand, Reg};

pub trait IntoOperand {
    fn into_op(self) -> Operand;
}

impl IntoOperand for u8 {
    fn into_op(self) -> Operand {
        Operand::Literal8(self)
    }
}

impl IntoOperand for u16 {
    fn into_op(self) -> Operand {
        Operand::Literal16(self)
    }
}

impl IntoOperand for u32 {
    fn into_op(self) -> Operand {
        Operand::Literal32(self)
    }
}

impl IntoOperand for u64 {
    fn into_op(self) -> Operand {
        Operand::Literal64(self)
    }
}

impl IntoOperand for Reg {
    fn into_op(self) -> Operand {
        Operand::Direct(self)
    }
}

impl IntoOperand for (Reg, OperandSize) {
    fn into_op(self) -> Operand {
        Operand::Indirect(self.0, Some(self.1), None)
    }
}

impl IntoOperand for (Reg, u64, OperandSize) {
    fn into_op(self) -> Operand {
        if self.1 == 0 {
            Operand::Indirect(self.0, Some(self.2), None)
        } else {
            Operand::IndirectDisplaced(self.0, self.1, Some(self.2), None)
        }
    }
}

impl IntoOperand for (Reg, Reg, RegScale, OperandSize) {
    fn into_op(self) -> Operand {
        Operand::IndirectScaledIndexed(self.0, self.1, self.2, Some(self.3), None)
    }
}

impl IntoOperand for (Reg, Reg, RegScale, u64, OperandSize) {
    fn into_op(self) -> Operand {
        Operand::IndirectScaledIndexedDisplaced(self.0, self.1, self.2, self.3, Some(self.4), None)
    }
}

impl IntoOperand for Operand {
    fn into_op(self) -> Operand {
        self
    }
}
