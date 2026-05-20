//! Ambiente do type checker — sinais globais + escopos locais.

use indexmap::IndexMap;
use smol_str::SmolStr;
use vex_hir::{DefId, HirField, HirFn, HirImpl, HirItem, HirModule, HirStruct};

use crate::ty::{lower_hir_type, Ty};

/// Assinatura de função: tipos dos parâmetros + tipo de retorno.
#[derive(Debug, Clone)]
pub struct FnSig {
    pub params: Vec<Ty>,
    pub ret: Ty,
}

/// Ambiente computado em uma pré-passagem sobre o HIR. Contém tudo o que
/// o checker precisa consultar **sem percorrer** corpos.
pub struct Env<'h> {
    pub module: &'h HirModule,
    /// Assinaturas indexadas por `DefId` da função.
    pub fns: IndexMap<DefId, FnSig>,
    /// Campos da struct indexados por `DefId` da struct.
    pub structs: IndexMap<DefId, IndexMap<SmolStr, Ty>>,
    /// Métodos: (struct DefId, nome) → FnSig.
    pub methods: IndexMap<(DefId, SmolStr), FnSig>,
}

impl<'h> Env<'h> {
    pub fn build(module: &'h HirModule) -> Self {
        let mut env = Self {
            module,
            fns: IndexMap::new(),
            structs: IndexMap::new(),
            methods: IndexMap::new(),
        };
        env.collect_structs();
        env.collect_fns();
        env.collect_impls();
        env
    }

    fn collect_structs(&mut self) {
        for item in &self.module.items {
            if let HirItem::Struct(s) = item {
                let fields = lower_fields(s, None);
                self.structs.insert(s.id, fields);
            }
        }
    }

    fn collect_fns(&mut self) {
        for item in &self.module.items {
            if let HirItem::Fn(f) = item {
                let sig = lower_fn_sig(f, None);
                self.fns.insert(f.id, sig);
            }
        }
    }

    fn collect_impls(&mut self) {
        // Necessário clonar a lista pois construímos enquanto iteramos.
        let impls: Vec<HirImpl> = self.module.items.iter().filter_map(|i| match i {
            HirItem::Impl(im) => Some(im.clone()),
            _ => None,
        }).collect();

        for im in impls {
            let self_ty = Ty::Struct(im.target);
            for m in &im.methods {
                let sig = lower_fn_sig(m, Some(&self_ty));
                self.methods.insert((im.target, m.name.clone()), sig);
            }
        }
    }
}

fn lower_fields(s: &HirStruct, self_ty: Option<&Ty>) -> IndexMap<SmolStr, Ty> {
    s.fields.iter()
        .map(|(n, HirField { ty, .. })| (n.clone(), lower_hir_type(ty, self_ty)))
        .collect()
}

fn lower_fn_sig(f: &HirFn, self_ty: Option<&Ty>) -> FnSig {
    let params = f.params.iter()
        .map(|p| {
            // Parâmetro `self`: tipo é o self_ty do impl atual.
            if p.name == "self" {
                self_ty.cloned().unwrap_or(Ty::Error)
            } else {
                lower_hir_type(&p.ty, self_ty)
            }
        })
        .collect();
    let ret = lower_hir_type(&f.ret_type, self_ty);
    FnSig { params, ret }
}
