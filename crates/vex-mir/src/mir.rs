//! Tipos do MIR. Inspirados em `rustc_middle::mir`, simplificados.

use smol_str::SmolStr;
use vex_ast::{BinOp, UnaryOp};
use vex_hir::DefId;
use vex_typeck::Ty;

pub type Span = std::ops::Range<usize>;

/// Índice de uma variável local (parâmetro, let, temporário).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

/// Índice de um basic block dentro da fn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

/// Módulo MIR — lista de fns lowered + tabela de structs (para layout).
#[derive(Debug, Clone)]
pub struct MirModule {
    pub fns: Vec<MirFn>,
    pub structs: Vec<MirStruct>,
    /// Resolução de `Callee::Method { struct_id, name }` para o `DefId`
    /// da função. Construído pelo lowerer; permite codegen direto sem
    /// virtual dispatch.
    pub methods: Vec<MirMethod>,
}

#[derive(Debug, Clone)]
pub struct MirMethod {
    pub struct_id: DefId,
    pub name: SmolStr,
    pub fn_id: DefId,
}

#[derive(Debug, Clone)]
pub struct MirStruct {
    pub id: DefId,
    pub name: SmolStr,
    pub fields: Vec<(SmolStr, Ty)>,
}

#[derive(Debug, Clone)]
pub struct MirFn {
    pub id: DefId,
    pub name: SmolStr,
    pub params: Vec<LocalId>,
    pub locals: Vec<MirLocal>,
    pub blocks: Vec<BasicBlock>,
    pub entry: BlockId,
    pub ret_ty: Ty,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MirLocal {
    pub id: LocalId,
    pub ty: Ty,
    /// Nome no fonte (quando vem de let/param); para temporários internos,
    /// `_t<n>`. Útil para pretty-print e debug.
    pub name: SmolStr,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub stmts: Vec<Statement>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone)]
pub enum Statement {
    /// `local = rvalue`
    Assign { local: LocalId, rvalue: Rvalue, span: Span },
    /// `*place = value` para atribuição via field/index/borrow.
    Store { place: Place, value: Operand, span: Span },
    /// No-op explícito (placeholder para hooks futuros de ownership).
    Nop,
}

/// Um "place" é um lvalue: para onde escrever / de onde ler com endereço.
#[derive(Debug, Clone)]
pub struct Place {
    pub local: LocalId,
    pub projections: Vec<Projection>,
}

#[derive(Debug, Clone)]
pub enum Projection {
    Field(SmolStr),
    Index(LocalId),
    Deref,
}

/// Operando — sempre "atômico" (sem cálculo embutido).
#[derive(Debug, Clone)]
pub enum Operand {
    Local(LocalId),
    Const(Const),
}

#[derive(Debug, Clone)]
pub enum Const {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(SmolStr),
    Unit,
}

/// Lado direito de uma assignment. Cobre todas as expressões "no segundo
/// nível" do programa.
#[derive(Debug, Clone)]
pub enum Rvalue {
    Use(Operand),
    BinaryOp { op: BinOp, lhs: Operand, rhs: Operand },
    UnaryOp { op: UnaryOp, val: Operand },
    Call { callee: Callee, args: Vec<Operand> },
    /// Acesso a campo: `local.field`.
    Field { obj: LocalId, field: SmolStr },
    /// Indexação: `local[idx]`.
    Index { obj: LocalId, idx: LocalId },
    /// `&local` ou `&mut local`.
    Ref { mutable: bool, place: Place },
    /// Construção de struct: já com campos lowered para operandos.
    StructInit { struct_id: DefId, fields: Vec<(SmolStr, Operand)> },
    /// `[a, b, c]`.
    ArrayInit { items: Vec<Operand> },
}

/// Quem chamar.
#[derive(Debug, Clone)]
pub enum Callee {
    Fn(DefId),
    Method { struct_id: DefId, name: SmolStr },
    Builtin(SmolStr),
}

/// Como o basic block termina.
#[derive(Debug, Clone)]
pub enum Terminator {
    Goto(BlockId),
    If { cond: LocalId, then: BlockId, otherwise: BlockId },
    Return(Option<LocalId>),
    /// Inalcançável (após panic, divisão por zero, etc. — pós-MVP).
    Unreachable,
}
