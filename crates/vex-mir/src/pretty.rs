//! Pretty-print do MIR. Útil para debug (`vex check --emit=mir`) e snapshot tests.

use std::fmt::Write;

use crate::mir::*;

pub fn pretty_print_module(m: &MirModule) -> String {
    let mut out = String::new();
    for s in &m.structs {
        writeln!(out, "struct #{} {} {{", s.id.0, s.name).unwrap();
        for (n, t) in &s.fields {
            writeln!(out, "    {n}: {t},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }
    for f in &m.fns {
        pretty_fn(f, &mut out);
        out.push('\n');
    }
    out
}

fn pretty_fn(f: &MirFn, out: &mut String) {
    write!(out, "fn #{} {}(", f.id.0, f.name).unwrap();
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        write!(out, "_{}", p.0).unwrap();
    }
    writeln!(out, ") -> {} {{", f.ret_ty).unwrap();

    for l in &f.locals {
        let m = if l.mutable { "mut " } else { "" };
        writeln!(out, "    let {m}_{} : {} ;  // {}", l.id.0, l.ty, l.name).unwrap();
    }
    out.push('\n');

    for b in &f.blocks {
        writeln!(out, "  bb{}:", b.id.0).unwrap();
        for s in &b.stmts {
            write!(out, "    ").unwrap();
            pretty_stmt(s, out);
            out.push('\n');
        }
        write!(out, "    -> ").unwrap();
        pretty_term(&b.terminator, out);
        out.push('\n');
    }
    out.push_str("}\n");
}

fn pretty_stmt(s: &Statement, out: &mut String) {
    match s {
        Statement::Assign { local, rvalue, .. } => {
            write!(out, "_{} = ", local.0).unwrap();
            pretty_rvalue(rvalue, out);
        }
        Statement::Store { place, value, .. } => {
            pretty_place(place, out);
            write!(out, " <- ").unwrap();
            pretty_operand(value, out);
        }
        Statement::Nop => out.push_str("nop"),
    }
}

fn pretty_rvalue(r: &Rvalue, out: &mut String) {
    match r {
        Rvalue::Use(o) => pretty_operand(o, out),
        Rvalue::BinaryOp { op, lhs, rhs } => {
            pretty_operand(lhs, out);
            write!(out, " {} ", bin_op_str(*op)).unwrap();
            pretty_operand(rhs, out);
        }
        Rvalue::UnaryOp { op, val } => {
            write!(out, "{}", unary_op_str(*op)).unwrap();
            pretty_operand(val, out);
        }
        Rvalue::Call { callee, args } => {
            pretty_callee(callee, out);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                pretty_operand(a, out);
            }
            out.push(')');
        }
        Rvalue::Field { obj, field } => {
            write!(out, "_{}.{}", obj.0, field).unwrap();
        }
        Rvalue::Index { obj, idx } => {
            write!(out, "_{}[_{}]", obj.0, idx.0).unwrap();
        }
        Rvalue::Ref { mutable, place } => {
            out.push('&');
            if *mutable { out.push_str("mut "); }
            pretty_place(place, out);
        }
        Rvalue::StructInit { struct_id, fields } => {
            write!(out, "struct#{} {{ ", struct_id.0).unwrap();
            for (i, (n, v)) in fields.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write!(out, "{n}: ").unwrap();
                pretty_operand(v, out);
            }
            out.push_str(" }");
        }
        Rvalue::ArrayInit { items } => {
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                pretty_operand(v, out);
            }
            out.push(']');
        }
    }
}

fn pretty_operand(o: &Operand, out: &mut String) {
    match o {
        Operand::Local(l) => write!(out, "_{}", l.0).unwrap(),
        Operand::Const(c) => match c {
            Const::Int(v)   => write!(out, "{v}i").unwrap(),
            Const::Float(v) => write!(out, "{v}f").unwrap(),
            Const::Bool(v)  => write!(out, "{v}").unwrap(),
            Const::Str(v)   => write!(out, "{v:?}").unwrap(),
            Const::Unit     => out.push_str("()"),
        },
    }
}

fn pretty_place(p: &Place, out: &mut String) {
    write!(out, "_{}", p.local.0).unwrap();
    for pr in &p.projections {
        match pr {
            Projection::Field(n) => write!(out, ".{n}").unwrap(),
            Projection::Index(i) => write!(out, "[_{}]", i.0).unwrap(),
            Projection::Deref    => out.push_str(".*"),
        }
    }
}

fn pretty_callee(c: &Callee, out: &mut String) {
    match c {
        Callee::Fn(id) => write!(out, "fn#{}", id.0).unwrap(),
        Callee::Method { struct_id, name } => write!(out, "method#{}.{}", struct_id.0, name).unwrap(),
        Callee::Builtin(name) => write!(out, "@{name}").unwrap(),
    }
}

fn pretty_term(t: &Terminator, out: &mut String) {
    match t {
        Terminator::Goto(b) => write!(out, "goto bb{}", b.0).unwrap(),
        Terminator::If { cond, then, otherwise } => {
            write!(out, "if _{} then bb{} else bb{}", cond.0, then.0, otherwise.0).unwrap();
        }
        Terminator::Return(opt) => {
            match opt {
                Some(l) => write!(out, "return _{}", l.0).unwrap(),
                None    => out.push_str("return"),
            }
        }
        Terminator::Unreachable => out.push_str("unreachable"),
    }
}

fn bin_op_str(op: vex_ast::BinOp) -> &'static str {
    use vex_ast::BinOp::*;
    match op {
        Add => "+", Sub => "-", Mul => "*", Div => "/", Mod => "%",
        Eq => "==", Neq => "!=", Lt => "<", Gt => ">", Lte => "<=", Gte => ">=",
        And => "&&", Or => "||",
    }
}

fn unary_op_str(op: vex_ast::UnaryOp) -> &'static str {
    match op { vex_ast::UnaryOp::Neg => "-", vex_ast::UnaryOp::Not => "!" }
}
