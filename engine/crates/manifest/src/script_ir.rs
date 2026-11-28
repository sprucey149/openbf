//! Closed, deterministic action-tree IR for translated BF2 mission scripts
//! (both Rise of the Empire campaign missions and offline-converted content
//! mod scripts).
//!
//! `bake` / `mungefs::lua50::ir` (and `scripts/campaign_ir.py` for stock
//! campaign missions) translate Lua 5.0 bytecode offline into [`ScriptDef`].
//! `game::campaign_script` evaluates it natively with arena-allocated tables,
//! insertion-order traversal, and `f32` numbers so simulation state remains
//! deterministic and deep-cloneable.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScriptDef {
    pub main: u32,
    pub fns: Vec<FnDef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FnDef {
    pub name: String,
    pub params: u16,
    #[serde(default)]
    pub vararg: bool,
    pub locals: u16,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Lt,
    Le,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IterKind {
    Ipairs,
    Pairs,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Nil,
    Bool(bool),
    Num(f32),
    Str(String),
    Local(u16),
    Upval(u16),
    Global(String),
    Index(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Method(Box<Expr>, String, Vec<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Closure(u32, Vec<Expr>),
    Table(Vec<(Expr, Expr)>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Do(Expr),
    SetLocal(u16, Expr),
    SetLocals(Vec<u16>, Expr),
    SetGlobal(String, Expr),
    SetUpval(u16, Expr),
    SetIndex(Expr, Expr, Expr),
    Return(Vec<Expr>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    ForNum(u16, Expr, Expr, Expr, Vec<Stmt>),
    ForIn(IterKind, Expr, Vec<u16>, Vec<Stmt>),
    Close(u16),
    Loop(Vec<Stmt>),
    Break,
    Continue,
}

impl ScriptDef {
    /// Constant script names passed to `ScriptCB_DoFile("...")` anywhere in
    /// this script, in order of appearance (deduplicated case-insensitively).
    pub fn do_files(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.fns {
            collect_do_files_stmts(&f.body, &mut out);
        }
        out
    }
}

fn push_unique_ci(out: &mut Vec<String>, name: &str) {
    if !name.is_empty() && !out.iter().any(|s| s.eq_ignore_ascii_case(name)) {
        out.push(name.to_owned());
    }
}

fn collect_do_files_stmts(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Do(e)
            | Stmt::SetLocal(_, e)
            | Stmt::SetLocals(_, e)
            | Stmt::SetGlobal(_, e)
            | Stmt::SetUpval(_, e) => collect_do_files_expr(e, out),
            Stmt::SetIndex(t, k, v) => {
                collect_do_files_expr(t, out);
                collect_do_files_expr(k, out);
                collect_do_files_expr(v, out);
            }
            Stmt::Return(es) => {
                for e in es {
                    collect_do_files_expr(e, out);
                }
            }
            Stmt::If(c, a, b) => {
                collect_do_files_expr(c, out);
                collect_do_files_stmts(a, out);
                collect_do_files_stmts(b, out);
            }
            Stmt::ForNum(_, a, b, c, body) => {
                collect_do_files_expr(a, out);
                collect_do_files_expr(b, out);
                collect_do_files_expr(c, out);
                collect_do_files_stmts(body, out);
            }
            Stmt::ForIn(_, coll, _, body) => {
                collect_do_files_expr(coll, out);
                collect_do_files_stmts(body, out);
            }
            Stmt::Loop(body) => collect_do_files_stmts(body, out),
            Stmt::Close(_) | Stmt::Break | Stmt::Continue => {}
        }
    }
}

fn collect_do_files_expr(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Call(f, args) => {
            if matches!(f.as_ref(), Expr::Global(g) if g == "ScriptCB_DoFile") {
                if let Some(Expr::Str(name)) = args.first() {
                    push_unique_ci(out, name);
                }
            }
            collect_do_files_expr(f, out);
            for a in args {
                collect_do_files_expr(a, out);
            }
        }
        Expr::Method(o, _, args) => {
            collect_do_files_expr(o, out);
            for a in args {
                collect_do_files_expr(a, out);
            }
        }
        Expr::Index(t, k) | Expr::Bin(_, t, k) => {
            collect_do_files_expr(t, out);
            collect_do_files_expr(k, out);
        }
        Expr::Not(x) | Expr::Neg(x) => collect_do_files_expr(x, out),
        Expr::Closure(_, caps) => {
            for c in caps {
                collect_do_files_expr(c, out);
            }
        }
        Expr::Table(pairs) => {
            for (k, v) in pairs {
                collect_do_files_expr(k, out);
                collect_do_files_expr(v, out);
            }
        }
        Expr::Nil
        | Expr::Bool(_)
        | Expr::Num(_)
        | Expr::Str(_)
        | Expr::Local(_)
        | Expr::Upval(_)
        | Expr::Global(_) => {}
    }
}
