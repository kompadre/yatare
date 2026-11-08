//! JS-subset front end: source text -> yatare bytecode assembly.
//!
//! The subset exists to be *compilable into a 14-op i32 stack VM with zero
//! heap and no `CALL` instruction*, so the constraints are structural:
//!
//! - **Numbers only:** wrapping i32 arithmetic; no floats, no division.
//! - **Bitwise:** `^` only (the ISA has XOR and no AND/OR/NOT/shifts).
//! - **No runtime calls:** `function` definitions are *inlined at compile
//!   time*; recursion (direct or mutual) is a compile error.
//! - **No strings at runtime:** selectors and `.css()` property names are
//!   compile-time literals; string *values* are rejected. Element state lives
//!   in local slots exposed through a [`ManifestEntry`] map for the harness.
//! - **Locals:** one flat scope per function, ≤16 i32 slots shared between
//!   user variables and element properties (overflow is a compile error).
//! - **jqLite facade:** `$("#sel")` resolves to a compile-time element handle;
//!   supported members/methods: `.offset()`, `.width()`, `.height()`,
//!   `.left`, `.top`, `.css("left"|"top"|"width"|"height", expr)`. Handles
//!   are chainable; using one where a number is required is a compile error.
//! - **Known deviations from JS:** `&&`/`||` return boolean `1`/`0` (JS
//!   returns the deciding operand); `let`/`const` behave like `var`;
//!   assignment is a statement, not an expression; no ternary, arrays,
//!   objects, closures, `this`, or prototypes.

mod codegen;
mod lexer;
mod parse;

use std::fmt;

use lexer::{Token, lex};

pub use codegen::generate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for JsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for JsError {}

pub(crate) fn err<T>(line: usize, msg: impl Into<String>) -> Result<T, JsError> {
    Err(JsError {
        line,
        msg: msg.into(),
    })
}

/// One element property mapped to a local slot: the harness I/O contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub selector: String,
    pub prop: String,
    pub slot: u8,
}

impl fmt::Display for ManifestEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" {} -> local[{}]",
            self.selector, self.prop, self.slot
        )
    }
}

#[derive(Debug, Clone)]
pub struct Compiled {
    /// Full assembly text, ready to write to a file (validated: it parses
    /// back and passes stack typing before `compile` returns).
    pub asm: String,
    pub manifest: Vec<ManifestEntry>,
}

/// Lex, parse, lower, and self-validate a JS-subset program.
pub fn compile(src: &str) -> Result<Compiled, JsError> {
    let tokens: Vec<Token> = lex(src)?;
    let items = parse::parse(&tokens)?;
    // Lowering self-validates: emitted asm must parse back through the real
    // asm parser and pass the optimizer's stack typing before returning.
    generate(items, src)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Item {
    Fn(FuncDef),
    Stmt(Stmt),
}

#[derive(Debug, Clone)]
pub struct FuncDef {
    pub line: usize,
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Var {
        line: usize,
        name: String,
        expr: Expr,
    },
    Assign {
        line: usize,
        name: String,
        op: AssignOp,
        expr: Expr,
    },
    If {
        line: usize,
        cond: Expr,
        then: Box<Stmt>,
        els: Option<Box<Stmt>>,
    },
    While {
        line: usize,
        cond: Expr,
        body: Box<Stmt>,
    },
    Return {
        line: usize,
        expr: Option<Expr>,
    },
    Expr {
        line: usize,
        expr: Expr,
    },
    Block {
        line: usize,
        stmts: Vec<Stmt>,
    },
}

impl Stmt {
    pub fn line(&self) -> usize {
        match self {
            Stmt::Var { line, .. }
            | Stmt::Assign { line, .. }
            | Stmt::If { line, .. }
            | Stmt::While { line, .. }
            | Stmt::Return { line, .. }
            | Stmt::Expr { line, .. }
            | Stmt::Block { line, .. } => *line,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Xor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i32),
    Bool(bool),
    Var(String),
    Str(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    BitNot(Box<Expr>),
    Binary {
        op: BinOp,
        l: Box<Expr>,
        r: Box<Expr>,
    },
    Logical {
        op: LogOp,
        l: Box<Expr>,
        r: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Member {
        base: Box<Expr>,
        prop: String,
    },
    Method {
        base: Box<Expr>,
        name: String,
        args: Vec<Expr>,
    },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_valid_program_and_validates_itself() {
        let c = compile("var x = 1 + 2 * 3; return x;").expect("compile");
        assert!(c.asm.contains("push"));
        assert!(c.manifest.is_empty());
    }

    #[test]
    fn manifest_describes_element_slots() {
        let c = compile(
            r##"
            var left = $("#banner").offset().left;
            $("#banner").css("left", left + 1);
            return left;
        "##,
        )
        .expect("compile");
        let entry = c
            .manifest
            .iter()
            .find(|e| e.selector == "#banner" && e.prop == "left")
            .expect("manifest entry");
        assert_eq!(
            entry.to_string(),
            format!("\"#banner\" left -> local[{}]", entry.slot)
        );
    }

    #[test]
    fn errors_carry_line_numbers() {
        for (src, want) in [
            ("var x = 1;\nx = y;", "undeclared"),
            ("var x = 1;\nvar x = 2;", "already declared"),
            ("return 1 / 2;", "division"),
            ("return 1.5;", "floating point"),
            ("return \"hi\";", "string values"),
            ("return f() + 1;", "unknown function"),
            ("return 3 ? 1 : 2;", "unexpected character"),
        ] {
            let e = compile(src).expect_err(src);
            assert!(
                e.msg.contains(want),
                "src={src:?}: got {:?}, want substring {want:?}",
                e.msg
            );
        }
        assert_eq!(compile("var x = 1;\nx = y;").unwrap_err().line, 2);
    }

    #[test]
    fn recursion_is_rejected() {
        let e = compile("function f(n) { return f(n); }\nreturn f(1);").unwrap_err();
        assert!(e.msg.contains("recurs"), "{e}");
        // mutual recursion too
        let e =
            compile("function f(n) { return g(n); }\nfunction g(n) { return f(n); }\nreturn f(1);")
                .unwrap_err();
        assert!(e.msg.contains("recurs"), "{e}");
    }

    #[test]
    fn local_overflow_is_rejected() {
        let mut src = String::new();
        for i in 0..17 {
            src.push_str(&format!("var v{i} = {i};\n"));
        }
        src.push_str("return v0;\n");
        let e = compile(&src).unwrap_err();
        assert!(e.msg.contains("local"), "{e}");
    }

    #[test]
    fn handle_where_number_required_is_rejected() {
        let e = compile(r##"return $("#a") + 1;"##).unwrap_err();
        assert!(e.msg.contains("number"), "{e}");
        // handle vars are legal — but only until a number is required
        let e = compile(r##"var x = $("#a"); return x + 1;"##).unwrap_err();
        assert!(e.msg.contains("number") || e.msg.contains("jQuery"), "{e}");
    }
}
