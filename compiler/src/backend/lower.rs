//! 受检 AST → 首片 typed IR。
//!
//! subset 闸已先行；这里仍以 `Result` 返回，避免闸与降级判定漂移时 panic。

use super::ir::{Func, Local, LocalId, Program, Stmt, StrArg, Ty};
use super::subset::{self, SubsetError};
use crate::parser::{Ex, Lit, Prog, St};
use std::collections::HashMap;

pub fn lower(prog: &Prog) -> Result<Program, SubsetError> {
    subset::check(prog)?;
    let mut funcs = Vec::new();
    for f in &prog.fns {
        if f.name != "main" {
            continue;
        }
        let mut locals: Vec<Local> = Vec::new();
        let mut body: Vec<Stmt> = Vec::new();
        let mut scope: HashMap<String, LocalId> = HashMap::new();
        for st in &f.body.stmts {
            match st {
                St::Let(name, None, Ex::Lit(Lit::Str(s), _, _), l, c) => {
                    let bytes = s.as_bytes().to_vec();
                    let id = LocalId(locals.len() as u32);
                    locals.push(Local {
                        id,
                        name: name.clone(),
                        ty: Ty::Bytes(bytes.len() as u64),
                    });
                    body.push(Stmt::BindStr {
                        local: id,
                        bytes,
                        line: *l,
                        col: *c,
                    });
                    scope.insert(name.clone(), id);
                }
                St::Expr(Some(Ex::Call(callee, args, l, c)), _, _) => {
                    let is_println =
                        matches!(callee.as_ref(), Ex::Path(n, _, _) if n == "println");
                    if !is_println || args.len() != 1 {
                        return Err(SubsetError::new(*l, *c, "lower：println 形态"));
                    }
                    let arg = match &args[0] {
                        Ex::Path(n, l2, c2) => match scope.get(n) {
                            Some(id) => StrArg::Local(*id),
                            None => {
                                return Err(SubsetError::new(
                                    *l2,
                                    *c2,
                                    format!("lower：'{}' 未绑定字符串", n),
                                ))
                            }
                        },
                        Ex::Lit(Lit::Str(s), _, _) => StrArg::Bytes(s.as_bytes().to_vec()),
                        other => {
                            let (l2, c2) = subset::expr_span(other);
                            return Err(SubsetError::new(l2, c2, "lower：println 实参"));
                        }
                    };
                    body.push(Stmt::Println {
                        arg,
                        line: *l,
                        col: *c,
                    });
                }
                _ => return Err(SubsetError::new(0, 0, "lower：未预期语句")),
            }
        }
        funcs.push(Func {
            name: "main".to_string(),
            line: f.span.0.max(1),
            locals,
            body,
        });
    }
    Ok(Program { funcs })
}

#[cfg(test)]
mod tests {
    use super::lower;
    use crate::backend::ir::{Stmt, StrArg, Ty};
    use crate::compile;

    fn ir(src: &str) -> crate::backend::ir::Program {
        let (prog, diags) = compile(src);
        assert!(diags.is_empty(), "{diags:?}");
        lower(&prog).expect("lower")
    }

    #[test]
    fn hello_lowers_to_one_local_and_two_stmts() {
        let p = ir("fn main() {\n    let greeting = \"Hello, ZOR\";\n    println(greeting);\n}\n");
        assert_eq!(p.funcs.len(), 1);
        let f = &p.funcs[0];
        assert_eq!(f.name, "main");
        assert_eq!(f.line, 1);
        assert_eq!(f.locals.len(), 1);
        assert_eq!(f.locals[0].ty, Ty::Bytes(10));
        assert_eq!(f.body.len(), 2);
        match &f.body[0] {
            Stmt::BindStr { local, bytes, line, .. } => {
                assert_eq!(local.0, 0);
                assert_eq!(bytes.as_slice(), b"Hello, ZOR");
                assert_eq!(*line, 2);
            }
            other => panic!("expected BindStr, got {other:?}"),
        }
        match &f.body[1] {
            Stmt::Println { arg: StrArg::Local(id), line, .. } => {
                assert_eq!(id.0, 0);
                assert_eq!(*line, 3);
            }
            other => panic!("expected Println(Local), got {other:?}"),
        }
    }

    #[test]
    fn literal_println_lowers_to_bytes() {
        let p = ir("fn main() { println(\"hi\"); }");
        match &p.funcs[0].body[0] {
            Stmt::Println { arg: StrArg::Bytes(b), .. } => assert_eq!(b.as_slice(), b"hi"),
            other => panic!("expected Println(Bytes), got {other:?}"),
        }
    }

    #[test]
    fn shadowing_allocates_new_local() {
        let p = ir("fn main() { let s = \"a\"; let s = \"bb\"; println(s); }");
        let f = &p.funcs[0];
        assert_eq!(f.locals.len(), 2);
        match &f.body[2] {
            Stmt::Println { arg: StrArg::Local(id), .. } => assert_eq!(id.0, 1),
            other => panic!("expected Println(Local), got {other:?}"),
        }
    }
}
