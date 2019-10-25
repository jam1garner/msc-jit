use std::io::prelude::*;
use super::{JitMemory, PAGE_SIZE};
use msc::{MscsbFile, Cmd, Script};
use std::io::{Cursor, SeekFrom};
use x86asm::{OperandSize, RegScale, InstructionWriter, Mnemonic, Mode, Operand, Reg};
use libc::c_void;
use std::process::{Command};
use std::collections::{HashSet, HashMap};

mod asm_helper;
use asm_helper::*;
mod printf;
use printf::msc_printf;
mod syscalls;

use Reg::*;
use Operand::*;
use OperandSize::*;
use Mnemonic::*;

pub struct CompiledProgram {
    pub mem: Vec<JitMemory>,
    pub string_section: Vec<u8>,
    pub string_offsets: Vec<*const c_void>,
    pub entrypoint_index: usize,
    pub global_vars: Vec<u32>,
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

static ARG_REGS: [Reg; 6] = [
    RDI, RSI, RDX, RCX, R8, R9
];
static ARG_REGS_32: [Reg; 6] = [
    EDI, ESI, EDX, ECX, R8D, R9D
];

mod asm_macro;
use asm_macro::asm_impl;

impl Compilable for MscsbFile {
    fn compile(&self) -> Option<CompiledProgram> {
        let global_vars = vec![0; 0x100];
        
        let mut string_writer = Cursor::new(Vec::new());
        let mut string_offsets: Vec<usize> = vec![];
        for string in self.strings.iter() {
            string_offsets.push(string_writer.get_ref().len());
            string_writer.write_all(string.as_bytes()).unwrap();
            string_writer.write_all(&[0u8]).unwrap();
        }
        let string_section = string_writer.into_inner();
        let string_offsets = string_offsets.iter().map(
            |offset| unsafe {
                string_section.as_ptr().add(*offset) as *const c_void
            }
        ).collect::<Vec<*const c_void>>();

        let mut mem = vec![];
        let mut call_relocs = vec![];
        for script_index in 0..self.scripts.len() {
            let mut last_cmd_pushint: Option<u32> = None;
            let mut ret_val_locations = HashSet::new();
            let mut jump_relocations = vec![];
            let mut command_locations = HashMap::new();
            // Setup stack frame and whatnot
            let buffer = Cursor::new(Vec::new());
            let mut writer = InstructionWriter::new(buffer, Mode::Long);


            macro_rules! asm {
                (
                    $(
                        $mnem:ident $($op:expr),*;
                    )*
                ) => {
                    asm_impl!(writer, {
                        $(
                            $mnem $($op),*
                        );*
                    })
                };
            }

            if let Some((arg_count, var_count)) = get_var_info(&self.scripts[script_index]) {
                writer.setup_stack_frame(u32::from(var_count)).unwrap();
                for i in 0..std::cmp::min(arg_count, 6) {
                    writer.mov(
                        (RBP, u64::from(i) * 4, Dword),
                        ARG_REGS_32[i as usize]
                    ).unwrap();
                }
                for i in 0..arg_count as isize - 6 {
                    writer.mov(
                        EAX,
                        (RSP, ((i as u64) * 8) + 16 + (var_count + ((4 - (var_count % 4)) % 4)) as u64 * 4, Dword),
                    ).unwrap();
                    writer.mov(
                        (RBP, ((i as u64) + 6) * 4, Dword),
                        EAX
                    ).unwrap();
                }
                for cmd in self.scripts[script_index].iter().skip(1) {
                    if ret_val_locations.contains(&(cmd.position + self.scripts[script_index].bounds.0)) {
                        writer.push(RAX).unwrap();
                    }
                    let command_asm_pos = writer.get_inner_writer_ref().position();
                    command_locations.insert(&cmd.position, command_asm_pos);
                    match cmd.cmd {
                        Cmd::Unk1 | Cmd::ErrorC | Cmd::Error37 | Cmd::Error4C => {
                            panic!("Unsupported command {:?}", cmd.cmd);
                        }
                        Cmd::Begin { .. } => {
                            panic!("Begin not allowed after first command of script");
                        }
                        Cmd::Jump { loc } | Cmd::Jump5 { loc } | Cmd::Else { loc } => {
                            asm!(
                                JMP 0u32;
                            );
                            jump_relocations.push((command_asm_pos, JMP, loc - self.scripts[script_index].bounds.0));
                        }
                        Cmd::Sys { sys_num, arg_count } => {
                            asm!(
                                MOV RDI, RSP;
                                MOV RSI, (u32::from(arg_count));
                                MOV RCX, (syscalls::SYSCALL_TABLE[sys_num as usize] as u64);
                                PUSH R15;
                                MOV R15, RSP;
                                AND R15, 8u8;
                                SUB RSP, R15;
                                CALL RCX;
                                ADD RSP, R15;
                                POP R15;
                                ADD RSP, (8 * arg_count);
                            );
                            if cmd.push_bit {
                                asm!(
                                    PUSH RAX;
                                );
                            }
                        }
                        Cmd::Push => {
                            if cmd.push_bit {
                                asm!(
                                    POP RAX;
                                    PUSH RAX;
                                    PUSH RAX;
                                );
                            }
                        }
                        Cmd::Pop => {
                            if !cmd.push_bit {
                                asm!(
                                    ADD RSP, 8u8;
                                );
                            }
                        }
                        Cmd::If { loc } | Cmd::IfNot { loc } => {
                            asm!(
                                POP RAX;
                                CMP RAX, 0u8;
                            );
                            let command_asm_pos = writer.get_inner_writer_ref().position();
                            let mnem = if let Cmd::If { .. } = cmd.cmd { JE } else { JNE };
                            asm!(
                                mnem 0u32;
                            );
                            jump_relocations.push((
                                command_asm_pos,
                                mnem,
                                loc - self.scripts[script_index].bounds.0
                            ));
                        }
                        Cmd::CallFunc { arg_count } | Cmd::CallFunc2 { arg_count } |
                        Cmd::CallFunc3 { arg_count } => {
                            if let Some(i) = last_cmd_pushint {
                                writer.seek(SeekFrom::Current(-5)).unwrap();
                                if arg_count > 6 {
                                    asm!(
                                        ADD RSP, ((arg_count - 6) * 8);
                                    );
                                }
                                let arg_reg_count = std::cmp::min(arg_count, 6);
                                for i in 0..arg_reg_count {
                                    asm!(
                                        POP ARG_REGS[(arg_reg_count - (i + 1)) as usize];
                                    );
                                }
                                if arg_count > 6 {
                                    asm!(
                                        SUB RSP, ((arg_count - 6) * 8);
                                    );
                                    for i in 0..(arg_count - arg_reg_count) {
                                        asm!(
                                            MOV RAX, (RSP, (u64::from(i) * 8) + (-0x30i64 as u64), Qword);
                                            MOV (RSP, (((arg_count - arg_reg_count) as u64 - (u64::from(i) + 1)) * 8), Qword), RAX;
                                        );
                                    }
                                }
                                let command_asm_pos = writer.get_inner_writer_ref().position();
                                command_locations.insert(&cmd.position, command_asm_pos);
                                call_relocs.push((script_index, command_asm_pos, i));
                                writer.mov_rax_0().unwrap();
                                asm!(
                                    CALL RAX;
                                );
                                if arg_count > 6 {
                                    asm!(
                                        ADD RSP, ((arg_count - 6) * 8);
                                    );
                                }
                            } else {
                                // Dynamically find function pointer
                                // (and cry at the performance impact)
                                panic!("Dynamic function calls not supported");
                            }
                        }
                        Cmd::PushShort { val } => {
                            if cmd.push_bit {
                                asm!(
                                    PUSH (u32::from(val));
                                );
                            }
                        }
                        Cmd::PushInt { val } => {
                            if cmd.push_bit {
                                asm!(
                                    PUSH val;
                                );
                            }
                        }
                        Cmd::IntToFloat { stack_pos } => {
                            asm!(
                                FILD (RSP, u64::from(stack_pos) * 8, Dword);
                                FSTP (RSP, u64::from(stack_pos) * 8, Dword);
                            );
                        }
                        Cmd::FloatToInt { stack_pos } => {
                            asm!(
                                FSTCW (RSP, -2i64 as u64, Word);
                                OR (RSP, -2i64 as u64, Word), 0xc00u16;
                                FLDCW (RSP, -2i64 as u64, Word);
                                FLD (RSP, u64::from(stack_pos) * 8, Dword);
                                FISTP (RSP, u64::from(stack_pos) * 8, Dword);
                            );
                        }
                        Cmd::PushVar { var_type, var_num } => {
                            if var_type == 0 {
                                // Local variable
                                asm!(
                                    MOV EAX, (RBP, u64::from(var_num) * 4, Dword);
                                    PUSH RAX;
                                );
                            } else {
                                // Global variable
                                writer.get_global(global_vars.as_ptr(), EAX, var_num).unwrap();
                                asm!(
                                    PUSH RAX;
                                );
                            }
                        }
                        Cmd::SetVar { var_type, var_num } | Cmd::VarSetF { var_type, var_num } => {
                            if var_type == 0 {
                                // Local var
                                asm!(
                                    POP RAX;
                                    MOV (RBP, u64::from(var_num) * 4, Dword), EAX;
                                );
                            } else {
                                // Global var
                                asm!(
                                    POP RCX;
                                );
                                writer.set_global(global_vars.as_ptr(), ECX, var_num).unwrap();
                            }
                        }
                        Cmd::IncI { var_type, var_num } | Cmd::DecI { var_type, var_num } => {
                            if var_type == 0 {
                                // Local var
                                asm!(
                                    INC (RBP, u64::from(var_num) * 4, Dword);
                                );
                            } else {
                                // Global var
                                writer.get_global(global_vars.as_ptr(), ECX, var_num).unwrap();
                                asm!(
                                    INC RCX;
                                );
                                writer.set_global(global_vars.as_ptr(), ECX, var_num).unwrap();
                            }
                        }
                        Cmd::IncF { var_type, var_num } | Cmd::DecF { var_type, var_num } => {
                            asm!(
                                MOV (RSP, -4i64 as u64, Dword),
                                        if let Cmd::IncF { .. } = cmd.cmd {
                                            1u32
                                        } else {
                                            -1i32 as u32
                                        };
                            );
                            if var_type == 0 {
                                // Local var
                                asm!(
                                    FLD (RBP, u64::from(var_num) * 4, Dword);
                                    FIADD (RSP, -4i64, Dword);
                                    FSTP (RBP, u64::from(var_num) * 4, Dword);
                                );
                            } else {
                                // Global var
                                writer.get_global_float(global_vars.as_ptr(), var_num).unwrap();
                                asm!(
                                    FIADD (RSP, -4i64 as u64, Dword);
                                );
                                writer.set_global_float(global_vars.as_ptr(), var_num).unwrap();
                            }
                        }
                        Cmd::AddVarByF { var_type, var_num } | Cmd::SubVarByF { var_type, var_num } |
                        Cmd::DivVarByF { var_type, var_num } | Cmd::MultVarByF { var_type, var_num }
                        => {
                            if var_type == 0 {
                                // Local var
                                asm!(
                                    FLD (RBP, u64::from(var_num) * 4, Dword);
                                    FADD (RSP, Dword);
                                    FSTP (RBP, u64::from(var_num) * 4, Dword);
                                );
                            } else {
                                // Global var
                                writer.get_global_float(global_vars.as_ptr(), var_num).unwrap();
                                asm!(
                                    FADD (RSP, Dword);
                                );
                                writer.set_global_float(global_vars.as_ptr(), var_num).unwrap();
                            }
                            asm!(
                                ADD RSP, 8u8;
                            );
                        }
                        Cmd::AddVarBy { var_type, var_num } | Cmd::SubVarBy { var_type, var_num } |
                        Cmd::AndVarBy { var_type, var_num } | Cmd::OrVarBy {var_type, var_num} |
                        Cmd::XorVarBy { var_type, var_num } => {
                            asm!(
                                POP RCX;
                            );
                            let operation = match cmd.cmd {
                                Cmd::AddVarBy { .. } => ADD,
                                Cmd::SubVarBy { .. } => SUB,
                                Cmd::AndVarBy { .. } => AND,
                                Cmd::OrVarBy { .. } => OR,
                                Cmd::XorVarBy { .. } => XOR,
                                _ => { unreachable!() }
                            };
                            if var_type == 0 {
                                asm!(
                                    MOV ECX, (RBP, u64::from(var_num) * 4, Dword);
                                    operation ECX, EAX;
                                    MOV (RBP, u64::from(var_num) * 4, Dword), ECX;
                                );
                            } else {
                                writer.get_global(global_vars.as_ptr(), EAX, var_num).unwrap();
                                asm!(
                                    operation EAX, ECX;
                                );
                                writer.set_global(global_vars.as_ptr(), EAX, var_num).unwrap();
                            }
                        }
                        Cmd::MultVarBy { var_type, var_num } | Cmd::DivVarBy { var_type, var_num } |
                        Cmd::ModVarBy { var_type, var_num } => {
                            asm!(
                                POP RCX;
                            );
                            let operation = match cmd.cmd {
                                Cmd::MultVarBy { .. } => IMUL,
                                Cmd::DivVarBy { .. } | Cmd::ModVarBy { .. } => IDIV,
                                _ => { unreachable!() }
                            };
                            if var_type == 0 {
                                asm!(
                                    MOV EAX, (RBP, u64::from(var_num) * 4, Dword);
                                    operation ECX;
                                    MOV 
                                        (RBP, u64::from(var_num) * 4, Dword),
                                        match cmd.cmd {
                                            Cmd::ModVarBy { .. } => EDX,
                                            _ => EAX
                                        };
                                );
                            } else {
                                writer.get_global(global_vars.as_ptr(), EAX, var_num).unwrap();
                                if let IDIV = operation {
                                    asm!(
                                        MOV EDX, 0u32;
                                    );
                                }
                                asm!(
                                    operation ECX;
                                );
                                if let Cmd::ModVarBy { .. } = cmd.cmd {
                                    asm!(
                                        MOV EAX, EDX;
                                    );
                                }
                                writer.set_global(global_vars.as_ptr(), EAX, var_num).unwrap();
                            }
                        }
                        Cmd::MultI | Cmd::DivI | Cmd::ModI => {
                            asm!(
                                POP RCX;
                                POP RAX;
                            );
                            if cmd.push_bit {
                                let op = match cmd.cmd {
                                    Cmd::MultI => IMUL,
                                    Cmd::DivI | Cmd::ModI => {
                                        asm!(
                                            MOV EDX, 0u32;
                                        );
                                        IDIV
                                    },
                                    _ => { unreachable!() }
                                };
                                asm!(
                                    op ECX;
                                    PUSH if let Cmd::ModI = cmd.cmd { RDX } else { RAX };
                                );
                            }
                        }
                        Cmd::AddI | Cmd::SubI | Cmd::ShiftL | Cmd::ShiftR | Cmd::AndI | Cmd::OrI |
                        Cmd::XorI => {
                            asm!(
                                POP RCX;
                                POP RAX;
                            );
                            if cmd.push_bit {
                                let op = match cmd.cmd {
                                            Cmd::AddI => ADD,
                                            Cmd::SubI => SUB,
                                            Cmd::ShiftR => SHR,
                                            Cmd::ShiftL => SHL,
                                            Cmd::AndI => AND,
                                            Cmd::OrI => OR,
                                            Cmd::XorI => XOR,
                                            _ => { unreachable!() }
                                        };
                                asm!(
                                    op EAX, match cmd.cmd {
                                                Cmd::ShiftR | Cmd::ShiftL => { CL }
                                                _ => { ECX }
                                            };
                                    PUSH RAX;
                                );
                            }
                        }
                        Cmd::Equals | Cmd::NotEquals | Cmd::LessThan | Cmd::LessOrEqual |
                        Cmd::Greater | Cmd::GreaterOrEqual => {
                            asm!(
                                POP RAX;
                                POP RCX;
                            );
                            if cmd.push_bit {
                                let (op, op_inverse) = match cmd.cmd {
                                    Cmd::Equals => (CMOVE, CMOVNE),
                                    Cmd::NotEquals => (CMOVNE, CMOVE),
                                    Cmd::LessThan => (CMOVL, CMOVGE),
                                    Cmd::LessOrEqual => (CMOVLE, CMOVG),
                                    Cmd::Greater => (CMOVG, CMOVLE),
                                    Cmd::GreaterOrEqual => (CMOVGE, CMOVL),
                                    _ => { unreachable!() }
                                };
                                asm!(
                                    XOR R8, R8;
                                    MOV EDX, 1u32;
                                    CMP ECX, EAX;
                                    op EAX, EDX;
                                    op_inverse EAX, R8D;
                                    PUSH RAX;
                                );
                            }
                        }
                        Cmd::EqualsF | Cmd::NotEqualsF | Cmd::LessThanF | Cmd::LessOrEqualF |
                        Cmd::GreaterF | Cmd::GreaterOrEqualF => {
                            if cmd.push_bit {
                                writer.copy_to_fpu_rev(2).unwrap();
                                asm!(
                                    MOV EDX, 1u32;
                                );
                                writer.fcompp().unwrap();
                                writer.fstsw_ax().unwrap();
                                asm!(
                                    FWAIT;
                                );
                                writer.sahf().unwrap();
                                let (op, op_inverse) = match cmd.cmd {
                                    Cmd::EqualsF => (CMOVE, CMOVNE),
                                    Cmd::NotEqualsF => (CMOVNE, CMOVE),
                                    Cmd::LessThanF => (CMOVB, CMOVAE),
                                    Cmd::LessOrEqualF => (CMOVBE, CMOVA),
                                    Cmd::GreaterF => (CMOVA, CMOVBE),
                                    Cmd::GreaterOrEqualF => (CMOVAE, CMOVB),
                                    _ => { unreachable!() }
                                };
                                asm!(
                                    op EAX, EDX;
                                    op_inverse EAX, R8D;
                                    ADD RSP, 16u8;
                                    PUSH RAX;
                                );
                            }
                        }
                        Cmd::NegI | Cmd::NotI => {
                            if cmd.push_bit {
                                let op = match cmd.cmd {
                                            Cmd::NegI => NEG,
                                            Cmd::NotI => NOT,
                                            _ => { unreachable!() }
                                        };
                                asm!(
                                    op (RSP, Dword);
                                );
                            } else {
                                asm!(
                                    POP RAX;
                                );
                            }
                        }
                        Cmd::NegF => {
                            writer.copy_to_fpu(1).unwrap();
                            asm!(
                                FCHS;
                                FSTP (RSP, Dword);
                            );
                        }
                        Cmd::Not => {
                            asm!(
                                POP RAX;
                            );
                            if cmd.push_bit {
                                asm!(
                                    XOR R8, R8;
                                    MOV EDX, 1u32;
                                    TEST RAX, RAX;
                                    CMOVE RAX, RDX;
                                    CMOVNZ RAX, R8;
                                );
                            }
                        }
                        Cmd::AddF | Cmd::SubF | Cmd::MultF | Cmd::DivF => {
                            if cmd.push_bit {
                                writer.copy_to_fpu(2).unwrap();
                                let op = match cmd.cmd {
                                            Cmd::AddF => FADD,
                                            Cmd::SubF => FSUB,
                                            Cmd::MultF => FMUL,
                                            Cmd::DivF => FDIV,
                                            _ => { unreachable!() }
                                        };
                                asm!(
                                    op ST, ST1;
                                    ADD RSP, 8u8;
                                    FSTP (RSP, Dword);
                                    FSTP ST0;
                                );
                            } else {
                                asm!(
                                    ADD RSP, 0x10u8;
                                );
                            }
                        }
                        Cmd::PrintF { arg_count } => {
                            if arg_count == 0 {
                                println!("WARNING: printf arg_count cannot be 0");
                                continue;
                            }
                            asm!(
                                MOV RSI, RSP;
                                MOV RAX, (RSP, 8 * (u64::from(arg_count) - 1), Qword);
                                MOV RDX, (u64::from(arg_count) - 1);
                                MOV RDI, (string_offsets.as_ptr() as u64);
                                MOV RDI, (RDI, RAX, RegScale::Eight, Qword);
                                MOV RCX, (msc_printf as u64);
                                PUSH R15;
                                MOV R15, RSP;
                                AND R15, 8u8;
                                SUB RSP, R15;
                                CALL RCX;
                                ADD RSP, R15;
                                POP R15;
                                ADD RSP, (8 * arg_count);
                            );
                        }
                        Cmd::Try { loc } => {
                            if cmd.push_bit {
                                ret_val_locations.insert(loc);
                            }
                        }
                        Cmd::Return6 | Cmd::Return8 => {
                            asm!(
                                POP RAX;
                            );
                            writer.write_ret(u32::from(var_count)).unwrap();
                        }
                        Cmd::Return7 | Cmd::Return9 | Cmd::End => {
                            writer.write_ret(u32::from(var_count)).unwrap();
                        }
                        Cmd::Exit => {
                            asm!(
                                MOV EAX, 60u32;
                                XOR EDI, EDI;
                                SYSCALL;
                            );
                        }
                        Cmd::Nop => {}
                    }
                    last_cmd_pushint = match cmd.cmd {
                        Cmd::PushInt { val } => {
                            Some(val)
                        }
                        Cmd::PushShort { val } => {
                            Some(u32::from(val))
                        }
                        _ => {
                            None
                        }
                    };
                }
                //writer.write_ret(u32::from(var_count)).unwrap();
                for relocation in jump_relocations {
                    writer.seek(SeekFrom::Start(relocation.0)).unwrap();
                    writer.write1(
                        relocation.1,
                        Literal32(
                            (*command_locations.get(&relocation.2).unwrap() as i64
                             - relocation.0 as i64
                             - match relocation.1 {
                                JMP => 5,
                                JE => 6,
                                JNE => 6,
                                _ => { unreachable!() }
                             })
                            as u32
                        )
                    ).unwrap();
                }
            } else {
                asm!(
                    RET;
                );
            }
            let buffer = writer.get_inner_writer_ref().get_ref();
            println!("\n\nEmitted asm:");
            objdump(&buffer);
            let mut code = JitMemory::new((buffer.len() + (PAGE_SIZE - 1)) / PAGE_SIZE);
            unsafe {
                code.as_slice()[..buffer.len()].copy_from_slice(&buffer[..]);
            }
            mem.push(code);
        }

        for (script_index, pos, script_offset) in call_relocs {
            let call_addr = mem[self.get_script_from_loc(script_offset).unwrap()].contents as u64;
            #[allow(clippy::cast_ptr_alignment)]
            unsafe {
                *(mem[script_index].contents.offset(pos as isize + 2) as *mut u64) = call_addr;
            }
        }
        
        //println!("\n\nEmitted asm:");
        
        /*for i in 0..mem.len() {
            unsafe {
                objdump(std::slice::from_raw_parts(mem[i].contents, mem[i].size));
            }
        }*/

        let entrypoint_index = self.get_script_from_loc(self.entrypoint)?;

        //println!("{}", buffer.iter().map(|b| format!("{:02X}", b)).collect::<Vec<String>>().join(" "));
        println!("\n\n\n");
        Some(CompiledProgram {
            mem, entrypoint_index,
            string_section, string_offsets, global_vars
        })
    }
}

fn objdump(buffer: &[u8]) {
    std::fs::File::create("/tmp/msc-jit-temp.bin").unwrap()
        .write_all(buffer).unwrap();
    // objdump -D -b binary -mi386 -Maddr16,data16,x86-64,intel /dev/stdin
    let output = Command::new("objdump")
        .args(&["-D", "-b", "binary", "-mi386", "-Maddr16,data16,x86-64,intel", "/tmp/msc-jit-temp.bin"])
        .output()
        .unwrap();
    std::fs::remove_file("/tmp/msc-jit-temp.bin").ok();
    println!("{}\n", &String::from_utf8_lossy(&output.stdout).trim()[97..]);
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
            // Flush printf buffer
            libc::printf("\n\0".as_ptr() as _);
            println!("Return value - 0x{:X}", ret);
        }
    }

    pub fn get_entrypoint_address(&self) -> u64 {
        self.mem[self.entrypoint_index].contents as u64
    }
}
