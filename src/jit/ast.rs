use msc::{Command, Script, Cmd};

pub trait AsAst {
    fn as_ast(&self) -> ScriptAst;
}

fn cmd_to_binop(c: Cmd) -> BinOp {
    match c {
        Cmd::AddI => BinOp::Add(Type::Int),
        Cmd::SubI => BinOp::Sub(Type::Int),
        Cmd::MultI => BinOp::Mult(Type::Int),
        Cmd::DivI => BinOp::Div(Type::Int),
        Cmd::ModI => BinOp::Mod,
        Cmd::AndI => BinOp::BitAnd,
        Cmd::OrI => BinOp::BitOr,
        Cmd::XorI => BinOp::BitXor,
        Cmd::ShiftL => BinOp::ShiftL,
        Cmd::ShiftR => BinOp::ShiftR,
        Cmd::AddF => BinOp::Add(Type::Float),
        Cmd::SubF => BinOp::Sub(Type::Float),
        Cmd::MultF => BinOp::Mult(Type::Float),
        Cmd::DivF => BinOp::Div(Type::Float),
        Cmd::Equals => BinOp::Equal(Type::Int),
        Cmd::NotEquals => BinOp::NotEqual(Type::Int),
        Cmd::LessThan => BinOp::LessThan(Type::Int),
        Cmd::LessOrEqual => BinOp::LessThanOrEqual(Type::Int),
        Cmd::Greater => BinOp::GreaterThan(Type::Int),
        Cmd::GreaterOrEqual => BinOp::GreaterThanOrEqual(Type::Int),
        Cmd::EqualsF => BinOp::Equal(Type::Float),
        Cmd::NotEqualsF => BinOp::NotEqual(Type::Float),
        Cmd::LessThanF => BinOp::LessThan(Type::Float),
        Cmd::LessOrEqualF => BinOp::LessThanOrEqual(Type::Float),
        Cmd::GreaterF => BinOp::GreaterThan(Type::Float),
        Cmd::GreaterOrEqualF => BinOp::GreaterThanOrEqual(Type::Float),
        _ => panic!("{:?} not valid BinOp", c)
    }
}

fn binop_cmd_type(c: Cmd) -> Type {
    match c {
        Cmd::AddI | Cmd::SubI | Cmd::MultI | Cmd::DivI | Cmd::ModI |
        Cmd::AndI | Cmd::OrI | Cmd::XorI | Cmd::ShiftL | Cmd::ShiftR |
        Cmd::Equals | Cmd::NotEquals | Cmd::LessThan | Cmd::LessOrEqual |
        Cmd::Greater | Cmd::GreaterOrEqual
            => Type::Int,
        Cmd::AddF | Cmd::SubF | Cmd::MultF | Cmd::DivF | Cmd::EqualsF |
        Cmd::NotEqualsF | Cmd::LessThanF | Cmd::LessOrEqualF |
        Cmd::GreaterF | Cmd::GreaterOrEqualF
            => Type::Float,
        _ => panic!("Cmd {:?} has no type", c)
    }
}

fn var_cmd_type(c: Cmd) -> Type {
    match c {
        Cmd::IncI { .. } | Cmd::DecI { .. } |
        Cmd::SetVar { .. } | Cmd::AddVarBy { .. } |
        Cmd::SubVarBy { .. } | Cmd::MultVarBy { .. } |
        Cmd::DivVarBy { .. } | Cmd::ModVarBy { .. } |
        Cmd::AndVarBy { .. } | Cmd::OrVarBy { .. } |
        Cmd::XorVarBy { .. }
            => Type::Int,
        Cmd::IncF { .. } | Cmd::DecF { .. } |
        Cmd::VarSetF { .. } | Cmd::AddVarByF { .. } |
        Cmd::SubVarByF { .. } | Cmd::MultVarByF { .. } |
        Cmd::DivVarByF { .. }
            => Type::Float,
        _ => panic!("Cmd {:?} has no type", c)
    }
}

fn cmd_to_assignop(c: Cmd) -> AssignOp {
    match c {
        Cmd::IncI { .. } => AssignOp::Add(Type::Int),
        Cmd::DecI { .. } => AssignOp::Sub(Type::Int),
        Cmd::SetVar { .. } => AssignOp::Set(Type::Int),
        Cmd::AddVarBy { .. } => AssignOp::Add(Type::Int),
        Cmd::SubVarBy { .. } => AssignOp::Sub(Type::Int),
        Cmd::MultVarBy { .. } => AssignOp::Mult(Type::Int),
        Cmd::DivVarBy { .. } => AssignOp::Div(Type::Int),
        Cmd::ModVarBy { .. } => AssignOp::Mod,
        Cmd::AndVarBy { .. } => AssignOp::And,
        Cmd::OrVarBy { .. } => AssignOp::Or,
        Cmd::XorVarBy { .. } => AssignOp::Xor,
        Cmd::IncF { .. } => AssignOp::Add(Type::Float),
        Cmd::DecF { .. } => AssignOp::Sub(Type::Int),
        Cmd::VarSetF { .. } => AssignOp::Set(Type::Float),
        Cmd::AddVarByF { .. } => AssignOp::Add(Type::Float),
        Cmd::SubVarByF { .. } => AssignOp::Sub(Type::Float),
        Cmd::MultVarByF { .. } => AssignOp::Mult(Type::Float),
        Cmd::DivVarByF { .. } => AssignOp::Div(Type::Float),
        _ => panic!("Cmd {:?} has no type", c)
    }
}

fn cmd_to_unaryop(c: Cmd) -> UnaryOp {
    match c {
        Cmd::Not => UnaryOp::Not,
        Cmd::NotI => UnaryOp::BitNot,
        Cmd::NegI => UnaryOp::Negate(Type::Int),
        Cmd::NegF => UnaryOp::Negate(Type::Float),
        _ => panic!("Cmd {:?} not a UnaryOp", c)
    }
}

fn unaryop_cmd_type(c: Cmd) -> Type {
    match c {
        Cmd::Not | Cmd::NotI | Cmd::NegI => Type::Int,
        Cmd::NegF => Type::Float,
        _ => panic!("Cmd {:?} not a UnaryOp", c)
    }
}

fn take_node<'a, I, T>(commands: &mut I, type_suspect: T) -> Option<Node>
where
    I: Iterator<Item = InterForm>,
    T: Into<Option<Type>>,
{
    let type_suspect = type_suspect.into();
    loop {
        let next_command = commands.next()?;
        match next_command {
            InterForm::Cmd{ cmd: c } => {
                match c.cmd {
                    Cmd::PushInt { val } => {
                        if c.push_bit {
                            return Some(
                                match type_suspect.unwrap_or(Type::Int) {
                                    Type::Int => Node::Const{ val: Const::U32(val) },
                                    Type::Float => Node::Const{
                                        val: Const::F32(
                                            unsafe { std::mem::transmute(val) }
                                        )
                                    },
                                })
                        }
                    }
                    Cmd::PushShort { val } => {
                        if c.push_bit {
                            return Some(Node::Const{ val: Const::U32(val as u32) });
                        }
                    }
                    Cmd::PushVar { var_type, var_num } => {
                        if c.push_bit {
                            return Some(Node::Var {
                                is_global: var_type == 1,
                                var_num
                            })
                        }
                    }
                    Cmd::AddI | Cmd::SubI | Cmd::MultI | Cmd::DivI | Cmd::ModI |
                    Cmd::AndI | Cmd::XorI | Cmd::ShiftL| Cmd::OrI  | Cmd::ShiftR |
                    Cmd::AddF | Cmd::SubF | Cmd::MultF | Cmd::DivF | Cmd::Equals |
                    Cmd::NotEquals | Cmd::LessThan | Cmd::LessOrEqual | Cmd::Greater |
                    Cmd::GreaterOrEqual | Cmd::EqualsF | Cmd::NotEqualsF | Cmd::LessThanF |
                    Cmd::LessOrEqualF | Cmd::GreaterF | Cmd::GreaterOrEqualF => {
                        if c.push_bit {
                            return Some(Node::BinOp {
                                op:    cmd_to_binop(c.cmd),
                                right: Box::new(take_node(commands, binop_cmd_type(c.cmd))?),
                                left:  Box::new(take_node(commands, binop_cmd_type(c.cmd))?),
                            })
                        }
                    }
                    Cmd::Return6 | Cmd::Return8 => {
                        return Some(Node::Return {
                            val: Some(Box::new(take_node(commands, None)?))
                        })
                    }
                    Cmd::Return7 | Cmd::Return9 | Cmd::End => {
                        return Some(Node::Return { val: None });
                    }
                    Cmd::IncI { var_type, var_num } | Cmd::DecI { var_type, var_num } |
                    Cmd::IncF { var_type, var_num } | Cmd::DecF { var_type, var_num } => {
                        return Some(Node::Assign {
                            op: cmd_to_assignop(c.cmd),
                            is_global: var_type == 1,
                            var_num,
                            right: Box::new(Node::const_from_type(1, var_cmd_type(c.cmd)))
                        })
                    }
                    Cmd::SetVar { var_type, var_num } | Cmd::AddVarBy { var_type, var_num } |
                    Cmd::SubVarBy { var_type, var_num } | Cmd::MultVarBy { var_type, var_num } |
                    Cmd::DivVarBy { var_type, var_num } | Cmd::ModVarBy { var_type, var_num } |
                    Cmd::AndVarBy { var_type, var_num } | Cmd::OrVarBy { var_type, var_num } |
                    Cmd::XorVarBy { var_type, var_num } |
                    Cmd::VarSetF { var_type, var_num } | Cmd::AddVarByF { var_type, var_num } |
                    Cmd::SubVarByF { var_type, var_num } | Cmd::MultVarByF { var_type, var_num } |
                    Cmd::DivVarByF { var_type, var_num } => {
                        return Some(Node::Assign {
                            op: cmd_to_assignop(c.cmd),
                            is_global: var_type == 1,
                            var_num,
                            right: Box::new(take_node(commands, None)?)
                        })
                    }
                    Cmd::Not | Cmd::NotI | Cmd::NegI | Cmd::NegF => {
                        if c.push_bit {
                            return Some(Node::UnaryOp {
                                op: cmd_to_unaryop(c.cmd),
                                left:  Box::new(take_node(commands, unaryop_cmd_type(c.cmd))?),
                            });
                        }
                    }
                    Cmd::IntToFloat { stack_pos } | Cmd::FloatToInt { stack_pos } => {
                        if stack_pos != 0 {
                            panic!("Variable stack_pos of int/float casting not supported");
                        }
                        return Some(Node::UnaryOp {
                            op: match c.cmd {
                                Cmd::IntToFloat { stack_pos: _ } => UnaryOp::ToFloat,
                                Cmd::FloatToInt { stack_pos: _ } => UnaryOp::ToInt,
                                _ => unreachable!()
                            },
                            left: Box::new(take_node(commands,
                                     match c.cmd {
                                        Cmd::IntToFloat { stack_pos: _ } => Type::Int,
                                        Cmd::FloatToInt { stack_pos: _ } => Type::Float,
                                        _ => unreachable!()
                                     }
                                  )?)
                        })
                    }
                    Cmd::PrintF { arg_count } => {
                        let mut args = vec![];
                        for _ in 0..arg_count-1 {
                            args.push(take_node(commands, Type::Float)?);
                        }
                        args.reverse();
                        let str_num = Box::new(take_node(commands, Type::Int)?);
                        return Some(Node::Printf {
                            str_num,
                            args
                        });
                    }
                    Cmd::Sys { arg_count, sys_num } => {
                        let mut args = vec![];
                        for _ in 0..arg_count {
                            args.push(take_node(commands, Type::Float)?);
                        }
                        args.reverse();
                        return Some(Node::SysCall {
                            sys_num,
                            args
                        });
                    }
                    Cmd::Jump { loc: _ } | Cmd::Jump5 { loc: _ } | Cmd::Else { loc: _ } => panic!("Jump unsupported"),
                    Cmd::Nop | Cmd::Begin { var_count: _, arg_count: _ } | Cmd::Unk1 |
                    Cmd::ErrorC | Cmd::Error4C | Cmd::Exit | Cmd::Push | Cmd::Pop => {}
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn group_structures<'a, I>(commands: &mut I) -> Vec<InterForm>
where
    I: Iterator<Item = &'a Command>,
{
    let mut out: Vec<InterForm> = vec![];
    while let Some(c) = commands.next() {
        match c.cmd {
            _ => {
                out.push(InterForm::Cmd { cmd: c.clone() });
            }
        }
    }
    out
}

impl AsAst for Script {
    fn as_ast(&self) -> ScriptAst {
        let mut i = self.commands.iter();
        let first_command = i.next().unwrap();
        let (var_count, arg_count) = 
            if let Cmd::Begin { var_count, arg_count } = first_command.cmd {
                (var_count, arg_count)
            }
            else {
                panic!("Script does not begin with Begin, begins with {:?}", first_command);
            };
        let temp = group_structures(&mut i);
        let mut commands = temp.iter().cloned().rev();
        
        let mut nodes = vec![];
        while let Some(node) = take_node(&mut commands, None) {
            nodes.push(node);
        }
        nodes.reverse();
        ScriptAst {
            nodes,
            var_count,
            arg_count,
        }
    }
}

#[derive(Debug)]
pub struct ScriptAst {
    pub var_count: u16,
    pub arg_count: u16,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone)]
pub enum InterForm {
    IfElseBlock {
        if_block: Vec<InterForm>,
        else_block: Option<Vec<InterForm>>
    },
    Node {
        node: Node
    },
    Cmd {
        cmd: Command
    },
}

#[derive(Debug, Clone)]
pub enum Node {
    Assign {
        op: AssignOp,
        is_global: bool,
        var_num: u16,
        right: Box<Node>,
    },
    Const {
        val: Const
    },
    BinOp { 
        op: BinOp,
        left: Box<Node>,
        right: Box<Node>,
    },
    UnaryOp {
        op: UnaryOp,
        left: Box<Node>,
    },
    If {
        cond: Box<Node>,
        if_block: Vec<Node>,
        else_block: Vec<Node>,
    },
    Return {
        val: Option<Box<Node>>
    },
    FuncCall {
        func_offset: u32,
        args: Vec<Node>,
    },
    SysCall {
        sys_num: u8,
        args: Vec<Node>,
    },
    Printf {
        str_num: Box<Node>,
        args: Vec<Node>
    },
    Var {
        is_global: bool,
        var_num: u16,
    }
}

impl Node {
    pub fn const_from_type(val: i32, t: Type) -> Node {
        match t {
            Type::Int => Node::Const { val: Const::U32( unsafe { std::mem::transmute(val) } ) },
            Type::Float => Node::Const { val: Const::F32(val as f32) } 
        }
    }

    pub fn as_u32(&self) -> Option<u64> {
        if let Node::Const { val } = self {
            if let Const::U32(val) = val {
                Some(*val as u64)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add(Type),
    Sub(Type),
    Mult(Type),
    Div(Type),
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    And,
    Or,
    ShiftR,
    ShiftL,
    LessThan(Type),
    LessThanOrEqual(Type),
    Equal(Type),
    NotEqual(Type),
    GreaterThanOrEqual(Type),
    GreaterThan(Type),
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Not,
    BitNot,
    ToFloat,
    ToInt,
    Negate(Type)
}

#[derive(Debug, Clone)]
pub enum AssignOp {
    Set(Type),
    Add(Type),
    Sub(Type),
    Mult(Type),
    Div(Type),
    Mod,
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone)]
pub enum Fix {
    Pre,
    Post
}

#[derive(Debug, Clone)]
pub enum Type {
    Int,
    Float
}

#[derive(Debug, Clone)]
pub enum Const {
    U32(u32),
    F32(f32),
    Str(String),
}
