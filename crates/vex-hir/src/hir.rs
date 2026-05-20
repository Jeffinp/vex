//! Tipos do HIR.
//!
//! Diferenças vs `vex_ast`:
//! - identificadores em referências viram `DefId`
//! - blocos preservam ordem mas escopos são planos (já lowered)
//! - todos os tipos nomeados são resolvidos a `DefId` de struct/alias

use indexmap::IndexMap;
use smol_str::SmolStr;
use vex_ast::{BinOp, UnaryOp};

pub type Span = std::ops::Range<usize>;

/// Identificador único de uma definição (fn, struct, const, variável local,
/// parâmetro). Atribuído pelo resolver durante a passagem AST→HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefId(pub u32);

/// Tipo de definição. Permite distinguir o que um `DefId` aponta sem
/// precisar consultar a tabela `defs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Fn,
    Struct,
    Const,
    Local,
    Param,
    /// `self` em métodos. Tem semântica especial (tipo é Self do impl).
    SelfParam,
}

/// Registro de uma definição na tabela global do módulo.
#[derive(Debug, Clone)]
pub struct Def {
    pub name: SmolStr,
    pub kind: DefKind,
    pub span: Span,
}

/// Módulo resolvido. Mantém tabela de defs e árvore HIR de items.
#[derive(Debug, Clone)]
pub struct HirModule {
    pub defs: Vec<Def>,
    pub items: Vec<HirItem>,
}

impl HirModule {
    pub fn def(&self, id: DefId) -> &Def {
        &self.defs[id.0 as usize]
    }
}

#[derive(Debug, Clone)]
pub enum HirItem {
    Fn(HirFn),
    Struct(HirStruct),
    Const(HirConst),
    Impl(HirImpl),
}

#[derive(Debug, Clone)]
pub struct HirFn {
    pub id: DefId,
    pub name: SmolStr,
    pub params: Vec<HirParam>,
    pub ret_type: HirType,
    pub body: HirBlock,
    pub is_pub: bool,
    pub is_comptime: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub id: DefId,
    pub name: SmolStr,
    pub ty: HirType,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirStruct {
    pub id: DefId,
    pub name: SmolStr,
    pub fields: IndexMap<SmolStr, HirField>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirField {
    pub name: SmolStr,
    pub ty: HirType,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirConst {
    pub id: DefId,
    pub name: SmolStr,
    pub ty: HirType,
    pub value: HirExpr,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirImpl {
    pub target: DefId,
    pub methods: Vec<HirFn>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub stmts: Vec<HirStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirStmt {
    Let {
        id: DefId,
        name: SmolStr,
        mutable: bool,
        type_ann: Option<HirType>,
        value: HirExpr,
        span: Span,
    },
    Return(Option<HirExpr>, Span),
    If {
        cond: HirExpr,
        then_body: HirBlock,
        else_body: Option<HirBlock>,
        span: Span,
    },
    While { cond: HirExpr, body: HirBlock, span: Span },
    For {
        var_id: DefId,
        var_name: SmolStr,
        iter: HirExpr,
        body: HirBlock,
        span: Span,
    },
    Break(Span),
    Continue(Span),
    Expr(HirExpr),
}

#[derive(Debug, Clone)]
pub enum HirExpr {
    Int(i64, Span),
    Float(f64, Span),
    Str(SmolStr, Span),
    Bool(bool, Span),
    /// Identificador resolvido. `DefId` aponta para fn/struct/const/local/param.
    Name { id: DefId, name: SmolStr, span: Span },
    /// Função built-in (print, println, etc.) ou desconhecida — typeck decide
    /// se é erro ou stdlib. Mantida como string até a stdlib existir formalmente.
    Builtin { name: SmolStr, span: Span },
    SelfRef(Span),
    BinOp { op: BinOp, left: Box<HirExpr>, right: Box<HirExpr>, span: Span },
    UnaryOp { op: UnaryOp, val: Box<HirExpr>, span: Span },
    Call { callee: Box<HirExpr>, args: Vec<HirExpr>, span: Span },
    MethodCall { receiver: Box<HirExpr>, name: SmolStr, args: Vec<HirExpr>, span: Span },
    FieldAccess { obj: Box<HirExpr>, field: SmolStr, span: Span },
    Index { obj: Box<HirExpr>, idx: Box<HirExpr>, span: Span },
    Array(Vec<HirExpr>, Span),
    StructLit { struct_id: DefId, name: SmolStr, fields: Vec<(SmolStr, HirExpr)>, span: Span },
    Match { val: Box<HirExpr>, arms: Vec<HirMatchArm>, span: Span },
    Block(HirBlock),
    Borrow { mutable: bool, val: Box<HirExpr>, span: Span },
    Assign { target: Box<HirExpr>, value: Box<HirExpr>, span: Span },
}

#[derive(Debug, Clone)]
pub struct HirMatchArm {
    pub pattern: HirPattern,
    pub body: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirPattern {
    Int(i64),
    Bool(bool),
    Str(SmolStr),
    Binding { id: DefId, name: SmolStr },
    Wildcard,
    Range { lo: i64, hi: i64, inclusive: bool },
}

#[derive(Debug, Clone)]
pub enum HirType {
    Int, Float, Bool, Str, Char, Void,
    /// Tipo nomeado resolvido para um struct.
    Struct(DefId),
    /// Tipo `Self` dentro de um impl block — resolvido por typeck.
    SelfTy,
    Array(Box<HirType>),
    Ref { mutable: bool, inner: Box<HirType> },
    /// Tipo nomeado ainda não resolvido (apenas `Self` aceito; outros viram erro).
    Unresolved(SmolStr),
}
