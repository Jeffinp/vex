# Vex Lang — Roadmap de Implementação
> Linguagem de programação: rápida como C++, segura como Rust, legível como Python.  
> Stack: **Rust** + **LLVM (inkwell)** + **WASM target futuro**

---

## Contexto e filosofia

Vex tem três pilares:
- **Performance** — compila para binário nativo via LLVM, zero GC
- **Safety** — ownership simplificado (menos verboso que Rust, mais seguro que C++)
- **Ergonomia** — sintaxe limpa, tipagem inferida, sem cerimônia

---

## Fase 0 — Setup do projeto (Dia 1)

### Objetivo
Repositório funcional com estrutura base pronta para desenvolver.

### Tarefas

```
vex/
├── Cargo.toml          # workspace
├── crates/
│   ├── vex-lexer/      # tokenização
│   ├── vex-parser/     # AST + parsing
│   ├── vex-typeck/     # type checker
│   ├── vex-codegen/    # LLVM IR
│   └── vex-cli/        # binário `vex`
├── std/                # stdlib futura
├── examples/           # programas .vex de teste
└── tests/              # testes de integração
```

**Dependências no `Cargo.toml` raiz:**
```toml
[workspace]
members = ["crates/*"]

[workspace.dependencies]
inkwell = { git = "https://github.com/TheDan64/inkwell", branch = "master", features = ["llvm17-0"] }
logos = "0.14"          # lexer por macro
ariadne = "0.4"         # erros com highlight bonito
clap = { version = "4", features = ["derive"] }
miette = "7"            # diagnósticos
```

**Entregável:** `cargo build` passa sem erros, estrutura de crates criada.

---

## Fase 1 — Lexer (Dias 2–3)

### Objetivo
Tokenizar qualquer programa `.vex` corretamente.

### Tokens a implementar

```rust
// crates/vex-lexer/src/lib.rs
#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    // Literais
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Int(i64),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    #[regex(r#""[^"]*""#, |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    Str(String),

    #[token("true")]  True,
    #[token("false")] False,

    // Keywords
    #[token("let")]    Let,
    #[token("fn")]     Fn,
    #[token("return")] Return,
    #[token("if")]     If,
    #[token("else")]   Else,
    #[token("while")]  While,
    #[token("for")]    For,
    #[token("in")]     In,
    #[token("struct")] Struct,
    #[token("impl")]   Impl,
    #[token("pub")]    Pub,
    #[token("use")]    Use,
    #[token("import")] Import,
    #[token("mut")]    Mut,
    #[token("match")]  Match,

    // Tipos primitivos
    #[token("int")]    TInt,
    #[token("float")]  TFloat,
    #[token("bool")]   TBool,
    #[token("str")]    TStr,
    #[token("void")]   TVoid,

    // Identificadores
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // Operadores
    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,
    #[token("%")] Percent,
    #[token("=")] Eq,
    #[token("==")] EqEq,
    #[token("!=")] Neq,
    #[token("<")]  Lt,
    #[token(">")]  Gt,
    #[token("<=")] Lte,
    #[token(">=")] Gte,
    #[token("&&")] And,
    #[token("||")] Or,
    #[token("!")  ] Bang,
    #[token("->")] Arrow,
    #[token("::")] ColonColon,

    // Pontuação
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token("[")] LBracket,
    #[token("]")] RBracket,
    #[token(":")] Colon,
    #[token(";")] Semi,
    #[token(",")] Comma,
    #[token(".")] Dot,

    #[regex(r"//[^\n]*", logos::skip)]  // comentários
    #[regex(r"[ \t\n\r]+", logos::skip)] // whitespace
    Error,
}
```

**Entregável:** Suite de testes unitários cobrindo todos os tokens. `cargo test -p vex-lexer` verde.

---

## Fase 2 — AST + Parser (Dias 4–7)

### Objetivo
Parser descent recursivo que produz uma AST tipada.

### Nós da AST

```rust
// crates/vex-parser/src/ast.rs

pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        mutable: bool,
        type_ann: Option<Type>,
        value: Expr,
    },
    Fn {
        name: String,
        params: Vec<Param>,
        ret_type: Type,
        body: Vec<Stmt>,
        is_pub: bool,
    },
    Return(Option<Expr>),
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    For {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
    },
    Struct {
        name: String,
        fields: Vec<(String, Type)>,
        is_pub: bool,
    },
    Impl {
        target: String,
        methods: Vec<Stmt>,
    },
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Ident(String),
    BinOp { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    UnaryOp { op: UnaryOp, val: Box<Expr> },
    Call { name: Box<Expr>, args: Vec<Expr> },
    FieldAccess { obj: Box<Expr>, field: String },
    Index { obj: Box<Expr>, idx: Box<Expr> },
    Array(Vec<Expr>),
    Struct { name: String, fields: Vec<(String, Expr)> },
    Match { val: Box<Expr>, arms: Vec<MatchArm> },
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone)]
pub enum Type {
    Int, Float, Bool, Str, Void,
    Named(String),
    Array(Box<Type>),
    Ref(Box<Type>),
    MutRef(Box<Type>),
    Fn(Vec<Type>, Box<Type>),
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
pub enum BinOp { Add, Sub, Mul, Div, Mod, Eq, Neq, Lt, Gt, Lte, Gte, And, Or }

#[derive(Debug, Clone)]
pub enum UnaryOp { Neg, Not }
```

### Exemplo de sintaxe Vex válida após essa fase

```vex
fn fib(n: int) -> int {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

fn main() -> void {
    let result: int = fib(10)
    print(result)
}
```

**Entregável:** Parser produz AST correta para os exemplos em `examples/`. `cargo test -p vex-parser` verde.

---

## Fase 3 — Type Checker (Dias 8–12)

### Objetivo
Validar tipos em compile-time. Erros claros com span.

### O que checar

```
1. Variáveis declaradas antes de usar
2. Tipos compatíveis em operações (int + int ✓, int + str ✗)
3. Retorno de funções bate com declaração
4. Chamadas com argumentos corretos (quantidade e tipo)
5. Campos de struct existem
6. Inferência de tipo em `let x = 42` → int
```

### Estrutura

```rust
// crates/vex-typeck/src/lib.rs

pub struct TypeEnv {
    scopes: Vec<HashMap<String, Type>>,
    functions: HashMap<String, FnSig>,
    structs: HashMap<String, Vec<(String, Type)>>,
}

impl TypeEnv {
    pub fn check_program(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> { ... }
    pub fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), TypeError> { ... }
    pub fn infer_expr(&mut self, expr: &Expr) -> Result<Type, TypeError> { ... }
}

#[derive(Debug)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
}
```

### Exemplo de erro esperado

```
erro[E001]: tipos incompatíveis
  --> main.vex:3:14
  |
3 |   let x: int = "hello"
  |                ^^^^^^^ esperado `int`, encontrado `str`
  |
  dica: remova a anotação de tipo ou converta o valor
```

**Entregável:** Type checker rejeita programas inválidos com mensagens úteis. `cargo test -p vex-typeck` verde.

---

## Fase 4 — Code Generation via LLVM (Dias 13–20)

### Objetivo
Compilar AST tipada para binário nativo via LLVM IR usando `inkwell`.

### Estrutura

```rust
// crates/vex-codegen/src/lib.rs
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::*;

pub struct Codegen<'ctx> {
    ctx: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    env: HashMap<String, PointerValue<'ctx>>,
    fns: HashMap<String, FunctionValue<'ctx>>,
}

impl<'ctx> Codegen<'ctx> {
    pub fn compile_program(&mut self, stmts: &[Stmt]) -> Result<(), CodegenError> { ... }
    pub fn compile_fn(&mut self, stmt: &Stmt) -> Result<FunctionValue<'ctx>, CodegenError> { ... }
    pub fn compile_expr(&mut self, expr: &Expr) -> Result<BasicValueEnum<'ctx>, CodegenError> { ... }
    pub fn emit_object(&self, path: &str) -> Result<(), CodegenError> { ... }
}
```

### Pipeline de compilação

```
arquivo.vex
    ↓ lex()
Vec<Token>
    ↓ parse()
Vec<Stmt> (AST)
    ↓ typecheck()
Vec<Stmt> (AST verificada)
    ↓ codegen()
LLVM IR (.ll)
    ↓ llc / lld
binário nativo
```

### Mapeamento de tipos Vex → LLVM

| Vex    | LLVM IR       |
|--------|---------------|
| int    | i64           |
| float  | double        |
| bool   | i1            |
| str    | i8* (ptr)     |
| void   | void          |
| struct | { field... }  |
| array  | [N x T]*      |

**Entregável:** `vex compile hello.vex` gera `hello` executável. Programa Hello World roda.

---

## Fase 5 — CLI (`vex` binary) (Dias 21–23)

### Objetivo
Ferramenta de linha de comando completa.

### Comandos

```bash
vex run   hello.vex          # compila e executa
vex build hello.vex          # só compila → ./hello
vex check hello.vex          # só type-check, sem compilar
vex fmt   hello.vex          # formata o código (futuro)
vex repl                     # REPL interativo
vex new   meu-projeto        # scaffold de projeto
```

### Estrutura do CLI

```rust
// crates/vex-cli/src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vex", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run { file: PathBuf },
    Build { file: PathBuf, #[arg(short, long)] output: Option<PathBuf> },
    Check { file: PathBuf },
    Repl,
    New { name: String },
}
```

**Entregável:** `vex run examples/fib.vex` funciona end-to-end.

---

## Fase 6 — Stdlib mínima (Dias 24–28)

### Funções built-in prioritárias

```vex
// I/O
print(val: any) -> void
println(val: any) -> void
input(prompt: str) -> str
read_file(path: str) -> str
write_file(path: str, content: str) -> void

// Conversões
to_int(val: str) -> int
to_float(val: str) -> float
to_str(val: any) -> str

// Arrays
len(arr: [T]) -> int
push(arr: mut [T], val: T) -> void
pop(arr: mut [T]) -> T
map(arr: [T], f: fn(T) -> U) -> [U]
filter(arr: [T], f: fn(T) -> bool) -> [T]

// Strings
split(s: str, sep: str) -> [str]
join(arr: [str], sep: str) -> str
trim(s: str) -> str
contains(s: str, sub: str) -> bool
```

**Entregável:** Exemplos usando stdlib compilam e rodam.

---

## Fase 7 — Testes de integração (Dias 29–32)

### Programas de benchmark

```
examples/
├── hello.vex         # hello world
├── fib.vex           # fibonacci recursivo
├── fizzbuzz.vex      # fizzbuzz clássico
├── bubble_sort.vex   # ordenação
├── linked_list.vex   # structs com ponteiros
├── http_server.vex   # (futuro — com stdlib net)
└── nn.vex            # (futuro — IA simples)
```

### Benchmark contra Rust/Python

```bash
# medir tempo de compilação e execução
hyperfine 'vex run fib.vex' 'python fib.py' 'rustc fib.rs && ./fib'
```

---

## Fase 8 — Features avançadas (Futuro)

### Ownership simplificado (não Rust verboso)

```vex
fn process(data: &[int]) -> int {  // borrow imutável, sem lifetime explícito
    return data[0]
}

fn modify(data: &mut [int]) -> void {  // borrow mutável
    data[0] = 99
}
```

### Pattern matching

```vex
match x {
    0     => print("zero"),
    1..10 => print("pequeno"),
    _     => print("grande"),
}
```

### Generics

```vex
fn max<T>(a: T, b: T) -> T {
    if a > b { return a }
    return b
}
```

### WASM target

```bash
vex build app.vex --target wasm32   # porta pra browser
```

### LSP (editor support)

```
vex-lsp   # language server protocol
          # autocomplete, go-to-def, hover types, erros inline
```

---

## Dependências de sistema necessárias

```bash
# Ubuntu/Debian
apt install llvm-17 llvm-17-dev clang-17 lld-17

# Fedora (seu caso)
dnf install llvm17 llvm17-devel clang lld

# Variável de ambiente
export LLVM_SYS_170_PREFIX=/usr/lib/llvm-17
```

---

## Ordem de execução para Claude Code

```
1. scaffold do workspace Cargo (Fase 0)
2. vex-lexer com logos + testes (Fase 1)
3. vex-parser AST completa + testes (Fase 2)
4. vex-typeck com inferência + erros bonitos (Fase 3)
5. vex-codegen inkwell básico — int/float/fn/call (Fase 4)
6. vex-cli com run/build/check (Fase 5)
7. stdlib mínima embutida (Fase 6)
8. testes de integração end-to-end (Fase 7)
```

> **Nota para Claude Code:** implemente cada fase em ordem. Não avance para a próxima antes de ter testes passando na atual. Use `cargo test --workspace` antes de cada commit.

---

## Exemplo de programa Vex alvo (deve rodar ao fim da Fase 6)

```vex
struct Ponto {
    x: float,
    y: float,
}

impl Ponto {
    fn distancia(self, outro: Ponto) -> float {
        let dx: float = self.x - outro.x
        let dy: float = self.y - outro.y
        return sqrt(dx * dx + dy * dy)
    }
}

fn main() -> void {
    let a: Ponto = Ponto { x: 0.0, y: 0.0 }
    let b: Ponto = Ponto { x: 3.0, y: 4.0 }
    let d: float = a.distancia(b)
    println(d)   // 5.0
}
```

---

*Vex Lang — construída por Jeff Almeida. v0.1 em desenvolvimento.*