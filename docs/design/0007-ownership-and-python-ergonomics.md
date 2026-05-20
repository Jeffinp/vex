# ADR 0007 — Ownership analysis + ergonomia Python-like

**Data:** 2026-05-20
**Status:** Aceito (5b parcial implementado, ergonomia v1)
**Autor:** Jeff Almeida

## Contexto

Duas frentes convergentes nesta fase:

1. **Ownership análise** (5b) — sobre o MIR/CFG, prover infraestrutura
   para ASAP destruction (Mojo), drop placement, last-use refinado,
   detecção de use-after-move.
2. **Ergonomia Python-like** — promessa fundadora: *legível como Python,
   seguro como Rust, rápido como C++*. O `hello.vex` atual em `fn main() -> void`
   é cópia de Rust. Precisa parecer Python.

## Pesquisa (5b)

Relatório do agent architect cobriu Vale (gen refs), Austral (linear
types), Mojo (ASAP), Rust (drop elaboration), Hylo. Recomendação:
**Opção A híbrida** — ASAP destruction + drop elaboration (sem flags
quando liveness for precisa) + linear types opt-in.

Implementação completa estimada em **~3.2k LOC, 13 dias**. Esta sub-fase
entrega a **infraestrutura analítica** (~700 LOC, 1 dia); emissão real
de drops + gen-refs + linear types ficam para 5b.2-5b.4.

Fontes-chave:
- Mojo: <https://mojolang.org/docs/manual/lifecycle/death/>
- Vale: <https://verdagon.dev/blog/generational-references>
- Austral: <https://borretti.me/article/how-australs-linear-type-checker-works>
- Rust drop: <https://rustc-dev-guide.rust-lang.org/mir/drop-elaboration.html>

## Decisões

### 1. Ergonomia Python-like (mudanças semânticas mínimas)

#### Script mode — top-level statements

Arquivos podem misturar items (fn/struct/impl/const/use) com statements
no topo. Statements top-level viram corpo de um `main()` sintético.

```vex
// Hello world Vex agora:
println("Hello, Vex!")
```

Implementação em `crates/vex-parser/src/lib.rs::parse`: heurística
`peek_is_item` (pula `pub`/`comptime`, depois esperando `fn`/`struct`/…)
decide se é item ou stmt. Conflito (arquivo tem `fn main()` **e**
stmts top-level) → erro de parse.

#### `let` opcional via auto-declaração

Primeira atribuição num escopo declara variável. Conceito Python:

```vex
x = 42         // primeira atribuição: declara
x = x + 1      // assignment subsequente: atualiza
println(x)     // OK
```

Implementação em `vex-hir::resolve::resolve_expr_stmt`: quando vê
`Stmt::Expr(Assign { target: Ident(name), value })` e `name` não está
no escopo, emite `HirStmt::Let { mutable: true, ... }` em vez de
`Assign`. Auto-declares são `mut` por padrão (Python tem rebinding).

Trade-off: typos em assignment não são detectados (assign para `foo`
inexistente vira declaração silenciosa). Mesmo trade-off do Python.
Aceito — Vex prioriza ergonomia neste ponto; typeck/linter futuros
podem alertar.

#### Aliases sintáticos

- **`def`** alias para `fn` (Python)
- **`class`** alias para `struct` (Python)

Implementado no lexer mapeando ambos para o mesmo variante de `Token`.
Formatter emite a forma canonical Python-friendly (`def` e `class`).
Programadores que preferem `fn`/`struct` podem usar; ferramentas
normalizam.

### 2. Ownership analysis (`vex-mir::ownership`)

Crate `vex-mir` ganha módulo `ownership` com 3 saídas:

| Saída | Para que serve |
|-------|----------------|
| `uses: Map<LocalId, Vec<Location>>` | todas as posições de uso (precisão de statement) |
| `last_use: Map<LocalId, Location>` | última posição CFG — base para ASAP drops |
| `drop_points: Vec<DropPoint>` | locais que precisam drop + onde |
| `errors: Vec<OwnershipError>` | use-after-move detectado |

`Location { block, stmt }` aponta para statement específico (não bloco
inteiro). `stmt = u32::MAX` significa "no terminator".

**`is_drop_required(Ty)`** classifica tipos:
- **Copy** (sem drop): Int, Float, Bool, Char, Void, Ref{..}
- **Owning** (precisa drop): Str, Struct(_), Array(_), Any, Error

Esta regra alinha com Mojo: `__copyinit__` é gratuito; tipos com
recursos sempre exigem `__del__`.

### 3. Algoritmo de last-use refinado

Antes (em `liveness::FnLiveness::last_use`): granularidade de
**bloco** — "_x foi visto pela última vez em bb3". Insuficiente para
ASAP — não diz **quando** dentro de bb3.

Agora (em `ownership::analyze`): granularidade de **statement** dentro
do bloco. Permite drop logo após o último uso real:

```
fn t() -> int {
    let p = Ponto { x: 1.0, y: 2.0 }   // bb0.0  — _p criado
    let d = p.distancia(other)          // bb0.5  — _p usado
                                        //         ↑ drop _p aqui (ASAP)
    let z = d + 1.0                     // bb0.7  — _p já dropado
    return z
}
```

### 4. Use-after-move (detecção básica)

Algoritmo simples sobre o CFG (sem dataflow):

1. Para cada `Rvalue::Use(Local)` / `Rvalue::Call { args }` /
   `Rvalue::StructInit { fields }` / `Rvalue::ArrayInit { items }`
   passando local **owning**, registrar `moves[local] = Location`.
2. Em assignments subsequentes a `local`, limpar move (rebind).
3. Em qualquer uso posterior de `local`, comparar `Location` — erro
   se `moved_at < used_at`.

**Limitações conscientes** (documentadas no código):
- Não rastreia move condicional em branches divergentes — se um caminho
  move e outro não, a análise é conservadora (não emite erro).
- Span do move report é placeholder (`0..0`) — refinamento em sub-fase
  posterior.

Para a v0.1 o objetivo é **detectar erros óbvios**; soundness completa
(estilo Rust borrow checker NLL) fica para v1.x.

### 5. Drop emission (a fazer)

Esta fase **não** emite `Statement::Drop` real. Próxima sub-fase
(5b.2): codegen consome `drop_points` e emite chamadas para
`vex_drop_*` no runtime. Por ora, **infra analítica pronta** —
visível via `vex check <arq> --emit=ownership`.

## CLI

```bash
vex check file.vex --emit=ownership
```

Output:
```
fn #2 distancia ownership:
  uses:
    _1 (struct#0) [drop]: bb0.2 bb0.6
    _2 (float): bb0.8
    ...
  last_use:
    _1 → bb0.6
    ...
  drop_points:
    drop _3 at bb0.1
    drop _1 at bb0.6
    drop _7 at bb0.5
```

`[drop]` marca locais owning. Drop points listados são onde codegen
emitirá `vex_drop_*` no futuro.

## Trade-offs aceitos

- **Auto-declaração silencia typos.** Mesma escolha do Python. Linter
  futuro pode alertar.
- **Análise de move conservadora.** Branches divergentes não geram
  erro mesmo quando há move em um caminho. Iteração futura adiciona
  dataflow rigoroso.
- **Drop emission adiada.** Esta sub-fase entrega análise — emissão
  vem com gen-ref tags e linear types numa sub-fase combinada.
- **Aliases `def`/`class` aceitos**, mas canonical é Python-friendly.
  Programadores podem misturar formas; formatter normaliza.

## Próximos passos

- **5b.2:** Codegen consome `drop_points`, emite `vex_drop_str`,
  `vex_drop_struct_<N>`, etc.
- **5b.3:** Gen-ref tags (Vale) opt-in via `Ty::Ref { mutable, .. }`.
  Layout: `{ ptr: i8*, target_gen: u64 }`.
- **5b.4:** Linear types opt-in via sintaxe (decidir entre `File!`,
  `@linear File`, ou `linear File`).
- **5b.5:** Mensagens de erro de move detalhadas com `moved_at` e
  `used_at` spans.
- **6:** Methods dentro de `class` block (auto-impl) — elimina o
  `impl` separado em programas simples.
