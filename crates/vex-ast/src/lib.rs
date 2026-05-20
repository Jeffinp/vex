//! AST nodes for Vex.
//!
//! Mantido em crate próprio (separado do parser) para que typeck/codegen
//! consigam consumir AST sem puxar dependência do parser.

use smol_str::SmolStr;

pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self { Self { node, span } }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Fn(FnDecl),
    Struct(StructDecl),
    Impl(ImplBlock),
    Use(UsePath),
    Const(ConstDecl),
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: SmolStr,
    pub params: Vec<Param>,
    pub ret_type: Type,
    pub body: Block,
    pub is_pub: bool,
    pub is_comptime: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: SmolStr,
    pub fields: Vec<(SmolStr, Type, Span)>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub target: SmolStr,
    pub methods: Vec<FnDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: SmolStr,
    pub ty: Type,
    pub value: Expr,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UsePath {
    pub segments: Vec<SmolStr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: SmolStr,
        mutable: bool,
        type_ann: Option<Type>,
        value: Expr,
        span: Span,
    },
    Return(Option<Expr>, Span),
    If {
        cond: Expr,
        then_body: Block,
        else_body: Option<Block>,
        span: Span,
    },
    While { cond: Expr, body: Block, span: Span },
    For { var: SmolStr, iter: Expr, body: Block, span: Span },
    Break(Span),
    Continue(Span),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64, Span),
    Float(f64, Span),
    Str(SmolStr, Span),
    Bool(bool, Span),
    Ident(SmolStr, Span),
    BinOp { op: BinOp, left: Box<Expr>, right: Box<Expr>, span: Span },
    UnaryOp { op: UnaryOp, val: Box<Expr>, span: Span },
    Call { callee: Box<Expr>, args: Vec<Expr>, span: Span },
    MethodCall { receiver: Box<Expr>, name: SmolStr, args: Vec<Expr>, span: Span },
    FieldAccess { obj: Box<Expr>, field: SmolStr, span: Span },
    Index { obj: Box<Expr>, idx: Box<Expr>, span: Span },
    Array(Vec<Expr>, Span),
    StructLit { name: SmolStr, fields: Vec<(SmolStr, Expr)>, span: Span },
    Match { val: Box<Expr>, arms: Vec<MatchArm>, span: Span },
    Block(Block),
    Ref { mutable: bool, val: Box<Expr>, span: Span },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Int(i64),
    Bool(bool),
    Str(SmolStr),
    Ident(SmolStr),
    Wildcard,
    Range { lo: i64, hi: i64, inclusive: bool },
}

#[derive(Debug, Clone)]
pub enum Type {
    Int, Float, Bool, Str, Void,
    Named(SmolStr),
    Array(Box<Type>),
    Ref { mutable: bool, inner: Box<Type> },
    Fn(Vec<Type>, Box<Type>),
    Infer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp { Add, Sub, Mul, Div, Mod, Eq, Neq, Lt, Gt, Lte, Gte, And, Or }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp { Neg, Not }
