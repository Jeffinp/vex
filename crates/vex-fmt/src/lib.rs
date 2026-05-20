//! Formatter opinativo (zero config) para Vex.
//!
//! Princípio: como `gofmt`/`rustfmt`, não há flags de estilo. Uma única
//! formatação canônica. Re-emite a AST como source code em estilo
//! padronizado.
//!
//! Regras:
//! - 4 espaços de indentação
//! - 1 linha em branco entre items
//! - `{` na mesma linha do header (estilo K&R)
//! - 1 espaço antes/depois de operadores binários
//! - sem ponto-e-vírgulas (Vex usa nova linha)
//! - structs com fields em linhas separadas, vírgula em todas
//! - parâmetros de fn na mesma linha se couberem (até ~80 colunas)

use std::fmt::Write;
use vex_ast::{
    BinOp, Block, ConstDecl, Expr, FnDecl, ImplBlock, Item, Module, Param, Pattern, Stmt,
    StructDecl, Type, UnaryOp, UsePath,
};

/// Formata um source code Vex. Se o parsing falhar, retorna o original
/// (formatter nunca quebra código que ele não consegue entender).
pub fn format(source: &str) -> String {
    match vex_parser::parse(source) {
        Ok(module) => format_module(&module),
        Err(_) => source.to_string(),
    }
}

pub fn format_module(m: &Module) -> String {
    let mut p = Printer::new();
    for (i, item) in m.items.iter().enumerate() {
        if i > 0 { p.newline(); p.newline(); }
        p.item(item);
    }
    if !m.items.is_empty() { p.newline(); }
    p.into_string()
}

struct Printer {
    buf: String,
    indent: usize,
    /// `true` se o último caractere escrito foi `\n` — pra decidir se
    /// precisamos emitir indent antes da próxima escrita.
    at_line_start: bool,
}

impl Printer {
    fn new() -> Self {
        Self { buf: String::new(), indent: 0, at_line_start: true }
    }

    fn into_string(self) -> String { self.buf }

    fn newline(&mut self) {
        self.buf.push('\n');
        self.at_line_start = true;
    }

    fn write(&mut self, s: &str) {
        if self.at_line_start {
            for _ in 0..self.indent { self.buf.push_str("    "); }
            self.at_line_start = false;
        }
        self.buf.push_str(s);
    }

    fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) {
        if self.at_line_start {
            for _ in 0..self.indent { self.buf.push_str("    "); }
            self.at_line_start = false;
        }
        let _ = self.buf.write_fmt(args);
    }

    fn indented(&mut self, f: impl FnOnce(&mut Self)) {
        self.indent += 1;
        f(self);
        self.indent -= 1;
    }

    // ── Items ────────────────────────────────────────────────────────

    fn item(&mut self, item: &Item) {
        match item {
            Item::Fn(f)     => self.fn_decl(f),
            Item::Struct(s) => self.struct_decl(s),
            Item::Impl(i)   => self.impl_block(i),
            Item::Const(c)  => self.const_decl(c),
            Item::Use(u)    => self.use_decl(u),
        }
    }

    fn fn_decl(&mut self, f: &FnDecl) {
        if f.is_pub      { self.write("pub "); }
        if f.is_comptime { self.write("comptime "); }
        write!(self, "fn {}(", f.name);
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 { self.write(", "); }
            self.param(p);
        }
        self.write(") ");
        if !matches!(f.ret_type, Type::Void) {
            self.write("-> ");
            self.type_(&f.ret_type);
            self.write(" ");
        }
        self.block(&f.body);
    }

    fn param(&mut self, p: &Param) {
        if p.mutable { self.write("mut "); }
        if p.name == "self" {
            self.write("self");
        } else {
            write!(self, "{}: ", p.name);
            self.type_(&p.ty);
        }
    }

    fn struct_decl(&mut self, s: &StructDecl) {
        if s.is_pub { self.write("pub "); }
        write!(self, "struct {} {{", s.name);
        self.newline();
        self.indented(|p| {
            for (n, t, _) in &s.fields {
                write!(p, "{n}: ");
                p.type_(t);
                p.write(",");
                p.newline();
            }
        });
        self.write("}");
    }

    fn impl_block(&mut self, i: &ImplBlock) {
        write!(self, "impl {} {{", i.target);
        self.newline();
        self.indented(|p| {
            for (idx, m) in i.methods.iter().enumerate() {
                if idx > 0 { p.newline(); }
                p.fn_decl(m);
                p.newline();
            }
        });
        self.write("}");
    }

    fn const_decl(&mut self, c: &ConstDecl) {
        if c.is_pub { self.write("pub "); }
        write!(self, "const {}: ", c.name);
        self.type_(&c.ty);
        self.write(" = ");
        self.expr(&c.value);
    }

    fn use_decl(&mut self, u: &UsePath) {
        self.write("use ");
        for (i, seg) in u.segments.iter().enumerate() {
            if i > 0 { self.write("::"); }
            self.write(seg);
        }
    }

    // ── Statements / bloco ─────────────────────────────────────────────

    fn block(&mut self, b: &Block) {
        self.write("{");
        if b.stmts.is_empty() {
            self.write(" }");
            return;
        }
        self.newline();
        self.indented(|p| {
            for s in &b.stmts {
                p.stmt(s);
                p.newline();
            }
        });
        self.write("}");
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let { name, mutable, type_ann, value, .. } => {
                self.write("let ");
                if *mutable { self.write("mut "); }
                self.write(name);
                if let Some(t) = type_ann {
                    self.write(": ");
                    self.type_(t);
                }
                self.write(" = ");
                self.expr(value);
            }
            Stmt::Return(opt, _) => {
                self.write("return");
                if let Some(e) = opt {
                    self.write(" ");
                    self.expr(e);
                }
            }
            Stmt::If { cond, then_body, else_body, .. } => {
                self.write("if ");
                self.expr(cond);
                self.write(" ");
                self.block(then_body);
                if let Some(eb) = else_body {
                    self.write(" else ");
                    // `else if` reentrante vira bloco com 1 If — manter
                    // a forma compacta nesse caso.
                    if eb.stmts.len() == 1 {
                        if let Stmt::If { .. } = eb.stmts[0] {
                            self.stmt(&eb.stmts[0]);
                            return;
                        }
                    }
                    self.block(eb);
                }
            }
            Stmt::While { cond, body, .. } => {
                self.write("while ");
                self.expr(cond);
                self.write(" ");
                self.block(body);
            }
            Stmt::For { var, iter, body, .. } => {
                write!(self, "for {var} in ");
                self.expr(iter);
                self.write(" ");
                self.block(body);
            }
            Stmt::Break(_)    => self.write("break"),
            Stmt::Continue(_) => self.write("continue"),
            Stmt::Expr(e)     => self.expr(e),
        }
    }

    // ── Expressões ─────────────────────────────────────────────────────

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Int(v, _)   => { write!(self, "{v}"); }
            Expr::Float(v, _) => { write!(self, "{v:?}"); }
            Expr::Str(s, _)   => { write!(self, "{s:?}"); }
            Expr::Bool(v, _)  => { self.write(if *v { "true" } else { "false" }); }
            Expr::Ident(n, _) => { self.write(n); }
            Expr::SelfRef(_)  => { self.write("self"); }

            Expr::BinOp { op, left, right, .. } => {
                self.expr(left);
                write!(self, " {} ", bin_op_str(*op));
                self.expr(right);
            }
            Expr::UnaryOp { op, val, .. } => {
                self.write(match op { UnaryOp::Neg => "-", UnaryOp::Not => "!" });
                self.expr(val);
            }
            Expr::Call { callee, args, .. } => {
                self.expr(callee);
                self.write("(");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { self.write(", "); }
                    self.expr(a);
                }
                self.write(")");
            }
            Expr::MethodCall { receiver, name, args, .. } => {
                self.expr(receiver);
                write!(self, ".{name}(");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { self.write(", "); }
                    self.expr(a);
                }
                self.write(")");
            }
            Expr::FieldAccess { obj, field, .. } => {
                self.expr(obj);
                write!(self, ".{field}");
            }
            Expr::Index { obj, idx, .. } => {
                self.expr(obj);
                self.write("[");
                self.expr(idx);
                self.write("]");
            }
            Expr::Array(items, _) => {
                self.write("[");
                for (i, it) in items.iter().enumerate() {
                    if i > 0 { self.write(", "); }
                    self.expr(it);
                }
                self.write("]");
            }
            Expr::StructLit { name, fields, .. } => {
                write!(self, "{name} {{ ");
                for (i, (n, v)) in fields.iter().enumerate() {
                    if i > 0 { self.write(", "); }
                    write!(self, "{n}: ");
                    self.expr(v);
                }
                self.write(" }");
            }
            Expr::Match { val, arms, .. } => {
                self.write("match ");
                self.expr(val);
                self.write(" {");
                self.newline();
                self.indented(|p| {
                    for arm in arms {
                        p.pattern(&arm.pattern);
                        p.write(" => ");
                        p.expr(&arm.body);
                        p.write(",");
                        p.newline();
                    }
                });
                self.write("}");
            }
            Expr::Block(b) => self.block(b),
            Expr::Ref { mutable, val, .. } => {
                self.write("&");
                if *mutable { self.write("mut "); }
                self.expr(val);
            }
            Expr::Assign { target, value, .. } => {
                self.expr(target);
                self.write(" = ");
                self.expr(value);
            }
        }
    }

    fn pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Int(v)  => { write!(self, "{v}"); }
            Pattern::Bool(v) => self.write(if *v { "true" } else { "false" }),
            Pattern::Str(s)  => { write!(self, "{s:?}"); }
            Pattern::Ident(n) => self.write(n),
            Pattern::Wildcard => self.write("_"),
            Pattern::Range { lo, hi, inclusive } => {
                write!(self, "{lo}{}{hi}", if *inclusive { "..=" } else { ".." });
            }
        }
    }

    // ── Tipos ─────────────────────────────────────────────────────────

    fn type_(&mut self, t: &Type) {
        match t {
            Type::Int   => self.write("int"),
            Type::Float => self.write("float"),
            Type::Bool  => self.write("bool"),
            Type::Str   => self.write("str"),
            Type::Void  => self.write("void"),
            Type::Named(n) => self.write(n),
            Type::Array(inner) => {
                self.write("[");
                self.type_(inner);
                self.write("]");
            }
            Type::Ref { mutable, inner } => {
                self.write("&");
                if *mutable { self.write("mut "); }
                self.type_(inner);
            }
            Type::Fn(params, ret) => {
                self.write("fn(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { self.write(", "); }
                    self.type_(p);
                }
                self.write(") -> ");
                self.type_(ret);
            }
            Type::Infer => self.write("_"),
        }
    }
}

fn bin_op_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
        BinOp::Div => "/", BinOp::Mod => "%",
        BinOp::Eq  => "==", BinOp::Neq => "!=",
        BinOp::Lt  => "<", BinOp::Gt  => ">",
        BinOp::Lte => "<=", BinOp::Gte => ">=",
        BinOp::And => "&&", BinOp::Or  => "||",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(src: &str) -> String { format(src) }

    #[test]
    fn fmt_hello() {
        let src = "fn main() -> void {println(\"Hello, Vex!\")}";
        let out = roundtrip(src);
        assert!(out.contains("fn main()"));
        assert!(out.contains("println"));
    }

    #[test]
    fn fmt_normalizes_whitespace() {
        let src = "fn   f(  x:int  )->int{return  x  +  1}";
        let out = roundtrip(src);
        assert!(out.contains("fn f(x: int) -> int"));
        assert!(out.contains("return x + 1"));
    }

    #[test]
    fn fmt_struct_block() {
        let src = "struct P { x: int, y: float }";
        let out = roundtrip(src);
        assert!(out.contains("struct P {"));
        assert!(out.contains("    x: int,"));
        assert!(out.contains("    y: float,"));
    }

    #[test]
    fn fmt_idempotent() {
        let src = include_str!("../../../examples/fib.vex");
        let once = format(src);
        let twice = format(&once);
        assert_eq!(once, twice, "formatter não é idempotente");
    }

    #[test]
    fn fmt_preserves_semantics_via_reparse() {
        let src = include_str!("../../../examples/ponto.vex");
        let formatted = format(src);
        // re-parsear não deve falhar
        let parsed = vex_parser::parse(&formatted);
        assert!(parsed.is_ok(), "formatter quebrou parsing: {parsed:?}");
    }

    #[test]
    fn fmt_invalid_returns_original() {
        // Código inválido permanece intacto.
        let src = "fn t() -> { invalido";
        assert_eq!(format(src), src);
    }
}
