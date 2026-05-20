# ADR 0003 — Name resolution e HIR

**Data:** 2026-05-19
**Status:** Aceito (Fase 3 implementada)
**Autor:** Jeff Almeida

## Contexto

Após o parser produzir uma AST, o compilador precisa transformar
identificadores (`String`) em referências resolvidas (`DefId`) antes de
inferir tipos. Essa transformação compõe duas responsabilidades:

1. **Resolução de nomes** — associar cada uso de identificador à sua
   declaração.
2. **Lowering AST → HIR** — IR intermediária com semântica explícita.

## Decisão

**Implementar HIR + resolver em um único crate (`vex-hir`) com algoritmo
de duas passagens.**

### Por que duas passagens

1. **Passagem 1 — collect items.** Registra todos os items top-level
   (fn, struct, const) em uma tabela de `DefId`s **antes** de visitar
   qualquer corpo. Isso habilita **forward references**: uma função pode
   chamar outra declarada mais abaixo no mesmo módulo, padrão esperado
   em Rust/Go/Zig.

2. **Passagem 2 — resolve bodies.** Percorre cada item resolvendo
   identificadores. Usa pilha de escopos (`Vec<IndexMap<SmolStr, DefId>>`)
   para variáveis locais, com fallback para a tabela global.

Alternativa rejeitada: single-pass com "definição vem antes de uso"
(estilo Python). Quebra ergonomia esperada em linguagens de sistemas.

### Estrutura do HIR

| Tipo | Propósito |
|------|-----------|
| `DefId(u32)` | índice opaco na tabela `defs` |
| `Def { name, kind, span }` | metadados da definição |
| `DefKind` | Fn / Struct / Const / Local / Param / SelfParam |
| `HirModule { defs, items }` | módulo resolvido |
| `HirExpr::Name { id, name, span }` | identificador resolvido |
| `HirExpr::Builtin { name, span }` | built-in stdlib (placeholder até Fase 8) |

Diferença chave vs AST:
- `Expr::Ident("foo")` vira `HirExpr::Name { id: DefId(5), ... }`.
- Struct literals carregam `struct_id: DefId` (validado em resolução).
- Tipos nomeados (`Type::Named("Ponto")`) viram `HirType::Struct(DefId)`.

### Política de erros

O resolver **acumula** erros em vez de abortar no primeiro. Isso
permite que `vex check` reporte todos os erros de uma vez. Quando uma
referência falha, o resolver substitui por um placeholder
(`HirExpr::Builtin` com nome original ou `DefId` recém-alocado para
struct unknown), preservando a estrutura do HIR para análises
subsequentes sem cascatear.

### Variantes de `ResolveError`

| Erro | Quando |
|------|--------|
| `Unknown` | identificador não declarado |
| `Duplicate` | redeclaração no mesmo módulo |
| `UnknownType` | tipo nomeado não encontrado |
| `UnknownStruct` | struct literal de tipo desconhecido |
| `SelfOutsideMethod` | `self` fora de impl block |
| `ImplOnUnknownType` | `impl Foo` para Foo inexistente |
| `InvalidAssignTarget` | `5 = x` (lhs não-lvalue) |

### Built-ins

Enquanto a stdlib formal (Fase 8) não existe, o resolver reconhece um
conjunto fechado de funções built-in (`print`, `println`, `len`, `sqrt`,
`to_int`, etc.). Identificadores nesse conjunto não geram erro de
resolução; viram `HirExpr::Builtin`. O type checker decide se a chamada
é válida.

## Trade-offs aceitos

- **Sem name mangling para impl methods.** Métodos dentro de
  `impl Foo { fn bar() }` recebem `DefId` próprio mas não há tabela
  global `Foo::bar` — Fase 4 resolve method dispatch via tipo do
  receptor.
- **`use` paths são ignorados.** Será implementado quando houver
  stdlib (Fase 8).
- **Patterns de match não validam exaustividade.** Apenas estrutural.
  Exhaustiveness checking é responsabilidade do typeck (Fase 4+).
- **Identifiers em patterns sempre criam binding.** Como Rust. Não
  distinguimos "constante existente" de "novo binding" no resolver;
  typeck pode decidir mais tarde se for necessário.

## Integração com o driver

`vex-driver::compile` agora roda:
```
source → lex → parse → resolve → [futuro: typeck → mir → codegen]
```

Erros de resolução são renderizados via `miette` com label e hint
contextual (ver `resolve_hint` em `vex-driver/src/lib.rs`).

Exemplo de output:
```
  × nome `foo` não declarado neste escopo
   ╭─[arquivo.vex:3:14]
 3 │   let x = foo
   ·           ─┬─
   ·            ╰── não declarada
   ·   help: declare com `let` antes de usar, ou verifique se há erro de digitação
   ╰────
```

## Próximos passos (Fase 4)

- Inferência de tipos sobre o HIR (Hindley-Milner local).
- Validação de campos em `StructLit` (campos existem? tipos batem?).
- Validação de chamadas (aridade + tipos).
- Method dispatch (typeck consulta `impl` blocks pelo `DefId` do receptor).
