use x86asm::{OperandSize, RegScale, Operand, Reg};

use Operand::*;

pub trait IntoOperand {
    fn into_op(self) -> Operand;
}

impl IntoOperand for u8 {
    fn into_op(self) -> Operand {
        Literal8(self)
    }
}

impl IntoOperand for u16 {
    fn into_op(self) -> Operand {
        Literal16(self)
    }
}

impl IntoOperand for u32 {
    fn into_op(self) -> Operand {
        Literal32(self)
    }
}

impl IntoOperand for u64 {
    fn into_op(self) -> Operand {
        Literal64(self)
    }
}

impl IntoOperand for Reg {
    fn into_op(self) -> Operand {
        Direct(self)
    }
}

impl IntoOperand for (Reg, OperandSize) {
    fn into_op(self) -> Operand {
        Indirect(self.0, Some(self.1), None)
    }
}

impl IntoOperand for (Reg, u64, OperandSize) {
    fn into_op(self) -> Operand {
        if self.1 == 0 {
            Indirect(self.0, Some(self.2), None)
        } else {
            IndirectDisplaced(self.0, self.1, Some(self.2), None)
        }
    }
}

impl IntoOperand for (Reg, i64, OperandSize) {
    fn into_op(self) -> Operand {
        IntoOperand::into_op((self.0, self.1 as u64, self.2))
    }
}

impl IntoOperand for (Reg, Reg, RegScale, OperandSize) {
    fn into_op(self) -> Operand {
        IndirectScaledIndexed(self.0, self.1, self.2, Some(self.3), None)
    }
}

impl IntoOperand for (Reg, Reg, RegScale, u64, OperandSize) {
    fn into_op(self) -> Operand {
        IndirectScaledIndexedDisplaced(self.0, self.1, self.2, self.3, Some(self.4), None)
    }
}

impl IntoOperand for Operand {
    fn into_op(self) -> Operand {
        self
    }
}
