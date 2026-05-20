//! Lowering MIR → LLVM IR.

use std::path::Path;

use indexmap::IndexMap;
use inkwell::basic_block::BasicBlock as LlvmBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::types::{BasicType, BasicTypeEnum, FunctionType, StructType};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate, OptimizationLevel};
use smol_str::SmolStr;
use vex_ast::{BinOp, UnaryOp};
use vex_hir::DefId;
use vex_mir::{
    BasicBlock, Callee, Const, LocalId, MirFn, MirModule, MirStruct, Operand, Place,
    Projection, Rvalue, Statement, Terminator,
};
use vex_typeck::Ty;

#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    #[error("erro LLVM: {0}")]
    Llvm(String),
    #[error("target inválido: {0}")]
    InvalidTarget(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct CodegenOptions {
    /// Triple do target (ex.: `x86_64-pc-windows-gnu`). `None` = host.
    pub target_triple: Option<String>,
    /// 0..=3
    pub opt_level: u8,
    /// Quando true, emite também um `.ll` ao lado do `.o`.
    pub emit_ir: bool,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self { target_triple: None, opt_level: 2, emit_ir: false }
    }
}

/// Compila `module` para um arquivo objeto `.o` em `out_obj`.
/// Retorna o triple efetivamente usado (útil pro linker).
pub fn compile_module(
    module: &MirModule,
    out_obj: &Path,
    opts: &CodegenOptions,
) -> Result<String, CodegenError> {
    Target::initialize_all(&InitializationConfig::default());

    let ctx = Context::create();
    let llvm_module = ctx.create_module("vex");
    let builder = ctx.create_builder();

    let triple_str = match &opts.target_triple {
        Some(t) => t.clone(),
        None => TargetMachine::get_default_triple()
            .as_str()
            .to_string_lossy()
            .into_owned(),
    };
    let triple = TargetTriple::create(&triple_str);
    llvm_module.set_triple(&triple);

    let target = Target::from_triple(&triple)
        .map_err(|e| CodegenError::InvalidTarget(format!("{e:?}")))?;
    let opt = match opts.opt_level {
        0 => OptimizationLevel::None,
        1 => OptimizationLevel::Less,
        2 => OptimizationLevel::Default,
        _ => OptimizationLevel::Aggressive,
    };
    let machine = target
        .create_target_machine(&triple, "generic", "", opt, RelocMode::PIC, CodeModel::Default)
        .ok_or_else(|| CodegenError::InvalidTarget("não foi possível criar target machine".into()))?;
    llvm_module.set_data_layout(&machine.get_target_data().get_data_layout());

    let mut cg = Codegen {
        ctx: &ctx,
        builder: &builder,
        module: &llvm_module,
        structs: IndexMap::new(),
        struct_fields: IndexMap::new(),
        fns: IndexMap::new(),
        method_lookup: IndexMap::new(),
        runtime: Runtime::declare(&ctx, &llvm_module),
    };

    cg.declare_structs(&module.structs);
    cg.declare_fns(&module.fns);
    for m in &module.methods {
        cg.method_lookup.insert((m.struct_id, m.name.clone()), m.fn_id);
    }
    for f in &module.fns {
        cg.compile_fn(f)?;
    }

    // Sanidade — falha se IR ficou inválido (catches bugs do codegen cedo).
    if let Err(e) = llvm_module.verify() {
        return Err(CodegenError::Llvm(e.to_string()));
    }

    if opts.emit_ir {
        let ll_path = out_obj.with_extension("ll");
        llvm_module
            .print_to_file(&ll_path)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
    }

    machine
        .write_to_file(&llvm_module, FileType::Object, out_obj)
        .map_err(|e| CodegenError::Llvm(e.to_string()))?;

    Ok(triple_str)
}

// ── Codegen state ───────────────────────────────────────────────────────

struct Codegen<'ctx, 'a> {
    ctx: &'ctx Context,
    builder: &'a Builder<'ctx>,
    module: &'a Module<'ctx>,
    structs: IndexMap<DefId, StructType<'ctx>>,
    /// Lista de campos (nome, tipo) por struct, na ordem de declaração.
    /// Permite resolver índice + tipo de field access sem ter ponteiro
    /// de volta para MirModule.
    struct_fields: IndexMap<DefId, Vec<(SmolStr, Ty)>>,
    fns: IndexMap<DefId, FunctionValue<'ctx>>,
    method_lookup: IndexMap<(DefId, SmolStr), DefId>,
    runtime: Runtime<'ctx>,
}

/// Externs do runtime declarados uma vez no módulo.
struct Runtime<'ctx> {
    print_int: FunctionValue<'ctx>,
    println_int: FunctionValue<'ctx>,
    print_float: FunctionValue<'ctx>,
    println_float: FunctionValue<'ctx>,
    print_bool: FunctionValue<'ctx>,
    println_bool: FunctionValue<'ctx>,
    print_str: FunctionValue<'ctx>,
    println_str: FunctionValue<'ctx>,
    sqrt: FunctionValue<'ctx>,
    abs_f: FunctionValue<'ctx>,
    abs_i: FunctionValue<'ctx>,
    array_alloc: FunctionValue<'ctx>,
    array_drop: FunctionValue<'ctx>,
}

impl<'ctx> Runtime<'ctx> {
    fn declare(ctx: &'ctx Context, module: &Module<'ctx>) -> Self {
        let i64_t = ctx.i64_type();
        let f64_t = ctx.f64_type();
        let bool_t = ctx.bool_type();
        let i8ptr = ctx.ptr_type(AddressSpace::default());
        let void = ctx.void_type();

        let decl = |name: &str, ty: FunctionType<'ctx>| {
            module.add_function(name, ty, Some(inkwell::module::Linkage::External))
        };

        Self {
            print_int:    decl("vex_print_int",    void.fn_type(&[i64_t.into()], false)),
            println_int:  decl("vex_println_int",  void.fn_type(&[i64_t.into()], false)),
            print_float:  decl("vex_print_float",  void.fn_type(&[f64_t.into()], false)),
            println_float:decl("vex_println_float",void.fn_type(&[f64_t.into()], false)),
            print_bool:   decl("vex_print_bool",   void.fn_type(&[bool_t.into()], false)),
            println_bool: decl("vex_println_bool", void.fn_type(&[bool_t.into()], false)),
            print_str:    decl("vex_print_str",    void.fn_type(&[i8ptr.into()], false)),
            println_str:  decl("vex_println_str",  void.fn_type(&[i8ptr.into()], false)),
            sqrt:         decl("vex_sqrt",         f64_t.fn_type(&[f64_t.into()], false)),
            abs_f:        decl("vex_abs_f64",      f64_t.fn_type(&[f64_t.into()], false)),
            abs_i:        decl("vex_abs_i64",      i64_t.fn_type(&[i64_t.into()], false)),
            array_alloc:  decl("vex_array_alloc",  i8ptr.fn_type(&[i64_t.into()], false)),
            array_drop:   decl("vex_array_drop",   void.fn_type(&[i8ptr.into(), i64_t.into()], false)),
        }
    }
}

impl<'ctx, 'a> Codegen<'ctx, 'a> {
    // ── Mapeamento de tipos ─────────────────────────────────────────────

    fn llvm_type(&self, t: &Ty) -> BasicTypeEnum<'ctx> {
        match t {
            Ty::Int   => self.ctx.i64_type().into(),
            Ty::Float => self.ctx.f64_type().into(),
            Ty::Bool  => self.ctx.bool_type().into(),
            Ty::Char  => self.ctx.i32_type().into(),
            Ty::Str   => self.ctx.ptr_type(AddressSpace::default()).into(),
            Ty::Void  => self.ctx.i8_type().into(), // placeholder; nunca usado direto
            Ty::Struct(id) => match self.structs.get(id) {
                Some(s) => (*s).into(),
                None => self.ctx.i8_type().into(),
            },
            // Arrays: fat pointer `{ ptr, i64 }` (16 bytes). Element type
            // não entra no LLVM type — codegen consulta `Ty::Array(inner)`
            // diretamente nas operações de Index/ArrayInit/Drop.
            Ty::Array(_) => self.array_struct_type().into(),
            Ty::Ref { .. } | Ty::Any | Ty::Error => {
                self.ctx.ptr_type(AddressSpace::default()).into()
            }
        }
    }

    /// Tipo LLVM do fat pointer de array: `{ ptr: i8*, len: i64 }`.
    fn array_struct_type(&self) -> StructType<'ctx> {
        self.ctx.struct_type(
            &[
                self.ctx.ptr_type(AddressSpace::default()).into(),
                self.ctx.i64_type().into(),
            ],
            false,
        )
    }

    /// Tamanho em bytes do elemento de um `Ty::Array(inner)`.
    /// Hoje suportamos apenas elementos com tamanho de máquina conhecido
    /// estaticamente. Suficiente para o MVP.
    fn elem_size_bytes(&self, elem: &Ty) -> u64 {
        match elem {
            Ty::Int | Ty::Float => 8,
            Ty::Bool => 1,
            Ty::Char => 4,
            Ty::Str | Ty::Ref { .. } | Ty::Struct(_) | Ty::Array(_)
            | Ty::Any | Ty::Error | Ty::Void => 8, // pointer-sized fallback
        }
    }

    fn fn_type(&self, params: &[Ty], ret: &Ty) -> FunctionType<'ctx> {
        let llparams: Vec<inkwell::types::BasicMetadataTypeEnum> =
            params.iter().map(|p| self.llvm_type(p).into()).collect();
        match ret {
            Ty::Void => self.ctx.void_type().fn_type(&llparams, false),
            other    => self.llvm_type(other).fn_type(&llparams, false),
        }
    }

    // ── Declarações ─────────────────────────────────────────────────────

    fn declare_structs(&mut self, structs: &[MirStruct]) {
        for s in structs {
            let st = self.ctx.opaque_struct_type(&format!("vex_{}", s.name));
            self.structs.insert(s.id, st);
            self.struct_fields.insert(s.id, s.fields.clone());
        }
        for s in structs {
            let body: Vec<BasicTypeEnum> = s.fields.iter().map(|(_, t)| self.llvm_type(t)).collect();
            self.structs.get(&s.id).unwrap().set_body(&body, false);
        }
    }

    fn declare_fns(&mut self, fns: &[MirFn]) {
        for f in fns {
            // Reaproveita params do MIR — tipos vêm de MirLocal.
            let param_tys: Vec<Ty> = f.params.iter()
                .map(|lid| f.locals[lid.0 as usize].ty.clone())
                .collect();
            let llfn_ty = self.fn_type(&param_tys, &f.ret_ty);
            let name = if f.name == "main" {
                "main".to_string()
            } else {
                format!("vex_fn_{}", f.id.0)
            };
            let func = self.module.add_function(&name, llfn_ty, None);
            self.fns.insert(f.id, func);
        }
    }

    // ── Compilação de função ────────────────────────────────────────────

    fn compile_fn(&mut self, f: &MirFn) -> Result<(), CodegenError> {
        let func = *self.fns.get(&f.id).unwrap();

        // 1) bloco entry com allocas para todos os locals
        let entry = self.ctx.append_basic_block(func, "entry");
        self.builder.position_at_end(entry);

        let mut local_ptrs: Vec<PointerValue<'ctx>> = Vec::with_capacity(f.locals.len());
        for l in &f.locals {
            let ty = self.llvm_type(&l.ty);
            let ptr = self.builder
                .build_alloca(ty, &format!("_{}", l.id.0))
                .map_err(builder_err)?;
            local_ptrs.push(ptr);
        }

        // Store dos params nos respectivos locals.
        for (i, param_local) in f.params.iter().enumerate() {
            let val = func.get_nth_param(i as u32).expect("param presente");
            let ptr = local_ptrs[param_local.0 as usize];
            self.builder.build_store(ptr, val).map_err(builder_err)?;
        }

        // 2) cria todos os blocos antes de compilar para suportar forward jumps
        let mut blocks: Vec<LlvmBlock<'ctx>> = Vec::with_capacity(f.blocks.len());
        for b in &f.blocks {
            blocks.push(self.ctx.append_basic_block(func, &format!("bb{}", b.id.0)));
        }

        // 3) entry → primeiro block do MIR
        self.builder
            .build_unconditional_branch(blocks[f.entry.0 as usize])
            .map_err(builder_err)?;

        // 4) compila cada bloco
        for b in &f.blocks {
            self.compile_block(b, &blocks, &local_ptrs, f)?;
        }

        Ok(())
    }

    fn compile_block(
        &self,
        b: &BasicBlock,
        blocks: &[LlvmBlock<'ctx>],
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
    ) -> Result<(), CodegenError> {
        self.builder.position_at_end(blocks[b.id.0 as usize]);

        for stmt in &b.stmts {
            self.compile_stmt(stmt, locals, f)?;
        }

        // Terminator
        match &b.terminator {
            Terminator::Goto(target) => {
                self.builder
                    .build_unconditional_branch(blocks[target.0 as usize])
                    .map_err(builder_err)?;
            }
            Terminator::If { cond, then, otherwise } => {
                let c = self.load_local_as_bool(locals, f, *cond)?;
                self.builder
                    .build_conditional_branch(c, blocks[then.0 as usize], blocks[otherwise.0 as usize])
                    .map_err(builder_err)?;
            }
            Terminator::Return(opt) => {
                match opt {
                    Some(l) => {
                        let v = self.load_local(locals, f, *l)?;
                        self.builder.build_return(Some(&v)).map_err(builder_err)?;
                    }
                    None => {
                        self.builder.build_return(None).map_err(builder_err)?;
                    }
                }
            }
            Terminator::Unreachable => {
                self.builder.build_unreachable().map_err(builder_err)?;
            }
        }
        Ok(())
    }

    fn compile_stmt(
        &self,
        s: &Statement,
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
    ) -> Result<(), CodegenError> {
        match s {
            Statement::Assign { local, rvalue, .. } => {
                let dest_ty = f.locals[local.0 as usize].ty.clone();
                let v = self.compile_rvalue(rvalue, locals, f, &dest_ty)?;
                self.builder
                    .build_store(locals[local.0 as usize], v)
                    .map_err(builder_err)?;
            }
            Statement::Store { place, value, .. } => {
                let ptr = self.place_ptr(place, locals, f)?;
                let v = self.operand_value(value, locals, f)?;
                self.builder.build_store(ptr, v).map_err(builder_err)?;
            }
            Statement::Drop { local, .. } => {
                self.compile_drop(*local, locals, f)?;
            }
            Statement::Nop => {}
        }
        Ok(())
    }

    /// Emite código de drop para `local` conforme o tipo:
    /// - **Array(_):** carrega `(ptr, len)` da struct, calcula
    ///   `len * elem_size` e chama `vex_array_drop(ptr, n_bytes)`.
    /// - **Str / Struct / outros owning:** no-op por ora (strings vivem
    ///   em `.rodata`; structs sem campos heap-resident não precisam).
    /// - **Tipos Copy:** no-op (não deveria nem chegar aqui — análise
    ///   filtra, mas mantemos defensivo).
    fn compile_drop(
        &self,
        local: LocalId,
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
    ) -> Result<(), CodegenError> {
        let ty = f.locals[local.0 as usize].ty.clone();
        match &ty {
            Ty::Array(elem) => {
                // Carrega fat pointer da alocação.
                let arr_ty = self.array_struct_type();
                let fat = self.builder
                    .build_load(arr_ty, locals[local.0 as usize], "arr_drop_ld")
                    .map_err(builder_err)?;
                let fat = fat.into_struct_value();
                let ptr_v = self.builder
                    .build_extract_value(fat, 0, "arr_ptr")
                    .map_err(builder_err)?;
                let len_v = self.builder
                    .build_extract_value(fat, 1, "arr_len")
                    .map_err(builder_err)?
                    .into_int_value();

                // n_bytes = len * elem_size
                let elem_size = self.ctx.i64_type().const_int(self.elem_size_bytes(elem), false);
                let nbytes = self.builder
                    .build_int_mul(len_v, elem_size, "arr_nbytes")
                    .map_err(builder_err)?;

                self.builder
                    .build_call(
                        self.runtime.array_drop,
                        &[ptr_v.into(), nbytes.into()],
                        "arr_drop",
                    )
                    .map_err(builder_err)?;
            }
            _ => {
                // Sem heap pra liberar — drop é no-op.
            }
        }
        Ok(())
    }

    // ── Lvalues / loads / stores ────────────────────────────────────────

    fn place_ptr(
        &self,
        p: &Place,
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let mut ptr = locals[p.local.0 as usize];
        let mut cur_ty = f.locals[p.local.0 as usize].ty.clone();

        for proj in &p.projections {
            match proj {
                Projection::Field(name) => {
                    let Ty::Struct(sid) = &cur_ty else {
                        return Err(CodegenError::Llvm("field access em não-struct".into()));
                    };
                    let st = *self.structs.get(sid).unwrap();
                    // Encontra índice do campo via ordem de declaração.
                    // (Mantemos cópia de field list no codegen seria mais limpo;
                    // por ora, inferimos do MIR via passagem auxiliar.)
                    let idx = self.field_index(*sid, name);
                    ptr = self.builder
                        .build_struct_gep(st, ptr, idx, &format!("gep_{name}"))
                        .map_err(builder_err)?;
                    cur_ty = self.field_type(*sid, name);
                }
                Projection::Index(_) | Projection::Deref => {
                    return Err(CodegenError::Llvm("projeção não suportada no MVP".into()));
                }
            }
        }
        Ok(ptr)
    }

    fn load_local(
        &self,
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
        id: LocalId,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let ty = self.llvm_type(&f.locals[id.0 as usize].ty);
        self.builder
            .build_load(ty, locals[id.0 as usize], &format!("ld_{}", id.0))
            .map_err(builder_err)
    }

    fn load_local_as_bool(
        &self,
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
        id: LocalId,
    ) -> Result<inkwell::values::IntValue<'ctx>, CodegenError> {
        let v = self.load_local(locals, f, id)?;
        match v {
            BasicValueEnum::IntValue(iv) => Ok(iv),
            _ => Err(CodegenError::Llvm("esperava i1 como condição".into())),
        }
    }

    fn operand_value(
        &self,
        o: &Operand,
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        match o {
            Operand::Local(l) => self.load_local(locals, f, *l),
            Operand::Const(c) => Ok(self.const_value(c)),
        }
    }

    fn const_value(&self, c: &Const) -> BasicValueEnum<'ctx> {
        match c {
            Const::Int(v)   => self.ctx.i64_type().const_int(*v as u64, true).into(),
            Const::Float(v) => self.ctx.f64_type().const_float(*v).into(),
            Const::Bool(v)  => self.ctx.bool_type().const_int(*v as u64, false).into(),
            Const::Str(s) => {
                // Cria global string com terminator null.
                let g = self.builder
                    .build_global_string_ptr(s, "vex_str")
                    .expect("global string");
                g.as_pointer_value().into()
            }
            Const::Unit => self.ctx.i8_type().const_int(0, false).into(),
        }
    }

    // ── Rvalues ─────────────────────────────────────────────────────────

    fn compile_rvalue(
        &self,
        r: &Rvalue,
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
        dest_ty: &Ty,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        match r {
            Rvalue::Use(o) => self.operand_value(o, locals, f),
            Rvalue::BinaryOp { op, lhs, rhs } => {
                let l = self.operand_value(lhs, locals, f)?;
                let r = self.operand_value(rhs, locals, f)?;
                self.compile_binop(*op, l, r)
            }
            Rvalue::UnaryOp { op, val } => {
                let v = self.operand_value(val, locals, f)?;
                self.compile_unop(*op, v)
            }
            Rvalue::Call { callee, args } => self.compile_call(callee, args, locals, f),
            Rvalue::Field { obj, field } => {
                let ty = &f.locals[obj.0 as usize].ty;
                let Ty::Struct(sid) = ty else {
                    return Err(CodegenError::Llvm("field em não-struct".into()));
                };
                let st = *self.structs.get(sid).unwrap();
                let idx = self.field_index(*sid, field);
                let ptr = self.builder
                    .build_struct_gep(st, locals[obj.0 as usize], idx, "field_gep")
                    .map_err(builder_err)?;
                let field_ty = self.llvm_type(&self.field_type(*sid, field));
                self.builder.build_load(field_ty, ptr, "field_ld").map_err(builder_err)
            }
            Rvalue::Ref { place, .. } => {
                let ptr = self.place_ptr(place, locals, f)?;
                Ok(ptr.as_basic_value_enum())
            }
            Rvalue::StructInit { struct_id, fields } => {
                let st = *self.structs.get(struct_id).unwrap();
                // Alloca temporária + GEP por campo + store.
                let tmp = self.builder
                    .build_alloca(st, "struct_tmp")
                    .map_err(builder_err)?;
                for (name, op) in fields {
                    let idx = self.field_index(*struct_id, name);
                    let ptr = self.builder
                        .build_struct_gep(st, tmp, idx, "init_gep")
                        .map_err(builder_err)?;
                    let v = self.operand_value(op, locals, f)?;
                    self.builder.build_store(ptr, v).map_err(builder_err)?;
                }
                let loaded = self.builder.build_load(st, tmp, "struct_ld").map_err(builder_err)?;
                Ok(loaded)
            }
            Rvalue::ArrayInit { items } => {
                // dest_ty = Array(elem). Aloca heap, popula, monta fat ptr.
                let Ty::Array(elem_box) = dest_ty else {
                    return Err(CodegenError::Llvm(
                        "ArrayInit em destino não-array".into(),
                    ));
                };
                let elem_ty = (**elem_box).clone();
                let elem_llty = self.llvm_type(&elem_ty);
                let elem_size = self.elem_size_bytes(&elem_ty);
                let len = items.len() as u64;

                // n_bytes = elem_size * len
                let nbytes_const = self.ctx.i64_type().const_int(elem_size * len.max(1), false);
                let ptr = self.builder
                    .build_call(
                        self.runtime.array_alloc,
                        &[nbytes_const.into()],
                        "arr_alloc",
                    )
                    .map_err(builder_err)?;
                let raw_ptr = match ptr.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
                    _ => return Err(CodegenError::Llvm("vex_array_alloc não retornou ptr".into())),
                };

                // Popula elementos via GEP + store.
                for (i, op) in items.iter().enumerate() {
                    let idx_v = self.ctx.i64_type().const_int(i as u64, false);
                    let slot = unsafe {
                        self.builder.build_gep(
                            elem_llty, raw_ptr, &[idx_v], &format!("arr_init_{i}"),
                        )
                    }
                    .map_err(builder_err)?;
                    let v = self.operand_value(op, locals, f)?;
                    self.builder.build_store(slot, v).map_err(builder_err)?;
                }

                // Monta { ptr, len } via insertvalue para evitar alloca.
                let arr_ty = self.array_struct_type();
                let undef = arr_ty.get_undef();
                let with_ptr = self.builder
                    .build_insert_value(undef, raw_ptr, 0, "arr_setptr")
                    .map_err(builder_err)?;
                let len_v = self.ctx.i64_type().const_int(len, false);
                let with_len = self.builder
                    .build_insert_value(with_ptr, len_v, 1, "arr_setlen")
                    .map_err(builder_err)?;
                Ok(with_len.as_basic_value_enum())
            }
            Rvalue::Index { obj, idx } => {
                // Array LIVE no `obj` é fat pointer { ptr, i64 }.
                let arr_ty = self.array_struct_type();
                let fat = self.builder
                    .build_load(arr_ty, locals[obj.0 as usize], "arr_ld")
                    .map_err(builder_err)?
                    .into_struct_value();
                let raw_ptr = self.builder
                    .build_extract_value(fat, 0, "arr_ptr")
                    .map_err(builder_err)?
                    .into_pointer_value();

                // Elem type via tipo do local `obj`.
                let elem_ty = match &f.locals[obj.0 as usize].ty {
                    Ty::Array(inner) => (**inner).clone(),
                    _ => return Err(CodegenError::Llvm("Index em não-array".into())),
                };
                let elem_llty = self.llvm_type(&elem_ty);

                let idx_v = self.load_local(locals, f, *idx)?.into_int_value();
                let slot = unsafe {
                    self.builder.build_gep(elem_llty, raw_ptr, &[idx_v], "arr_idx")
                }
                .map_err(builder_err)?;
                self.builder
                    .build_load(elem_llty, slot, "arr_ld_elem")
                    .map_err(builder_err)
            }
        }
    }

    fn compile_binop(
        &self,
        op: BinOp,
        l: BasicValueEnum<'ctx>,
        r: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        use BinOp::*;

        // Branch integer vs float pelo shape dos operandos.
        if let (BasicValueEnum::IntValue(li), BasicValueEnum::IntValue(ri)) = (l, r) {
            let v: BasicValueEnum = match op {
                Add => self.builder.build_int_add(li, ri, "add").map_err(builder_err)?.into(),
                Sub => self.builder.build_int_sub(li, ri, "sub").map_err(builder_err)?.into(),
                Mul => self.builder.build_int_mul(li, ri, "mul").map_err(builder_err)?.into(),
                Div => self.builder.build_int_signed_div(li, ri, "div").map_err(builder_err)?.into(),
                Mod => self.builder.build_int_signed_rem(li, ri, "rem").map_err(builder_err)?.into(),
                Eq  => self.builder.build_int_compare(IntPredicate::EQ,  li, ri, "eq" ).map_err(builder_err)?.into(),
                Neq => self.builder.build_int_compare(IntPredicate::NE,  li, ri, "neq").map_err(builder_err)?.into(),
                Lt  => self.builder.build_int_compare(IntPredicate::SLT, li, ri, "lt" ).map_err(builder_err)?.into(),
                Gt  => self.builder.build_int_compare(IntPredicate::SGT, li, ri, "gt" ).map_err(builder_err)?.into(),
                Lte => self.builder.build_int_compare(IntPredicate::SLE, li, ri, "lte").map_err(builder_err)?.into(),
                Gte => self.builder.build_int_compare(IntPredicate::SGE, li, ri, "gte").map_err(builder_err)?.into(),
                And => self.builder.build_and(li, ri, "and").map_err(builder_err)?.into(),
                Or  => self.builder.build_or(li, ri, "or").map_err(builder_err)?.into(),
            };
            return Ok(v);
        }
        if let (BasicValueEnum::FloatValue(lf), BasicValueEnum::FloatValue(rf)) = (l, r) {
            let v: BasicValueEnum = match op {
                Add => self.builder.build_float_add(lf, rf, "fadd").map_err(builder_err)?.into(),
                Sub => self.builder.build_float_sub(lf, rf, "fsub").map_err(builder_err)?.into(),
                Mul => self.builder.build_float_mul(lf, rf, "fmul").map_err(builder_err)?.into(),
                Div => self.builder.build_float_div(lf, rf, "fdiv").map_err(builder_err)?.into(),
                Mod => self.builder.build_float_rem(lf, rf, "frem").map_err(builder_err)?.into(),
                Eq  => self.builder.build_float_compare(FloatPredicate::OEQ, lf, rf, "feq").map_err(builder_err)?.into(),
                Neq => self.builder.build_float_compare(FloatPredicate::ONE, lf, rf, "fne").map_err(builder_err)?.into(),
                Lt  => self.builder.build_float_compare(FloatPredicate::OLT, lf, rf, "flt").map_err(builder_err)?.into(),
                Gt  => self.builder.build_float_compare(FloatPredicate::OGT, lf, rf, "fgt").map_err(builder_err)?.into(),
                Lte => self.builder.build_float_compare(FloatPredicate::OLE, lf, rf, "fle").map_err(builder_err)?.into(),
                Gte => self.builder.build_float_compare(FloatPredicate::OGE, lf, rf, "fge").map_err(builder_err)?.into(),
                And | Or => return Err(CodegenError::Llvm("&&/|| em float inválido".into())),
            };
            return Ok(v);
        }
        Err(CodegenError::Llvm("binop com operandos de tipos incompatíveis no codegen".into()))
    }

    fn compile_unop(
        &self,
        op: UnaryOp,
        v: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        match (op, v) {
            (UnaryOp::Neg, BasicValueEnum::IntValue(i))   =>
                Ok(self.builder.build_int_neg(i, "neg").map_err(builder_err)?.into()),
            (UnaryOp::Neg, BasicValueEnum::FloatValue(f)) =>
                Ok(self.builder.build_float_neg(f, "fneg").map_err(builder_err)?.into()),
            (UnaryOp::Not, BasicValueEnum::IntValue(i))   =>
                Ok(self.builder.build_not(i, "not").map_err(builder_err)?.into()),
            _ => Err(CodegenError::Llvm("unop tipo inválido".into())),
        }
    }

    fn compile_call(
        &self,
        callee: &Callee,
        args: &[Operand],
        locals: &[PointerValue<'ctx>],
        f: &MirFn,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let arg_vals: Vec<BasicValueEnum> = args.iter()
            .map(|a| self.operand_value(a, locals, f))
            .collect::<Result<_, _>>()?;
        let arg_meta: Vec<inkwell::values::BasicMetadataValueEnum> =
            arg_vals.iter().copied().map(Into::into).collect();

        match callee {
            Callee::Fn(id) => {
                let func = *self.fns.get(id).ok_or_else(|| {
                    CodegenError::Llvm(format!("fn #{} não declarada", id.0))
                })?;
                let call = self.builder
                    .build_call(func, &arg_meta, "call")
                    .map_err(builder_err)?;
                Ok(match call.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    inkwell::values::ValueKind::Instruction(_) =>
                        self.ctx.i8_type().const_int(0, false).into(),
                })
            }
            Callee::Method { struct_id, name } => {
                let key = (*struct_id, name.clone());
                let Some(fn_id) = self.method_lookup.get(&key).copied() else {
                    return Err(CodegenError::Llvm(format!(
                        "método não resolvido: struct#{} {}", struct_id.0, name
                    )));
                };
                let func = *self.fns.get(&fn_id).ok_or_else(|| {
                    CodegenError::Llvm(format!("fn #{} não declarada", fn_id.0))
                })?;
                let call = self.builder
                    .build_call(func, &arg_meta, "mcall")
                    .map_err(builder_err)?;
                Ok(match call.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    inkwell::values::ValueKind::Instruction(_) =>
                        self.ctx.i8_type().const_int(0, false).into(),
                })
            }
            Callee::Builtin(name) => self.compile_builtin(name, &arg_vals),
        }
    }

    fn compile_builtin(
        &self,
        name: &SmolStr,
        args: &[BasicValueEnum<'ctx>],
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let zero: BasicValueEnum = self.ctx.i8_type().const_int(0, false).into();
        let arg0 = args.first().copied();

        // Built-ins que retornam valor — chamadas separadas pra preservar
        // o BasicValueEnum retornado.
        if let (n, Some(BasicValueEnum::FloatValue(_))) = (name.as_str(), arg0) {
            let target = match n {
                "sqrt" => Some(self.runtime.sqrt),
                "abs"  => Some(self.runtime.abs_f),
                _ => None,
            };
            if let Some(t) = target {
                let arg_meta: Vec<inkwell::values::BasicMetadataValueEnum> =
                    args.iter().copied().map(Into::into).collect();
                let call = self.builder.build_call(t, &arg_meta, "rt_call").map_err(builder_err)?;
                return Ok(match call.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    inkwell::values::ValueKind::Instruction(_) => zero,
                });
            }
        }
        if let (n, Some(BasicValueEnum::IntValue(_))) = (name.as_str(), arg0) {
            if n == "abs" {
                let arg_meta: Vec<inkwell::values::BasicMetadataValueEnum> =
                    args.iter().copied().map(Into::into).collect();
                let call = self.builder.build_call(self.runtime.abs_i, &arg_meta, "rt_call")
                    .map_err(builder_err)?;
                return Ok(match call.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    inkwell::values::ValueKind::Instruction(_) => zero,
                });
            }
        }

        // `len` para arrays: extrai segundo campo do fat pointer.
        if name.as_str() == "len" {
            if let Some(BasicValueEnum::StructValue(sv)) = arg0 {
                let len = self.builder
                    .build_extract_value(sv, 1, "len_extract")
                    .map_err(builder_err)?;
                return Ok(len);
            }
        }

        // print/println: dispatch por tipo do primeiro argumento.
        let fnv = match (name.as_str(), arg0) {
            ("print",   Some(BasicValueEnum::IntValue(v))) if v.get_type().get_bit_width() == 1
                                                              => self.runtime.print_bool,
            ("print",   Some(BasicValueEnum::IntValue(_)))     => self.runtime.print_int,
            ("println", Some(BasicValueEnum::IntValue(v))) if v.get_type().get_bit_width() == 1
                                                              => self.runtime.println_bool,
            ("println", Some(BasicValueEnum::IntValue(_)))     => self.runtime.println_int,
            ("print",   Some(BasicValueEnum::FloatValue(_)))   => self.runtime.print_float,
            ("println", Some(BasicValueEnum::FloatValue(_)))   => self.runtime.println_float,
            ("print",   Some(BasicValueEnum::PointerValue(_))) => self.runtime.print_str,
            ("println", Some(BasicValueEnum::PointerValue(_))) => self.runtime.println_str,
            _ => return Ok(zero),
        };

        let arg_meta: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().copied().map(Into::into).collect();
        self.builder.build_call(fnv, &arg_meta, "rt_call").map_err(builder_err)?;
        Ok(zero)
    }

    // ── Helpers de struct ───────────────────────────────────────────────

    fn field_index(&self, sid: DefId, name: &SmolStr) -> u32 {
        self.struct_fields.get(&sid)
            .and_then(|fs| fs.iter().position(|(n, _)| n == name))
            .map(|p| p as u32)
            .unwrap_or(0)
    }
    fn field_type(&self, sid: DefId, name: &SmolStr) -> Ty {
        self.struct_fields.get(&sid)
            .and_then(|fs| fs.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone()))
            .unwrap_or(Ty::Error)
    }
}

fn builder_err(e: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError::Llvm(e.to_string())
}
