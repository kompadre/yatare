//! Recursive-descent parser with precedence climbing for the JS subset.
//!
//! Precedence (loose -> tight): `||` `&&` `^` `==` `!=` `<` `<=` `>` `>=`
//! `+` `-` `*` — C/JS-faithful (notably `==` binds tighter than `^`) except
//! that `|`, `&`, and shifts are absent (the ISA has XOR only).

use super::lexer::{Tok, Token};
use super::{AssignOp, BinOp, Expr, FuncDef, Item, JsError, LogOp, Stmt, err};

pub fn parse(tokens: &[Token]) -> Result<Vec<Item>, JsError> {
    let mut p = P {
        toks: tokens,
        pos: 0,
    };
    let mut items = Vec::new();
    while !p.at_eof() {
        if p.peek_kw("function") {
            items.push(Item::Fn(p.func()?));
        } else {
            items.push(Item::Stmt(p.stmt()?));
        }
    }
    Ok(items)
}

enum Op {
    Bin(BinOp),
    Log(LogOp),
}

struct P<'a> {
    toks: &'a [Token],
    pos: usize,
}

impl P<'_> {
    fn line(&self) -> usize {
        self.toks[self.pos].line
    }
    fn tok(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn at_eof(&self) -> bool {
        matches!(self.tok(), Tok::Eof)
    }
    fn peek_p(&self, p: &str) -> bool {
        matches!(self.tok(), Tok::P(x) if *x == p)
    }
    fn peek_kw(&self, k: &str) -> bool {
        matches!(self.tok(), Tok::Kw(x) if *x == k)
    }
    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if !matches!(t, Tok::Eof) {
            self.pos += 1;
        }
        t
    }
    fn expected(&self, what: &str) -> JsError {
        JsError {
            line: self.line(),
            msg: format!("expected {what}, found {}", self.tok().describe()),
        }
    }
    fn expect_p(&mut self, p: &str) -> Result<(), JsError> {
        if self.peek_p(p) {
            self.bump();
            Ok(())
        } else {
            Err(self.expected(&format!("`{p}`")))
        }
    }
    fn expect_ident(&mut self, what: &str) -> Result<(String, usize), JsError> {
        let line = self.line();
        match self.tok().clone() {
            Tok::Ident(s) => {
                self.bump();
                Ok((s, line))
            }
            t => Err(JsError {
                line,
                msg: format!("expected {what}, found {}", t.describe()),
            }),
        }
    }

    // --- statements -------------------------------------------------------

    fn block_body(&mut self) -> Result<Vec<Stmt>, JsError> {
        let line = self.line();
        self.expect_p("{")?;
        let mut stmts = Vec::new();
        while !self.peek_p("}") {
            if self.at_eof() {
                return err(line, "unterminated block");
            }
            stmts.push(self.stmt()?);
        }
        self.bump();
        Ok(stmts)
    }

    fn func(&mut self) -> Result<FuncDef, JsError> {
        let line = self.line();
        self.bump(); // `function`
        let (name, _) = self.expect_ident("function name")?;
        self.expect_p("(")?;
        let mut params = Vec::new();
        if !self.peek_p(")") {
            loop {
                let (p, _) = self.expect_ident("parameter name")?;
                if params.contains(&p) {
                    return err(line, format!("duplicate parameter `{p}`"));
                }
                params.push(p);
                if self.peek_p(",") {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect_p(")")?;
        let body = self.block_body()?;
        Ok(FuncDef {
            line,
            name,
            params,
            body,
        })
    }

    fn stmt(&mut self) -> Result<Stmt, JsError> {
        let line = self.line();
        if self.peek_kw("var") || self.peek_kw("let") || self.peek_kw("const") {
            self.bump();
            let (name, _) = self.expect_ident("variable name")?;
            self.expect_p("=")?;
            let expr = self.expr(1)?;
            self.expect_p(";")?;
            return Ok(Stmt::Var { line, name, expr });
        }
        if self.peek_kw("if") {
            self.bump();
            self.expect_p("(")?;
            let cond = self.expr(1)?;
            self.expect_p(")")?;
            let then = Box::new(self.stmt()?);
            let els = if self.peek_kw("else") {
                self.bump();
                Some(Box::new(self.stmt()?))
            } else {
                None
            };
            return Ok(Stmt::If {
                line,
                cond,
                then,
                els,
            });
        }
        if self.peek_kw("while") {
            self.bump();
            self.expect_p("(")?;
            let cond = self.expr(1)?;
            self.expect_p(")")?;
            let body = Box::new(self.stmt()?);
            return Ok(Stmt::While { line, cond, body });
        }
        if self.peek_kw("return") {
            self.bump();
            let expr = if self.peek_p(";") {
                None
            } else {
                Some(self.expr(1)?)
            };
            self.expect_p(";")?;
            return Ok(Stmt::Return { line, expr });
        }
        if self.peek_p("{") {
            let stmts = self.block_body()?;
            return Ok(Stmt::Block { line, stmts });
        }

        let e = self.expr(1)?;
        if self.peek_p("=") || self.peek_p("+=") || self.peek_p("-=") {
            let op = match self.bump() {
                Tok::P("=") => AssignOp::Set,
                Tok::P("+=") => AssignOp::Add,
                _ => AssignOp::Sub,
            };
            let name = match e {
                Expr::Var(n) => n,
                _ => {
                    return err(
                        line,
                        "invalid assignment target (only variables; element properties are set via .css())",
                    );
                }
            };
            let expr = self.expr(1)?;
            self.expect_p(";")?;
            Ok(Stmt::Assign {
                line,
                name,
                op,
                expr,
            })
        } else {
            self.expect_p(";")?;
            Ok(Stmt::Expr { line, expr: e })
        }
    }

    // --- expressions ------------------------------------------------------

    fn peek_binop(&self) -> Option<(u8, Op)> {
        match self.tok() {
            Tok::P(p) => match *p {
                // C-faithful: == binds tighter than ^ (the classic
                // `x & MASK == 0` trap) -> `a ^ b == c` == `a ^ (b == c)`.
                "||" => Some((1, Op::Log(LogOp::Or))),
                "&&" => Some((2, Op::Log(LogOp::And))),
                "^" => Some((3, Op::Bin(BinOp::Xor))),
                "==" => Some((4, Op::Bin(BinOp::Eq))),
                "!=" => Some((4, Op::Bin(BinOp::Ne))),
                "<" => Some((5, Op::Bin(BinOp::Lt))),
                "<=" => Some((5, Op::Bin(BinOp::Le))),
                ">" => Some((5, Op::Bin(BinOp::Gt))),
                ">=" => Some((5, Op::Bin(BinOp::Ge))),
                "+" => Some((6, Op::Bin(BinOp::Add))),
                "-" => Some((6, Op::Bin(BinOp::Sub))),
                "*" => Some((7, Op::Bin(BinOp::Mul))),
                _ => None,
            },
            _ => None,
        }
    }

    fn expr(&mut self, min: u8) -> Result<Expr, JsError> {
        let mut lhs = self.unary()?;
        while let Some((prec, op)) = self.peek_binop() {
            if prec < min {
                break;
            }
            self.bump();
            let rhs = self.expr(prec + 1)?;
            lhs = match op {
                Op::Bin(b) => Expr::Binary {
                    op: b,
                    l: Box::new(lhs),
                    r: Box::new(rhs),
                },
                Op::Log(l) => Expr::Logical {
                    op: l,
                    l: Box::new(lhs),
                    r: Box::new(rhs),
                },
            };
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, JsError> {
        if self.peek_p("-") {
            self.bump();
            let inner = self.unary()?;
            return Ok(match inner {
                Expr::Int(v) => Expr::Int(v.wrapping_neg()),
                other => Expr::Neg(Box::new(other)),
            });
        }
        if self.peek_p("!") {
            self.bump();
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.peek_p("~") {
            self.bump();
            return Ok(Expr::BitNot(Box::new(self.unary()?)));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, JsError> {
        let mut e = self.primary()?;
        loop {
            if self.peek_p(".") {
                self.bump();
                let (name, _) = self.expect_ident("property name")?;
                if self.peek_p("(") {
                    self.bump();
                    let args = self.call_args()?;
                    e = Expr::Method {
                        base: Box::new(e),
                        name,
                        args,
                    };
                } else {
                    e = Expr::Member {
                        base: Box::new(e),
                        prop: name,
                    };
                }
                continue;
            }
            if self.peek_p("(") {
                // call on a bare identifier (functions) — anything else is v0-illegal
                let Expr::Var(fname) = e else {
                    return err(
                        self.line(),
                        "only named functions and jQuery methods can be called",
                    );
                };
                self.bump();
                let args = self.call_args()?;
                e = Expr::Call { name: fname, args };
                continue;
            }
            break;
        }
        Ok(e)
    }

    fn call_args(&mut self) -> Result<Vec<Expr>, JsError> {
        let mut args = Vec::new();
        if !self.peek_p(")") {
            loop {
                args.push(self.expr(1)?);
                if self.peek_p(",") {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect_p(")")?;
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expr, JsError> {
        let line = self.line();
        match self.tok().clone() {
            Tok::Int(v) => {
                self.bump();
                Ok(Expr::Int(v))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Str(s))
            }
            Tok::Kw("true") => {
                self.bump();
                Ok(Expr::Bool(true))
            }
            Tok::Kw("false") => {
                self.bump();
                Ok(Expr::Bool(false))
            }
            Tok::Kw(k) => Err(JsError {
                line,
                msg: format!("expected expression, found keyword `{k}`"),
            }),
            Tok::Ident(s) => {
                self.bump();
                Ok(Expr::Var(s))
            }
            Tok::P("(") => {
                self.bump();
                let e = self.expr(1)?;
                self.expect_p(")")?;
                Ok(e)
            }
            Tok::P("[") => err(line, "array literals are unsupported"),
            Tok::P(p) => Err(JsError {
                line,
                msg: format!("expected expression, found `{p}`"),
            }),
            Tok::Eof => err(line, "unexpected end of file"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::lexer::lex;

    fn items(src: &str) -> Vec<Item> {
        parse(&lex(src).unwrap()).unwrap()
    }

    fn expr_of(src: &str) -> Expr {
        match items(src).swap_remove(0) {
            Item::Stmt(Stmt::Expr { expr, .. }) => expr,
            Item::Stmt(Stmt::Return { expr: Some(e), .. }) => e,
            other => panic!("expected expr stmt, got {other:?}"),
        }
    }

    #[test]
    fn precedence_is_c_faithful() {
        // * binds tighter than +, which binds tighter than comparisons
        match expr_of("return 1 + 2 * 3;") {
            Expr::Binary {
                op: BinOp::Add, r, ..
            } => assert!(matches!(*r, Expr::Binary { op: BinOp::Mul, .. })),
            other => panic!("{other:?}"),
        }
        // comparisons bind tighter than ^ (C/JS rule)
        match expr_of("return a ^ b == c;") {
            Expr::Binary {
                op: BinOp::Xor, r, ..
            } => assert!(matches!(*r, Expr::Binary { op: BinOp::Eq, .. })),
            other => panic!("{other:?}"),
        }
        // && binds looser than comparisons
        match expr_of("return a < b && c;") {
            Expr::Logical {
                op: LogOp::And, l, ..
            } => assert!(matches!(*l, Expr::Binary { op: BinOp::Lt, .. })),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unary_and_postfix() {
        assert!(matches!(expr_of("return -5;"), Expr::Int(-5)));
        assert!(matches!(expr_of("return -x;"), Expr::Neg(_)));
        assert!(matches!(expr_of("return !x;"), Expr::Not(_)));
        let e = expr_of(r##"return $("#a").css("left", 1).width();"##);
        assert!(matches!(e, Expr::Method { name, .. } if name == "width"));
    }

    #[test]
    fn statements_and_functions() {
        let its =
            items("function f(a, b) { if (a > b) { return a; } return b; }\nvar x = f(1, 2);\n");
        assert!(matches!(its[0], Item::Fn(_)));
        assert!(matches!(its[1], Item::Stmt(Stmt::Var { .. })));
    }

    #[test]
    fn parse_errors_have_lines_and_messages() {
        for (src, want) in [
            ("var x = ;", "expected expression"),
            ("if (x) {", "unterminated block"),
            ("return (1;", "expected `)`"),
            ("1 +;", "expected expression"),
            ("a.1();", "expected property name"),
        ] {
            let e = parse(&lex(src).unwrap()).expect_err(src);
            assert!(
                e.msg.contains(want),
                "src={src:?}: got {:?}, want {want:?}",
                e.msg
            );
        }
    }
}
