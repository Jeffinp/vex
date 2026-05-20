# ADR 0002 — Parser: recursive descent + Pratt

**Data:** 2026-05-19
**Status:** Aceito (Fase 2 implementada)
**Autor:** Jeff Almeida

## Contexto

Vex precisa de um parser. Opções consideradas:

1. **LALRPOP** (parser generator LR(1))
2. **chumsky / winnow** (parser combinators)
3. **Recursive descent hand-written**

## Decisão

**Recursive descent hand-written, com Pratt parsing para expressões.**

Justificativa:

1. **Controle de erros.** Parsers gerados produzem mensagens genéricas
   ("expected token X"). Hand-written permite mensagens contextuais
   ("esperava nome de campo após `:`"). Mensagens de erro são feature da
   Vex — não detalhe de implementação.

2. **Precedente forte.** rustc, Carbon, Zig, e Ruff (após v0.4 em 2024)
   usam hand-written. Ruff migrou de gerador → hand-written e ganhou em
   velocidade e qualidade.

3. **Pratt resolve a complexidade de expressões.** 15+ níveis de
   precedência viraria 15+ funções em recursive descent puro. Pratt
   unifica num único loop parametrizado por *binding power*. Padrão
   recomendado por matklad e adotado por rustc.

4. **Performance previsível.** Sem mágica de macro/codegen — fácil
   profilear e otimizar.

## Trade-offs aceitos

- **Mais código que LALRPOP.** ~700 linhas vs ~150 linhas de grammar.
  Aceito — controle de erros vale.
- **Manutenção manual da gramática.** A spec em `docs/grammar.ebnf` pode
  divergir do parser. Mitigação: testes snapshot sobre `examples/`
  capturam regressões de comportamento.

## Estrutura do crate

```
crates/vex-parser/src/
├── lib.rs       # entrada pública `parse(source) -> Module`
├── cursor.rs    # peek/bump/expect sobre stream de tokens
├── error.rs     # ParseError com variantes spans
├── ty.rs        # parse_type
├── expr.rs      # parse_expr via Pratt
├── stmt.rs      # parse_stmt + parse_block
└── item.rs      # parse_item (fn, struct, impl, const, use)
```

## Tabela de precedência (binding power)

| BP  | Operadores              | Associatividade |
|-----|-------------------------|-----------------|
| 17  | `()` `.` `[]` (postfix) | left            |
| 15  | `-` `!` (prefix)        | right           |
| 11/12 | `*` `/` `%`           | left            |
| 9/10  | `+` `-`               | left            |
| 7/8   | `<` `>` `<=` `>=`     | left            |
| 5/6   | `==` `!=`             | left            |
| 3/4   | `&&`                  | left            |
| 1/2   | `\|\|`                  | left            |
| 0     | `=` (assignment)      | right           |

BP par/ímpar codifica associatividade — esquerda usa o par menor, direita
o ímpar maior. Detalhes em
<https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html>.

## Heurísticas de desambiguação

- **`Ident { ... }` como struct literal:** apenas se o identificador
  começa com maiúscula. Evita conflito com `if cond { body }`.
- **`self`:** parseado como `Expr::SelfRef` em expressões; como
  parâmetro com tipo `Self` em assinaturas de fn.
- **`return` sem valor:** detectado quando o próximo token é `}` ou `;`
  ou EOF.
- **`else if`:** elaborado como `else { if ... }` (bloco com um único
  Stmt::If dentro). Mantém AST regular.

## Próximos passos (Fases futuras)

- **Recovery de erros:** sync points em `;` `}` `fn` `struct` `let`.
  Atualmente o parser para no primeiro erro.
- **Spans em assignment/method-call:** validar que cobrem início→fim.
- **Patterns mais ricos:** tuple, struct destructuring (post-MVP).
