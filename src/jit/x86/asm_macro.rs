#[macro_export] macro_rules! asm_impl {
    (
        $writer:ident,
        {
            $(
                $mnem:ident $($op:expr),*
            );*
        }
    ) => {
        $(
            asm_impl!(@asm
                      $writer,
                      $mnem 
                      $(, crate::jit::x86::asm_helper::IntoOperand::into_op($op))*
            );
        )*
    };

    (
        @asm $writer:ident,
        $mnem:ident
    ) => {
        $writer.write0($mnem).unwrap();
    };

    (
        @asm $writer:ident,
        $mnem:ident, $op:expr
    ) => {
        $writer.write1($mnem, $op).unwrap();
    };

    (
        @asm $writer:ident,
        $mnem:ident, $op:expr, $op2:expr
    ) => {
        $writer.write2($mnem, $op, $op2).unwrap();
    };

    (
        @asm $writer:ident,
        $mnem:ident, $op:expr, $op2:expr, $op3:expr
    ) => {
        $writer.write3($mnem, $op, $op2, $op3).unwrap();
    };
}

pub use asm_impl;
