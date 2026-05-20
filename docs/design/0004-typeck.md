# ADR 0004 — Type checker bidirecional simples

**Data:** 2026-05-19
**Status:** Aceito (Fase 4 implementada)
**Autor:** Jeff Almeida

## Contexto

Vex precisa validar tipos antes de gerar código. Opções consideradas:

1. **Hindley-Milner com unification** (estilo OCaml, Roc)
2. **Bidirecional simples** (estilo TypeScript "no infer")
3. **Apenas anotações explícitas** (estilo Go inicial)

## Decisão

**Bidirecional simples + inferência local para `let`.**

- Top-down quando há tipo esperado (retorno, anotação `let x: T = ...`,
  parâmetros de chamada, campos de struct literal).
- Bottom-up para inferir o tipo de expressões livres.
- **Sem unification.** Tipos primitivos batem por igualdade estrutural;
  não há generics no MVP. Generics monomorfizados ficam para v1.2+.

Por que não HM completo:
- Vex não tem polimorfismo paramétrico na v0.1. HM sem generics é overkill.
- Erros HM (`occurs check`, ciclos em unification) confundem usuários
  novos. Mensagens de "esperava X, encontrei Y" são mais legíveis.
- Implementação cabe em ~600 linhas. HM exigiria substituição,
  generalização, instanciação — facilmente 2000+ linhas.

## Arquitetura

```
crates/vex-typeck/src/
├── lib.rs    # API pública check_module()
├── ty.rs     # Ty enum, lower_hir_type, unify, builtin_signature
├── env.rs    # Env (fn sigs, struct fields, methods) pré-computado
└── check.rs  # Checker — percorre HIR, acumula erros
```

### Tipos (`Ty`)

```rust
enum Ty {
    Int, Float, Bool, Str, Char, Void,
    Struct(DefId),
    Array(Box<Ty>),
    Ref { mutable: bool, inner: Box<Ty> },
    Any,    // built-ins polimórficos
    Error,  // propaga sem cascatear erros adicionais
}
```

`Ty::Any` é compatível com tudo (usado em `print`, `len`, etc.).
`Ty::Error` propaga: operações sobre `Error` produzem `Error` sem
gerar diagnóstico secundário. Mesma técnica do `tcx.types.err` do rustc.

### Pré-passagem (`Env`)

Antes de checar bodies, constrói tabelas:
- `fns: DefId → FnSig` para chamadas diretas.
- `structs: DefId → IndexMap<field_name, Ty>` para field access e literals.
- `methods: (struct_id, name) → FnSig` para method dispatch.

`Self` em assinaturas de impl methods é resolvido para `Ty::Struct(target_id)`
durante a construção do `Env`.

### Checagem

`Checker` mantém:
- pilha de escopos locais (`Vec<IndexMap<DefId, Ty>>`)
- `self_ty` corrente (dentro de impl)
- `expected_ret` corrente (para validar `return`)
- vetor de erros acumulados

`infer_expr(e) -> Ty` faz bottom-up. Erros não abortam — `Ty::Error` é
retornado e propaga.

### Erros (15 variantes)

| Erro | Quando |
|------|--------|
| `Mismatch` | tipos não unificam (retorno, let anot., args de call, fields) |
| `BadBinOp` | `+ - * / %` em tipos não-numéricos, `==` em tipos diferentes, etc. |
| `BadUnaryOp` | `-x` em não-numérico, `!x` em não-bool |
| `BadArity` | chamada com número errado de args |
| `NotCallable` | tentativa de chamar não-fn |
| `UnknownField` | acesso a campo inexistente em struct |
| `UnknownMethod` | método não existe para o receptor |
| `BadReturn` | retorno do tipo errado |
| `NonBoolCond` | condição de `if`/`while` não-bool |
| `NonIntIndex` | índice de array não-int |
| `NotIndexable` | indexação em tipo não-array |
| `NoFields` | `.field` em tipo sem campos |
| `MissingField` | struct literal incompleto |
| `ExtraField` | struct literal com campo desconhecido |

## Built-ins (até stdlib formal — Fase 8)

| Função | Assinatura |
|--------|-----------|
| `print`, `println` | `(Any) -> void` |
| `input` | `(str) -> str` |
| `read_file` | `(str) -> str` |
| `write_file` | `(str, str) -> void` |
| `to_int` | `(str) -> int` |
| `to_float` | `(str) -> float` |
| `to_str` | `(Any) -> str` |
| `len` | `(Any) -> int` (aceita arrays e str) |
| `push` | `(Any, Any) -> void` |
| `pop` | `(Any) -> Any` |
| `sqrt`, `abs` | `(float) -> float` |
| `min`, `max` | `(Any, Any) -> Any` |

`Any` é placeholder até a stdlib introduzir generics monomorfizados.

## Method dispatch

Para `recv.method(args)`:
1. Inferir tipo de `recv` → `Ty::Struct(id)` ou `Ty::Ref { inner: Struct(id) }`.
2. Lookup em `env.methods[(id, method_name)]`.
3. Skipar o primeiro parâmetro (`self`) ao checar args.

Sem auto-deref complexo. `&self` vs `self` distinguidos por tipo. Quem
recebe `&P` chama `(&p).method()` (auto-promoção é responsabilidade do
codegen, não do typeck).

## Limitações conscientes do MVP

- **Sem validação de retorno em todos os paths.** `if x { return 1 }`
  sem else compila — esperado, segue convenção Rust/Zig. Validação
  precisa de CFG (Fase 5).
- **Match não-exaustivo.** Patterns são checados estruturalmente, mas
  exhaustiveness vira responsabilidade de typeck avançado pós-v0.1.
- **Sem operadores compostos.** `+=`, `-=` são lexados mas não chegam
  como `BinOp` no AST ainda. Adicionar no parser quando útil.
- **Sem inferência cross-statement.** `let x; ... x = 5`: x permanece
  sem tipo. Esperado declarar com valor inicial.

## Integração

`vex-driver::compile` agora roda:
```
source → lex → parse → resolve → typeck → [codegen ainda pendente]
```

`vex check` mostra erros de tipo com label + hint contextual via
`miette` (driver::typeck_hint).

## Próximos passos (Fase 5)

- MIR: lowering de HIR validado para CFG.
- Inserção de gen-ref checks (Vale-style ownership).
- Linear-type validation para resources marcados.
- ASAP destruction (último uso).
