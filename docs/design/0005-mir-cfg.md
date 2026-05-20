# ADR 0005 — MIR como Control-Flow Graph

**Data:** 2026-05-19
**Status:** Aceito (Fase 5 implementada — split em 5a/5b)
**Autor:** Jeff Almeida

## Contexto

Entre o HIR (árvore aninhada) e o LLVM IR (instruções planas com basic
blocks) precisamos de uma IR intermediária. Opções:

1. **Compilar HIR direto para LLVM IR.** Simples mas mistura análise de
   ownership/control-flow com codegen — atrapalha quando quisermos:
   - validar gen-ref checks
   - implementar dead-code elimination Vex-specific
   - servir LSP com tipos por linha
2. **MIR estilo rustc.** CFG explícito com basic blocks + terminators.
   Mais código, mas separação de responsabilidades.

## Decisão

**MIR estilo rustc, simplificado.** Implementado em `vex-mir`.

### Estrutura

```
crates/vex-mir/src/
├── lib.rs    # exporta MirModule, lower_module, pretty_print_module
├── mir.rs    # tipos: LocalId, BlockId, MirFn, BasicBlock, Statement, Rvalue,
│             #        Operand, Place, Projection, Callee, Terminator
├── lower.rs  # HIR → MIR via FnLowerer recursivo
└── pretty.rs # pretty-print para `vex check --emit=mir`
```

### Características

- **Locals indexados** (`LocalId(u32)`): parâmetros + lets + temporários.
- **Basic blocks** (`BlockId(u32)`): cada um termina em um `Terminator`.
- **Operandos atômicos**: `Operand` é `Local(_)` ou `Const(_)` — nunca
  cálculo embutido. Cálculos viram `Rvalue::BinaryOp { lhs, rhs }`.
- **Places para lvalues**: `field.x`, `arr[i]` representados como
  `Place { local, projections }` (rustc-style).
- **Lowering de `for`**: convertido em `i = 0; len = len(iter); while
  i < len { x = iter[i]; body; i += 1 }`. Não é loop genérico ainda.
- **Lowering de `match`**: stub no MVP (lowera só scrutinee). Match real
  vira jump table no codegen (pós-MVP).

### Split Fase 5 → 5a/5b

A Fase 5 original do roadmap incluía:
- (a) MIR CFG
- (b) Ownership baseline (gen refs + linear types + ASAP destruction)

A parte (b) requer **análise de movimentos sobre o CFG** — ou seja,
precisa do CFG construído. Para não bloquear o codegen (Fase 6) com
ownership ainda em design, divido:

- **Fase 5a (concluída):** lowering HIR → MIR. CFG estruturado, places,
  rvalues, terminators.
- **Fase 5b (a fazer):** análise sobre o MIR — last-use, gen-ref check
  insertion, linear-type validation, ASAP drop points. Pode ser feito
  em paralelo com codegen (que aceita MIR sem checks por ora).

### Limitações conscientes do MVP

- **Tipo dos temporários é aproximado.** `infer_expr_type` no lowerer
  é uma cópia conservadora — não cobre `Call` retornando tipo certo
  (fica `Ty::Error`). Codegen vai precisar consultar a tabela de
  signatures do typeck. Alternativa futura: typeck emite tabela
  `expr_id → Ty` consumida pelo lowerer.
- **Sem dead-block removal.** Se um statement aparece após `return`,
  cria-se bloco morto. Limpeza fica para uma passada futura.
- **Match stub.** Lowering completo de match exige decision trees
  (algoritmo de Maranget). Pós-MVP.
- **Closures não existem.** `Callee::Builtin` é fallback para
  identifiers locais sendo chamados — não é caminho real.

### CLI: `vex check --emit=mir`

Útil para debug e inspeção. Imprime o CFG textualmente:

```
fn #1 fib(_0) -> int {
    let _0 : int ;       // n
    let _1 : bool ;      // _t0
    ...

  bb0:
    _1 = _0 <= 1i
    -> if _1 then bb1 else bb2

  bb1:
    -> return _0

  bb2:
    ...
}
```

## Próximos passos

- **Fase 5b:** ownership analysis sobre o MIR.
- **Fase 6:** codegen MIR → LLVM IR via inkwell.
- **Match lowering:** decision trees (pós-MVP).
- **MIR optimization passes:** const propagation, dead block removal,
  copy elimination.
