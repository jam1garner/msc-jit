use msc::{Command, Script, Cmd};

pub trait AsAst {
    fn as_ast(&self) -> Vec<Node>;
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
        Cmd::IncI { var_type: _, var_num: _ } | Cmd::DecI { var_type: _, var_num: _ } |
        Cmd::SetVar { var_type: _, var_num: _ } | Cmd::AddVarBy { var_type: _, var_num: _ } |
        Cmd::SubVarBy { var_type: _, var_num: _ } | Cmd::MultVarBy { var_type: _, var_num: _ } |
        Cmd::DivVarBy { var_type: _, var_num: _ } | Cmd::ModVarBy { var_type: _, var_num: _ } |
        Cmd::AndVarBy { var_type: _, var_num: _ } | Cmd::OrVarBy { var_type: _, var_num: _ } |
        Cmd::XorVarBy { var_type: _, var_num: _ }
            => Type::Int,
        Cmd::IncF { var_type: _, var_num: _ } | Cmd::DecF { var_type: _, var_num: _ } |
        Cmd::VarSetF { var_type: _, var_num: _ } | Cmd::AddVarByF { var_type: _, var_num: _ } |
        Cmd::SubVarByF { var_type: _, var_num: _ } | Cmd::MultVarByF { var_type: _, var_num: _ } |
        Cmd::DivVarByF { var_type: _, var_num: _ }
            => Type::Float,
        _ => panic!("Cmd {:?} has no type", c)
    }
}

fn cmd_to_assignop(c: Cmd) -> AssignOp {
    match c {
        Cmd::IncI { var_type: _, var_num: _ } => AssignOp::Add(Type::Int),
        Cmd::DecI { var_type: _, var_num: _ } => AssignOp::Sub(Type::Int),
        Cmd::SetVar { var_type: _, var_num: _ } => AssignOp::Set(Type::Int),
        Cmd::AddVarBy { var_type: _, var_num: _ } => AssignOp::Add(Type::Int),
        Cmd::SubVarBy { var_type: _, var_num: _ } => AssignOp::Sub(Type::Int),
        Cmd::MultVarBy { var_type: _, var_num: _ } => AssignOp::Mult(Type::Int),
        Cmd::DivVarBy { var_type: _, var_num: _ } => AssignOp::Div(Type::Int),
        Cmd::ModVarBy { var_type: _, var_num: _ } => AssignOp::Mod,
        Cmd::AndVarBy { var_type: _, var_num: _ } => AssignOp::And,
        Cmd::OrVarBy { var_type: _, var_num: _ } => AssignOp::Or,
        Cmd::XorVarBy { var_type: _, var_num: _ } => AssignOp::Xor,
        Cmd::IncF { var_type: _, var_num: _ } => AssignOp::Add(Type::Float),
        Cmd::DecF { var_type: _, var_num: _ } => AssignOp::Sub(Type::Int),
        Cmd::VarSetF { var_type: _, var_num: _ } => AssignOp::Set(Type::Float),
        Cmd::AddVarByF { var_type: _, var_num: _ } => AssignOp::Add(Type::Float),
        Cmd::SubVarByF { var_type: _, var_num: _ } => AssignOp::Sub(Type::Float),
        Cmd::MultVarByF { var_type: _, var_num: _ } => AssignOp::Mult(Type::Float),
        Cmd::DivVarByF { var_type: _, var_num: _ } => AssignOp::Div(Type::Float),
        _ => panic!("Cmd {:?} has no type", c)
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
        println!("{:?}", next_command);
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
                    Cmd::AddF | Cmd::SubF | Cmd::MultF | Cmd::DivF => {
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
                    Cmd::Nop | Cmd::Begin { var_count: _, arg_count: _ } | Cmd::Unk1 |
                    Cmd::ErrorC | Cmd::Error4C => {}
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl AsAst for Script {
    fn as_ast(&self) -> Vec<Node> {
        let mut commands = self.commands
            .iter()
            .map(|c| InterForm::Cmd { cmd: c.clone() })
            .rev();
        
        let mut nodes = vec![];
        loop {
            let a = take_node(&mut commands, None);
            println!("{:?}", a);
            if let Some(node) = a {
                nodes.push(node);
            } else {
                break
            }
        }
        nodes.reverse();
        nodes
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
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
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum UnaryOp {
    Not,
    BitNot,
    ToFloat,
    ToInt,
    Negate(Type)
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum Fix {
    Pre,
    Post
}

#[derive(Debug)]
pub enum Type {
    Int,
    Float
}

#[derive(Debug)]
pub enum Const {
    U32(u32),
    F32(f32),
    Str(String),
}
