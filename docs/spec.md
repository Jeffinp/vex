# Vex — Especificação da Linguagem (v0.1 draft)

> Documento vivo. Atualizado a cada fase do roadmap.

## Filosofia

Vex é uma linguagem de sistemas que busca **rápida como C++, segura como Rust, legível como Python**.

Três princípios não-negociáveis:

1. **Sem fluxo de controle escondido** (princípio do Zig).
   Se não parece uma chamada de função, não é. Sem operator overloading
   surpresa, sem exceptions implícitas, sem alocações invisíveis.
2. **Ergonomia primeiro**. Tipos inferidos onde possível, sintaxe enxuta,
   mensagens de erro extraordinárias (estilo Elm/Rust).
3. **Custo zero quando possível**. Ownership simplificado via *generational
   references* (estilo Vale) + *linear types* opcionais para recursos
   (estilo Austral). Sem GC.

## Sintaxe alvo (informal)

```vex
// declarações
fn nome(p1: T1, p2: T2) -> T3 { ... }
struct Nome { field: T, ... }
impl Nome { fn metodo(self, ...) -> ... { ... } }
let x = expr                  // imutável, tipo inferido
let mut x: int = 0            // mutável, tipo anotado
const PI: float = 3.14        // constante de compilação

// controle
if cond { ... } else { ... }
while cond { ... }
for x in iter { ... }
match val { pat => expr, ... }

// referencias
&x         // borrow imutável
&mut x     // borrow mutável (linear se T: linear)
```

## Tipos primitivos

| Tipo  | LLVM IR | Notas |
|-------|---------|-------|
| int   | i64     | sempre 64-bit |
| float | double  | IEEE-754 64-bit |
| bool  | i1      | |
| str   | { ptr, len } | UTF-8, length-prefixed |
| void  | void    | tipo unit |

## Modelo de ownership

Em desenvolvimento. Ver `docs/design/0002-ownership.md` (futuro).

Resumo:
- Valores por default são **owned** (move semantics).
- Referências `&T` e `&mut T` validadas via *generational references*.
- Tipos marcados como `linear` (file, socket, lock) exigem consumo único.
- Destruição ASAP no último uso (não no final do escopo).

## Gramática

Ver `docs/grammar.ebnf`.
