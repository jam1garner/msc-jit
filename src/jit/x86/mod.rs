use std::io::prelude::*;
use super::{JitMemory, PAGE_SIZE};
use msc::{MscsbFile, Cmd, Script};
use std::io::{Cursor, SeekFrom};
use x86asm::{OperandSize, RegScale, InstructionEncodingError, InstructionWriter, Mnemonic, Mode, Operand, Reg};
use libc::c_void;
use std::process::{Command};
use std::collections::{HashSet, HashMap};

mod asm_helper;
use asm_helper::*;
mod printf;
use printf::msc_printf;

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
    Reg::RDI, Reg::RSI, Reg::RDX, Reg::RCX, Reg::R8, Reg::R9
];

impl Compilable for MscsbFile {
    fn compile(&self) -> Option<CompiledProgram> {
        let global_vars = vec![0; 0x100];
        
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
            if let Some((_, var_count)) = get_var_info(&self.scripts[script_index]) {
                writer.setup_stack_frame(var_count as u32).ok()?;
                for cmd in self.scripts[script_index].iter().skip(1) {
                    if ret_val_locations.contains(&(cmd.position + self.scripts[script_index].bounds.0)) {
                        writer.push(Reg::RAX).ok()?;
                    }
                    let command_asm_pos = writer.get_inner_writer_ref().position();
                    command_locations.insert(&cmd.position, command_asm_pos);
                    match cmd.cmd {
                        Cmd::Begin { arg_count: _, var_count: _ } => {
                            panic!("Begin not allowed after first command of script");
                        }
                        Cmd::Jump { loc } | Cmd::Jump5 { loc } | Cmd::Else { loc } => {
                            writer.write1(
                                Mnemonic::JMP,
                                Operand::Literal32(0)
                            ).unwrap();
                            jump_relocations.push((command_asm_pos, Mnemonic::JMP, loc - self.scripts[script_index].bounds.0));
                        }
                        Cmd::If { loc } | Cmd::IfNot { loc } => {
                            writer.pop(Reg::RAX).ok()?;
                            writer.write2(
                                Mnemonic::CMP,
                                Operand::Direct(Reg::RAX),
                                Operand::Literal8(0)
                            ).unwrap();
                            let command_asm_pos = writer.get_inner_writer_ref().position();
                            let mnem = if let Cmd::If { loc: _ } = cmd.cmd {
                                Mnemonic::JE
                            } else {
                                Mnemonic::JNE
                            };
                            writer.write1(
                                mnem,
                                Operand::Literal32(0)
                            ).unwrap();
                            jump_relocations.push((command_asm_pos, mnem, loc - self.scripts[script_index].bounds.0));
                        }
                        Cmd::CallFunc { arg_count } | Cmd::CallFunc2 { arg_count } |
                        Cmd::CallFunc3 { arg_count } => {
                            for i in 0..arg_count {
                                writer.pop(ARG_REGS[(arg_count - (i + 1)) as usize]).unwrap();
                            }
                            if let Some(i) = last_cmd_pushint {
                                writer.seek(SeekFrom::Current(-5)).unwrap();
                                let command_asm_pos = writer.get_inner_writer_ref().position();
                                command_locations.insert(&cmd.position, command_asm_pos);
                                call_relocs.push((script_index, command_asm_pos, i));
                                writer.mov(Reg::RDI, 0u64).unwrap();
                                writer.call(Reg::RDI).ok()?;
                            } else {
                                // Dynamically find function pointer
                                // (and cry at the performance impact)
                                panic!("Dynamic function calls not supported");
                            }
                        }
                        Cmd::PushShort { val } => {
                            if cmd.push_bit {
                                writer.push(val as u32).ok()?;
                            }
                        }
                        Cmd::PushInt { val } => {
                            if cmd.push_bit {
                                writer.push(val).ok()?;
                            }
                        }
                        Cmd::IntToFloat { stack_pos } => {
                            writer.write1(
                                Mnemonic::FILD,
                                (Reg::RSP, stack_pos as u64 * 8, OperandSize::Dword).into_op()
                            ).unwrap();
                            writer.write1(
                                Mnemonic::FSTP,
                                (Reg::RSP, stack_pos as u64 * 8, OperandSize::Dword).into_op()
                            ).unwrap();
                        }
                        Cmd::FloatToInt { stack_pos } => {
                            writer.write1(
                                Mnemonic::FSTCW,
                                (Reg::RSP, -2i64 as u64, OperandSize::Word).into_op()
                            ).unwrap();
                            writer.write2(
                                Mnemonic::OR,
                                (Reg::RSP, -2i64 as u64, OperandSize::Word).into_op(),
                                Operand::Literal16(0xc00)
                            ).unwrap();
                            writer.write1(
                                Mnemonic::FLDCW,
                                (Reg::RSP, -2i64 as u64, OperandSize::Word).into_op()
                            ).unwrap();
                            writer.write1(
                                Mnemonic::FLD,
                                (Reg::RSP, stack_pos as u64 * 8, OperandSize::Dword).into_op()
                            ).unwrap();
                            writer.write1(
                                Mnemonic::FISTP,
                                (Reg::RSP, stack_pos as u64 * 8, OperandSize::Dword).into_op()
                            ).unwrap();
                        }
                        Cmd::PushVar { var_type, var_num } => {
                            if var_type == 0 {
                                // Local variable
                                writer.mov(
                                    Reg::EAX,
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword)
                                ).unwrap();
                                writer.push(Reg::RAX).ok()?;
                            } else {
                                // Global variable
                                writer.get_global(global_vars.as_ptr(), Reg::EAX, var_num).unwrap();
                                writer.push(Reg::RAX).ok()?;
                            }
                        }
                        Cmd::SetVar { var_type, var_num } | Cmd::VarSetF { var_type, var_num } => {
                            if var_type == 0 {
                                // Local var
                                writer.pop(Reg::RAX).ok()?;
                                writer.mov(
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword),
                                    Reg::EAX
                                ).unwrap();
                            } else {
                                // Global var
                                writer.pop(Reg::RCX).ok()?;
                                writer.set_global(global_vars.as_ptr(), Reg::ECX, var_num).unwrap();
                            }
                        }
                        Cmd::IncI { var_type, var_num } | Cmd::DecI { var_type, var_num } => {
                            if var_type == 0 {
                                // Local var
                                writer.write1(
                                    Mnemonic::INC,
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword).into_op()
                                ).unwrap();
                            } else {
                                // Global var
                                writer.get_global(global_vars.as_ptr(), Reg::ECX, var_num).unwrap();
                                writer.write1(
                                    Mnemonic::INC,
                                    Operand::Direct(Reg::RCX)
                                ).unwrap();
                                writer.set_global(global_vars.as_ptr(), Reg::ECX, var_num).unwrap();
                            }
                        }
                        Cmd::IncF { var_type, var_num } | Cmd::DecF { var_type, var_num } => {
                            writer.mov(
                                (Reg::RSP, -4i64 as u64, OperandSize::Dword),
                                if let Cmd::IncF { var_type: _, var_num: _ } = cmd.cmd {
                                    1u32
                                } else {
                                    -1i32 as u32
                                }
                            ).unwrap();
                            if var_type == 0 {
                                // Local var
                                writer.write1(
                                    Mnemonic::FLD,
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword).into_op()
                                ).unwrap();
                                writer.write1(
                                    Mnemonic::FIADD,
                                    (Reg::RSP, -4i64 as u64, OperandSize::Dword).into_op()
                                ).unwrap();
                                writer.write1(
                                    Mnemonic::FSTP,
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword).into_op()
                                ).unwrap();
                            } else {
                                // Global var
                                writer.get_global_float(global_vars.as_ptr(), var_num).unwrap();
                                writer.write1(
                                    Mnemonic::FIADD,
                                    (Reg::RSP, -4i64 as u64, OperandSize::Dword).into_op()
                                ).unwrap();
                                writer.set_global_float(global_vars.as_ptr(), var_num).unwrap();
                            }
                        }
                        Cmd::AddVarByF { var_type, var_num } | Cmd::SubVarByF { var_type, var_num } |
                        Cmd::DivVarByF { var_type, var_num } | Cmd::MultVarByF { var_type, var_num }
                        => {
                            if var_type == 0 {
                                // Local var
                                writer.write1(
                                    Mnemonic::FLD,
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword).into_op()
                                ).unwrap();
                                writer.write1(
                                    Mnemonic::FADD,
                                    (Reg::RSP, OperandSize::Dword).into_op()
                                ).unwrap();
                                writer.write1(
                                    Mnemonic::FSTP,
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword).into_op()
                                ).unwrap();
                            } else {
                                // Global var
                                writer.get_global_float(global_vars.as_ptr(), var_num).unwrap();
                                writer.write1(
                                    Mnemonic::FADD,
                                    (Reg::RSP, OperandSize::Dword).into_op()
                                ).unwrap();
                                writer.set_global_float(global_vars.as_ptr(), var_num).unwrap();
                            }
                            writer.write2(
                                Mnemonic::ADD,
                                Operand::Direct(Reg::RSP),
                                Operand::Literal8(0x8)
                            ).unwrap();
                        }
                        Cmd::AddVarBy { var_type, var_num } | Cmd::SubVarBy { var_type, var_num } |
                        Cmd::AndVarBy { var_type, var_num } | Cmd::OrVarBy {var_type, var_num} |
                        Cmd::XorVarBy { var_type, var_num } => {
                            writer.pop(Reg::RCX).ok()?;
                            let operation = match cmd.cmd {
                                Cmd::AddVarBy { var_type: _, var_num: _ } => Mnemonic::ADD,
                                Cmd::SubVarBy { var_type: _, var_num: _ } => Mnemonic::SUB,
                                Cmd::AndVarBy { var_type: _, var_num: _ } => Mnemonic::AND,
                                Cmd::OrVarBy { var_type: _, var_num: _ } => Mnemonic::OR,
                                Cmd::XorVarBy { var_type: _, var_num: _ } => Mnemonic::XOR,
                                _ => { unreachable!() }
                            };
                            if var_type == 0 {
                                writer.mov(
                                    Reg::ECX,
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword)
                                ).ok()?;
                                writer.write2(
                                    operation,
                                    Operand::Direct(Reg::ECX),
                                    Operand::Direct(Reg::EAX),
                                ).unwrap();
                                writer.mov(
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword),
                                    Reg::ECX
                                ).ok()?;
                            } else {
                                writer.get_global(global_vars.as_ptr(), Reg::EAX, var_num).unwrap();
                                writer.write2(
                                    operation,
                                    Operand::Direct(Reg::EAX),
                                    Operand::Direct(Reg::ECX)
                                ).unwrap();
                                writer.set_global(global_vars.as_ptr(), Reg::EAX, var_num).unwrap();
                            }
                        }
                        Cmd::MultVarBy { var_type, var_num } | Cmd::DivVarBy { var_type, var_num } |
                        Cmd::ModVarBy { var_type, var_num } => {
                            writer.pop(Reg::RCX).ok()?;
                            let operation = match cmd.cmd {
                                Cmd::MultVarBy { var_type: _, var_num: _ }
                                    => Mnemonic::IMUL,
                                Cmd::DivVarBy { var_type: _, var_num: _ } |
                                Cmd::ModVarBy { var_type: _, var_num: _ }
                                    => Mnemonic::IDIV,
                                _ => { unreachable!() }
                            };
                            if var_type == 0 {
                                writer.mov(
                                    Reg::EAX,
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword)
                                ).ok()?;
                                writer.write1(
                                    operation,
                                    Operand::Direct(Reg::ECX),
                                ).unwrap();
                                writer.mov(
                                    (Reg::RBP, var_num as u64 * 4, OperandSize::Dword),
                                    match cmd.cmd {
                                        Cmd::ModVarBy { var_type: _, var_num: _ } => Reg::EDX,
                                        _ => Reg::EAX
                                    },
                                ).ok()?;
                            } else {
                                writer.get_global(global_vars.as_ptr(), Reg::EAX, var_num).unwrap();
                                if let Mnemonic::IDIV = operation {
                                    writer.mov(Reg::EDX, 0u32).unwrap();
                                }
                                writer.write1(
                                    operation,
                                    Operand::Direct(Reg::ECX)
                                ).unwrap();
                                if let Cmd::ModVarBy { var_type: _, var_num: _ } = cmd.cmd {
                                    writer.mov(Reg::EAX, Reg::EDX).unwrap();
                                }
                                writer.set_global(global_vars.as_ptr(), Reg::EAX, var_num).unwrap();
                            }
                        }
                        Cmd::MultI | Cmd::DivI | Cmd::ModI => {
                            writer.pop(Reg::RCX).ok()?;
                            writer.pop(Reg::RAX).ok()?;
                            if cmd.push_bit {
                                let op = match cmd.cmd {
                                    Cmd::MultI => Mnemonic::IMUL,
                                    Cmd::DivI | Cmd::ModI => {
                                        writer.mov(Reg::EDX, 0u32).unwrap();
                                        Mnemonic::IDIV
                                    },
                                    _ => { unreachable!() }
                                };
                                writer.write1(
                                    op,
                                    Operand::Direct(Reg::ECX)
                                ).ok()?;
                                writer.push(
                                    if let Cmd::ModI = cmd.cmd {
                                        Reg::RDX
                                    } else {
                                        Reg::RAX
                                    }
                                ).ok()?;
                            }
                        }
                        Cmd::AddI | Cmd::SubI | Cmd::ShiftL | Cmd::ShiftR | Cmd::AndI | Cmd::OrI |
                        Cmd::XorI => {
                            writer.pop(Reg::RCX).ok()?;
                            writer.pop(Reg::RAX).ok()?;
                            if cmd.push_bit {
                                writer.write2(
                                    match cmd.cmd {
                                        Cmd::AddI => Mnemonic::ADD,
                                        Cmd::SubI => Mnemonic::SUB,
                                        Cmd::ShiftR => Mnemonic::SHR,
                                        Cmd::ShiftL => Mnemonic::SHL,
                                        Cmd::AndI => Mnemonic::AND,
                                        Cmd::OrI => Mnemonic::OR,
                                        Cmd::XorI => Mnemonic::XOR,
                                        _ => { unreachable!() }
                                    },
                                    Operand::Direct(Reg::EAX),
                                    Operand::Direct(Reg::ECX)
                                ).ok()?;
                                writer.push(Reg::RAX).ok()?;
                            }
                        }
                        Cmd::Equals | Cmd::NotEquals | Cmd::LessThan | Cmd::LessOrEqual |
                        Cmd::Greater | Cmd::GreaterOrEqual => {
                            writer.pop(Reg::RAX).ok()?;
                            writer.pop(Reg::RCX).ok()?;
                            if cmd.push_bit {
                                writer.write2(
                                    Mnemonic::XOR,
                                    Operand::Direct(Reg::R8),
                                    Operand::Direct(Reg::R8)
                                ).unwrap();
                                writer.mov(Reg::EDX, 1u32).unwrap();
                                writer.write2(
                                    Mnemonic::CMP,
                                    Operand::Direct(Reg::ECX),
                                    Operand::Direct(Reg::EAX)
                                ).unwrap();
                                let ops = match cmd.cmd {
                                    Cmd::Equals => (Mnemonic::CMOVE, Mnemonic::CMOVNE),
                                    Cmd::NotEquals => (Mnemonic::CMOVNE, Mnemonic::CMOVE),
                                    Cmd::LessThan => (Mnemonic::CMOVL, Mnemonic::CMOVGE),
                                    Cmd::LessOrEqual => (Mnemonic::CMOVLE, Mnemonic::CMOVG),
                                    Cmd::Greater => (Mnemonic::CMOVG, Mnemonic::CMOVLE),
                                    Cmd::GreaterOrEqual => (Mnemonic::CMOVGE, Mnemonic::CMOVL),
                                    _ => { unreachable!() }
                                };
                                writer.write2(
                                    ops.0,
                                    Operand::Direct(Reg::EAX),
                                    Operand::Direct(Reg::EDX)
                                ).unwrap();
                                writer.write2(
                                    ops.1,
                                    Operand::Direct(Reg::EAX),
                                    Operand::Direct(Reg::R8D)
                                ).unwrap();
                                writer.push(Reg::RAX).unwrap();
                            }
                        }
                        Cmd::EqualsF | Cmd::NotEqualsF | Cmd::LessThanF | Cmd::LessOrEqualF |
                        Cmd::GreaterF | Cmd::GreaterOrEqualF => {
                            if cmd.push_bit {
                                writer.copy_to_fpu_rev(2).ok()?;
                                writer.mov(Reg::EDX, 1u32).unwrap();
                                writer.fcompp().unwrap();
                                writer.fstsw_ax().unwrap();
                                writer.write0(
                                    Mnemonic::FWAIT
                                ).unwrap();
                                writer.sahf().unwrap();
                                let ops = match cmd.cmd {
                                    Cmd::EqualsF => (Mnemonic::CMOVE, Mnemonic::CMOVNE),
                                    Cmd::NotEqualsF => (Mnemonic::CMOVNE, Mnemonic::CMOVE),
                                    Cmd::LessThanF => (Mnemonic::CMOVB, Mnemonic::CMOVAE),
                                    Cmd::LessOrEqualF => (Mnemonic::CMOVBE, Mnemonic::CMOVA),
                                    Cmd::GreaterF => (Mnemonic::CMOVA, Mnemonic::CMOVBE),
                                    Cmd::GreaterOrEqualF => (Mnemonic::CMOVAE, Mnemonic::CMOVB),
                                    _ => { unreachable!() }
                                };
                                writer.write2(
                                    ops.0,
                                    Operand::Direct(Reg::EAX),
                                    Operand::Direct(Reg::EDX)
                                ).unwrap();
                                writer.write2(
                                    ops.1,
                                    Operand::Direct(Reg::EAX),
                                    Operand::Direct(Reg::R8D)
                                ).unwrap();
                                writer.write2(
                                    Mnemonic::ADD,
                                    Operand::Direct(Reg::RSP),
                                    Operand::Literal8(16)
                                ).unwrap();
                                writer.push(Reg::RAX).unwrap();
                            }
                        }
                        Cmd::NegI | Cmd::NotI => {
                            if cmd.push_bit {
                                writer.write1(
                                    match cmd.cmd {
                                        Cmd::NegI => Mnemonic::NEG,
                                        Cmd::NotI => Mnemonic::NOT,
                                        _ => { unreachable!() }
                                    },
                                    (Reg::RSP, OperandSize::Dword).into_op()
                                ).ok()?;
                            } else {
                                writer.pop(Reg::RAX).ok()?;
                            }
                        }
                        Cmd::Not => {
                            writer.pop(Reg::RAX).ok()?;
                            if cmd.push_bit {
                                writer.write2(
                                    Mnemonic::XOR,
                                    Operand::Direct(Reg::R8),
                                    Operand::Direct(Reg::R8)
                                ).unwrap();
                                writer.mov(Reg::EDX, 1u32).unwrap();
                                writer.write2(
                                    Mnemonic::TEST,
                                    Operand::Direct(Reg::RAX),
                                    Operand::Direct(Reg::RAX)
                                ).unwrap();
                                writer.write2(
                                    Mnemonic::CMOVE,
                                    Operand::Direct(Reg::RAX),
                                    Operand::Direct(Reg::RDX)
                                ).unwrap();
                                writer.write2(
                                    Mnemonic::CMOVNZ,
                                    Operand::Direct(Reg::RAX),
                                    Operand::Direct(Reg::R8)
                                ).unwrap();
                            }
                        }
                        Cmd::AddF | Cmd::SubF | Cmd::MultF | Cmd::DivF => {
                            if cmd.push_bit {
                                writer.copy_to_fpu(2).unwrap();
                                writer.write2(
                                    match cmd.cmd {
                                        Cmd::AddF => Mnemonic::FADD,
                                        Cmd::SubF => Mnemonic::FSUB,
                                        Cmd::MultF => Mnemonic::FMUL,
                                        Cmd::DivF => Mnemonic::FDIV,
                                        _ => { unreachable!() }
                                    },
                                    Operand::Direct(Reg::ST),
                                    Operand::Direct(Reg::ST1)
                                ).unwrap();
                                writer.write2(
                                    Mnemonic::ADD,
                                    Operand::Direct(Reg::RSP),
                                    Operand::Literal8(0x8)
                                ).unwrap();
                                writer.write1(
                                    Mnemonic::FSTP,
                                    (Reg::RSP, OperandSize::Dword).into_op()
                                ).unwrap();
                                writer.write1(
                                    Mnemonic::FSTP,
                                    Operand::Direct(Reg::ST0)
                                ).unwrap();
                            } else {
                                writer.write2(
                                    Mnemonic::ADD,
                                    Operand::Direct(Reg::RSP),
                                    Operand::Literal8(0x10)
                                ).unwrap();
                            }
                        }
                        Cmd::PrintF { arg_count } => {
                            if arg_count == 0 {
                                println!("WARNING: printf arg_count cannot be 0");
                                continue;
                            }
                            writer.mov(Reg::RSI, Reg::RSP).ok()?;
                            writer.write2(
                                Mnemonic::ADD,
                                Operand::Direct(Reg::RSP),
                                Operand::Literal8(8 * (arg_count - 1))
                            ).unwrap();
                            writer.pop(Reg::RAX).ok()?;
                            writer.mov(Reg::RDX, arg_count as u64 - 1 ).ok()?;
                            writer.mov(Reg::RDI, string_offsets.as_ptr() as u64).ok()?;
                            writer.mov(
                                Reg::RDI,
                                (Reg::RDI, Reg::RAX, RegScale::Eight, OperandSize::Qword)
                            ).ok()?;
                            writer.mov(Reg::RCX, msc_printf as u64).ok()?;
                            writer.call(Reg::RCX).ok()?;
                        }
                        Cmd::Try { loc } => {
                            if cmd.push_bit {
                                ret_val_locations.insert(loc);
                            }
                        }
                        Cmd::Return6 | Cmd::Return8 => {
                            writer.pop(Reg::RAX).ok()?;
                            writer.write_ret(var_count as u32).ok()?;
                        }
                        Cmd::Return7 | Cmd::Return9 | Cmd::End => {
                            writer.write_ret(var_count as u32).ok()?;
                        }
                        Cmd::Exit => {
                            writer.mov(Reg::EAX, 60u32).unwrap();
                            writer.write2(
                                Mnemonic::XOR,
                                Operand::Direct(Reg::EDI),
                                Operand::Direct(Reg::EDI)
                            ).unwrap();
                            writer.write0(
                                Mnemonic::SYSCALL
                            ).unwrap();
                        }
                        Cmd::Nop => {}
                        _ => {
                            println!("{:?} not recognized", cmd);
                        }
                    }
                    last_cmd_pushint = match cmd.cmd {
                        Cmd::PushInt { val } => {
                            Some(val)
                        }
                        Cmd::PushShort { val } => {
                            Some(val as u32)
                        }
                        _ => {
                            None
                        }
                    };
                }
                //writer.write_ret(var_count as u32).ok()?;
                for relocation in jump_relocations {
                    writer.seek(SeekFrom::Start(relocation.0)).unwrap();
                    writer.write1(
                        relocation.1,
                        Operand::Literal32(
                            (*command_locations.get(&relocation.2).unwrap() as i64
                             - relocation.0 as i64
                             - match relocation.1 {
                                Mnemonic::JMP => 5,
                                Mnemonic::JE => 6,
                                Mnemonic::JNE => 6,
                                _ => { unreachable!() }
                             })
                            as u32
                        )
                    ).unwrap();
                }
            } else {
                writer.write0(Mnemonic::RET).ok()?;
            }
            let buffer = writer.get_inner_writer_ref().get_ref();
            let mut code = JitMemory::new((buffer.len() + (PAGE_SIZE - 1)) / PAGE_SIZE);
            unsafe {
                &code.as_slice()[..buffer.len()].copy_from_slice(&buffer[..]);
            }
            println!("\n\nEmitted asm:");
            objdump(buffer);
            mem.push(code);
        }

        for (script_index, pos, script_offset) in call_relocs {
            let call_addr = mem[self.get_script_from_loc(script_offset).unwrap()].contents as u64;
            unsafe {
                *(mem[script_index].contents.offset(pos as isize + 2) as *mut u64) = call_addr;
            }
        }

        let entrypoint_index = self.get_script_from_loc(self.entrypoint)?;

        //println!("{}", buffer.iter().map(|b| format!("{:02X}", b)).collect::<Vec<String>>().join(" "));
        println!("\n\n\n");
        Some(CompiledProgram {
            mem, entrypoint_index,
            string_section, string_offsets, global_vars
        })
    }
}

fn objdump(buffer: &Vec<u8>) {
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

