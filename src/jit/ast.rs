use msc::{Command, Script, Cmd};
use std::slice::Iter;

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
        _ => panic!("{:?} not valid BinOp", c)
    }
}

fn take_node<'a, I, T>(commands: &mut I, type_suspect: T) -> Option<Node<'a>>
where
    I: Iterator<Item = InterForm<'a>>,
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
                    Cmd::AddI | Cmd::SubI | Cmd::MultI | Cmd::DivI | Cmd::ModI => {
                        if c.push_bit {
                            return Some(Node::BinOp {
                                op:    cmd_to_binop(c.cmd),
                                right: Box::new(take_node(commands, Type::Int)?),
                                left:  Box::new(take_node(commands, Type::Int)?),
                            })
                        } 
                    }
                    Cmd::Return6 | Cmd::Return8 => {
                        return Some(Node::Return {
                            val: Some(Box::new(take_node(commands, None)?))
                        })
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    None
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
        nodes
    }
}

#[derive(Debug)]
pub enum InterForm<'a> {
    IfElseBlock {
        if_block: Vec<InterForm<'a>>,
        else_block: Option<Vec<InterForm<'a>>>
    },
    Node {
        node: Node<'a>
    },
    Cmd {
        cmd: Command
    },
}

#[derive(Debug)]
pub enum Node<'a> {
    Assign {
        op: AssignOp,
        is_global: bool,
        var_num: u32,
    },
    Const {
        val: Const
    },
    BinOp { 
        op: BinOp,
        left: Box<Node<'a>>,
        right: Box<Node<'a>>,
    },
    UnaryOp {
        op: UnaryOp,
        left: &'a Node<'a>,
    },
    If {
        cond: &'a Node <'a>,
        if_block: Vec<Node<'a>>,
        else_block: Vec<Node<'a>>,
    },
    Return {
        val: Option<Box<Node<'a>>>
    },
    FuncCall {
        func_offset: u32,
        args: Vec<Node<'a>>,
    },
    SysCall {
        sys_num: u8,
        args: Vec<Node<'a>>,
    },
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
    Inc(Fix, Type),
    Dec(Fix, Type),
    Not,
    BitNot,
    ToFloat,
    ToInt,
    Negate(Type)
}

#[derive(Debug)]
pub enum AssignOp {
    Set,
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
