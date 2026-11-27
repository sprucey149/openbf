//! Offline translator from Lua 5.0 bytecode ([`Proto`]) to openbf's closed,
//! deterministic action-tree IR ([`manifest::script_ir::ScriptDef`]).
//!
//! Ported from `scripts/campaign_ir.py` and extended with `while`/`repeat`
//! loops (`Stmt::Loop` / `Stmt::Continue`) and `break` (`Stmt::Break`) so
//! community mod mission scripts translate offline during `bake mod-convert`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use manifest::script_ir::{BinOp, Expr, FnDef, IterKind, ScriptDef, Stmt};
use thiserror::Error;

use super::{rk_base, LuaConst, LuaError, Op50, Proto};

#[derive(Error, Debug)]
pub enum IrError {
    #[error("Lua 5.0 parse error: {0}")]
    Parse(#[from] LuaError),
    #[error("unsupported Lua 5.0 construct: {0}")]
    Unsupported(String),
}

#[derive(Clone, Copy, Debug)]
struct Dec {
    op: Op50,
    a: usize,
    b: usize,
    c: usize,
    bx: usize,
    sbx: isize,
}

fn dec(ins: u32) -> Result<Dec, IrError> {
    let op = Op50::from_u32(ins)?;
    let a = ((ins >> 24) & 0xFF) as usize;
    let c = ((ins >> 6) & 0x1FF) as usize;
    let b = ((ins >> 15) & 0x1FF) as usize;
    let bx = ((ins >> 6) & 0x3FFFF) as usize;
    let sbx = bx as isize - 131_071;
    Ok(Dec {
        op,
        a,
        b,
        c,
        bx,
        sbx,
    })
}

fn jump_target(pc: usize, sbx: isize) -> Option<usize> {
    let t = pc as isize + 1 + sbx;
    (t >= 0).then_some(t as usize)
}

fn binop(op: Op50) -> Option<BinOp> {
    Some(match op {
        Op50::Add => BinOp::Add,
        Op50::Sub => BinOp::Sub,
        Op50::Mul => BinOp::Mul,
        Op50::Div => BinOp::Div,
        Op50::Pow => BinOp::Pow,
        _ => return None,
    })
}

fn cmp_binop(op: Op50) -> Option<BinOp> {
    Some(match op {
        Op50::Eq => BinOp::Eq,
        Op50::Lt => BinOp::Lt,
        Op50::Le => BinOp::Le,
        _ => return None,
    })
}

fn successors(code: &[Dec], pc: usize) -> Vec<usize> {
    let n = code.len();
    let d = code[pc];
    let mut out = Vec::new();
    let mut push = |t: usize| {
        if t < n {
            out.push(t);
        }
    };
    match d.op {
        Op50::Jmp => {
            if let Some(t) = jump_target(pc, d.sbx) {
                push(t);
            }
        }
        Op50::Eq | Op50::Lt | Op50::Le | Op50::Test => {
            push(pc + 1);
            push(pc + 2);
        }
        Op50::LoadBool if d.c != 0 => {
            push(pc + 2);
        }
        Op50::Return | Op50::TailCall => {}
        Op50::ForLoop => {
            if let Some(t) = jump_target(pc, d.sbx) {
                push(t);
            }
            push(pc + 1);
        }
        Op50::TForPrep => {
            if let Some(t) = jump_target(pc, d.sbx) {
                push(t);
            }
        }
        Op50::TForLoop => {
            push(pc + 1);
            push(pc + 2);
        }
        _ => push(pc + 1),
    }
    out
}

fn use_def(p: &Proto, code: &[Dec], base: usize, pc: usize) -> (Vec<usize>, Vec<usize>) {
    let d = code[pc];
    let top = usize::from(p.maxstack).max(d.a + 1);
    let rk = |x: usize| if x >= base { None } else { Some(x) };
    match d.op {
        Op50::Move => (vec![d.b], vec![d.a]),
        Op50::LoadK | Op50::LoadBool | Op50::GetUpval | Op50::GetGlobal | Op50::NewTable => {
            (Vec::new(), vec![d.a])
        }
        Op50::LoadNil => (Vec::new(), (d.a..=d.b).collect()),
        Op50::GetTable => {
            let mut u = vec![d.b];
            if let Some(r) = rk(d.c) {
                u.push(r);
            }
            (u, vec![d.a])
        }
        Op50::SetGlobal | Op50::SetUpval => (vec![d.a], Vec::new()),
        Op50::SetTable => {
            let mut u = vec![d.a];
            if let Some(r) = rk(d.b) {
                u.push(r);
            }
            if let Some(r) = rk(d.c) {
                u.push(r);
            }
            (u, Vec::new())
        }
        Op50::SelfOp => {
            let mut u = vec![d.b];
            if let Some(r) = rk(d.c) {
                u.push(r);
            }
            (u, vec![d.a, d.a + 1])
        }
        Op50::Add | Op50::Sub | Op50::Mul | Op50::Div | Op50::Pow => {
            let mut u = Vec::new();
            if let Some(r) = rk(d.b) {
                u.push(r);
            }
            if let Some(r) = rk(d.c) {
                u.push(r);
            }
            (u, vec![d.a])
        }
        Op50::Unm | Op50::Not => (vec![d.b], vec![d.a]),
        Op50::Concat => ((d.b..=d.c).collect(), vec![d.a]),
        Op50::Eq | Op50::Lt | Op50::Le => {
            let mut u = Vec::new();
            if let Some(r) = rk(d.b) {
                u.push(r);
            }
            if let Some(r) = rk(d.c) {
                u.push(r);
            }
            (u, Vec::new())
        }
        Op50::Test => (vec![d.b], Vec::new()),
        Op50::Call | Op50::TailCall => {
            let u_end = if d.b != 0 { d.a + d.b } else { top };
            let d_end = if d.c != 0 { d.a + d.c - 1 } else { top };
            ((d.a..u_end).collect(), (d.a..d_end).collect())
        }
        Op50::Return => {
            let u_end = if d.b != 0 { d.a + d.b - 1 } else { top };
            ((d.a..u_end).collect(), Vec::new())
        }
        Op50::ForLoop => (vec![d.a, d.a + 1, d.a + 2], vec![d.a]),
        Op50::TForPrep => (vec![d.a], vec![d.a, d.a + 1]),
        Op50::TForLoop => (
            vec![d.a, d.a + 1, d.a + 2],
            (d.a + 2..d.a + 3 + d.c).collect(),
        ),
        Op50::SetList | Op50::SetListO => {
            let n = d.bx % 32 + 1;
            ((d.a..=d.a + n).collect(), Vec::new())
        }
        Op50::Closure => (Vec::new(), vec![d.a]),
        Op50::Jmp | Op50::Close => (Vec::new(), Vec::new()),
    }
}

fn liveness(
    p: &Proto,
    code: &[Dec],
    base: usize,
) -> (Vec<BTreeSet<usize>>, Vec<BTreeSet<usize>>, Vec<Vec<usize>>) {
    let n = code.len();
    let mut ud: Vec<(Vec<usize>, Vec<usize>)> =
        (0..n).map(|i| use_def(p, code, base, i)).collect();
    let mut skip = BTreeSet::new();
    for i in 0..n {
        if code[i].op == Op50::Closure {
            if let Some(child) = p.protos.get(code[i].bx) {
                let nups = usize::from(child.nups);
                let mut caps = Vec::new();
                for j in 1..=nups {
                    if i + j < n {
                        skip.insert(i + j);
                        if code[i + j].op == Op50::Move {
                            caps.push(code[i + j].b);
                        }
                    }
                }
                ud[i] = (caps, vec![code[i].a]);
            }
        }
    }
    let mut succ: Vec<Vec<usize>> = (0..n)
        .map(|i| {
            if skip.contains(&i) {
                Vec::new()
            } else {
                successors(code, i)
            }
        })
        .collect();
    for i in 0..n {
        if code[i].op == Op50::Closure {
            if let Some(child) = p.protos.get(code[i].bx) {
                let next = i + 1 + usize::from(child.nups);
                succ[i] = if next < n { vec![next] } else { Vec::new() };
            }
        }
    }
    let mut live_in = vec![BTreeSet::new(); n];
    let mut live_out = vec![BTreeSet::new(); n];
    let mut changed = true;
    while changed {
        changed = false;
        for i in (0..n).rev() {
            let mut out = BTreeSet::new();
            for &s in &succ[i] {
                out.extend(&live_in[s]);
            }
            let (uses, defs) = &ud[i];
            let mut inn = out.clone();
            for &d in defs {
                inn.remove(&d);
            }
            for &u in uses {
                inn.insert(u);
            }
            if out != live_out[i] || inn != live_in[i] {
                live_out[i] = out;
                live_in[i] = inn;
                changed = true;
            }
        }
    }
    (live_in, live_out, succ)
}

fn post_dominators(succ: &[Vec<usize>], n: usize) -> Vec<usize> {
    let exit = n;
    let words = (n + 1).div_ceil(64);
    let mut full = vec![!0u64; words];
    let rem = (n + 1) % 64;
    if rem != 0 {
        full[words - 1] = (1u64 << rem) - 1;
    }
    let mut pdom = vec![full; n + 1];
    pdom[exit] = vec![0u64; words];
    pdom[exit][exit / 64] |= 1u64 << (exit % 64);

    let mut changed = true;
    while changed {
        changed = false;
        for i in (0..n).rev() {
            let mut new = vec![!0u64; words];
            if succ[i].is_empty() {
                new.clone_from(&pdom[exit]);
            } else {
                for &s in &succ[i] {
                    for w in 0..words {
                        new[w] &= pdom[s][w];
                    }
                }
            }
            new[i / 64] |= 1u64 << (i % 64);
            if new != pdom[i] {
                pdom[i] = new;
                changed = true;
            }
        }
    }
    let counts: Vec<u32> = pdom
        .iter()
        .map(|bits| bits.iter().map(|w| w.count_ones()).sum())
        .collect();
    let mut ipdom = vec![exit; n];
    for i in 0..n {
        let want = counts[i].saturating_sub(1);
        for c in 0..=n {
            if c != i && (pdom[i][c / 64] & (1u64 << (c % 64))) != 0 && counts[c] == want {
                ipdom[i] = c;
                break;
            }
        }
    }
    ipdom
}

#[derive(Clone, Debug, PartialEq)]
enum RegVal {
    Expr(Expr),
    SelfFn(Expr, Expr),
    Iter(IterKind, Expr),
}

impl RegVal {
    fn as_expr(&self) -> Result<Expr, IrError> {
        match self {
            RegVal::Expr(e) => Ok(e.clone()),
            RegVal::SelfFn(_, _) => Err(IrError::Unsupported("dangling SelfFn value".into())),
            RegVal::Iter(_, _) => Err(IrError::Unsupported("dangling Iter value".into())),
        }
    }
}

fn is_const(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Nil | Expr::Bool(_) | Expr::Num(_) | Expr::Str(_) | Expr::Closure(_, _)
    )
}

fn refs_volatile(e: &Expr) -> bool {
    match e {
        Expr::Global(_)
        | Expr::Index(_, _)
        | Expr::Upval(_)
        | Expr::Local(_)
        | Expr::Call(_, _)
        | Expr::Method(_, _, _)
        | Expr::Table(_) => true,
        Expr::Bin(_, a, b) => refs_volatile(a) || refs_volatile(b),
        Expr::Not(x) | Expr::Neg(x) => refs_volatile(x),
        Expr::Nil | Expr::Bool(_) | Expr::Num(_) | Expr::Str(_) | Expr::Closure(_, _) => false,
    }
}

fn reads_local_expr(e: &Expr, r: u16) -> bool {
    match e {
        Expr::Local(x) => *x == r,
        Expr::Index(a, b) | Expr::Bin(_, a, b) => reads_local_expr(a, r) || reads_local_expr(b, r),
        Expr::Not(x) | Expr::Neg(x) => reads_local_expr(x, r),
        Expr::Table(pairs) => pairs
            .iter()
            .any(|(k, v)| reads_local_expr(k, r) || reads_local_expr(v, r)),
        Expr::Call(f, args) => {
            reads_local_expr(f, r) || args.iter().any(|x| reads_local_expr(x, r))
        }
        Expr::Method(o, _, args) => {
            reads_local_expr(o, r) || args.iter().any(|x| reads_local_expr(x, r))
        }
        _ => false,
    }
}

fn reads_local(v: &RegVal, r: u16) -> bool {
    match v {
        RegVal::Expr(e) => reads_local_expr(e, r),
        RegVal::SelfFn(o, m) => reads_local_expr(o, r) || reads_local_expr(m, r),
        RegVal::Iter(_, c) => reads_local_expr(c, r),
    }
}

fn const_expr(c: &LuaConst) -> Result<Expr, IrError> {
    Ok(match c {
        LuaConst::Nil => Expr::Nil,
        LuaConst::Bool(b) => Expr::Bool(*b),
        LuaConst::Num(n) => {
            let f = *n as f32;
            if !f.is_finite() {
                return Err(IrError::Unsupported("non-finite numeric constant".into()));
            }
            Expr::Num(f)
        }
        LuaConst::Str(s) => Expr::Str(s.clone()),
    })
}

#[derive(Clone, Copy, Debug, Default)]
struct LoopCtx {
    break_target: Option<usize>,
    continue_target: Option<usize>,
    /// Inside a bottom-tested `while` condition: `(body_pc, flag_local)`.
    cond_ok: Option<(usize, u16)>,
}

struct Unit<'a> {
    base: usize,
    fns: Vec<Option<FnDef>>,
    ids: BTreeMap<usize, u32>,
    queue: VecDeque<(&'a Proto, u32, String)>,
    parent: Vec<Option<u32>>,
    current: Option<u32>,
}

impl<'a> Unit<'a> {
    fn add_proto(&mut self, p: &'a Proto, name: String) -> u32 {
        let key = p as *const Proto as usize;
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        let fid = self.fns.len() as u32;
        self.ids.insert(key, fid);
        self.fns.push(None);
        self.queue.push_back((p, fid, name));
        self.parent.push(self.current);
        fid
    }
}

type RegState = BTreeMap<usize, RegVal>;

struct FnTranslator<'a, 'u> {
    unit: &'u mut Unit<'a>,
    p: &'a Proto,
    name: String,
    base: usize,
    code: Vec<Dec>,
    n: usize,
    live_in: Vec<BTreeSet<usize>>,
    live_out: Vec<BTreeSet<usize>>,
    ipdom: Vec<usize>,
    back_jmps_to: BTreeMap<usize, Vec<usize>>,
    captured: BTreeSet<usize>,
    vararg: bool,
    nlocals: u16,
}

impl<'a, 'u> FnTranslator<'a, 'u> {
    fn new(unit: &'u mut Unit<'a>, p: &'a Proto, name: String) -> Result<Self, IrError> {
        let base = unit.base;
        let code: Vec<Dec> = p.code.iter().map(|&ins| dec(ins)).collect::<Result<_, _>>()?;
        let n = code.len();
        let (live_in, live_out, succ) = liveness(p, &code, base);
        let ipdom = post_dominators(&succ, n);
        let mut back_jmps_to: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (pc, d) in code.iter().enumerate() {
            if d.op == Op50::Jmp {
                if let Some(target) = jump_target(pc, d.sbx) {
                    if target <= pc {
                        let prev_tfor = pc > 0 && code[pc - 1].op == Op50::TForLoop;
                        if !prev_tfor {
                            back_jmps_to.entry(target).or_default().push(pc);
                        }
                    }
                }
            }
        }
        let vararg = p.is_vararg != 0;
        let nlocals = u16::from(p.maxstack).max(u16::from(p.numparams) + u16::from(vararg));
        Ok(Self {
            unit,
            p,
            name,
            base,
            code,
            n,
            live_in,
            live_out,
            ipdom,
            back_jmps_to,
            captured: BTreeSet::new(),
            vararg,
            nlocals,
        })
    }

    fn alloc_local(&mut self) -> u16 {
        let r = self.nlocals;
        self.nlocals = self.nlocals.saturating_add(1);
        r
    }

    fn k(&self, x: usize) -> Result<Expr, IrError> {
        let c = self
            .p
            .constants
            .get(x)
            .ok_or_else(|| IrError::Unsupported(format!("{}: constant {x} out of range", self.name)))?;
        const_expr(c)
    }

    fn k_str(&self, x: usize) -> Result<String, IrError> {
        match self.p.constants.get(x) {
            Some(LuaConst::Str(s)) => Ok(s.clone()),
            _ => Err(IrError::Unsupported(format!(
                "{}: expected string constant at {x}",
                self.name
            ))),
        }
    }

    fn read(&self, st: &RegState, r: usize) -> RegVal {
        st.get(&r)
            .cloned()
            .unwrap_or(RegVal::Expr(Expr::Local(r as u16)))
    }

    fn read_expr(&self, st: &RegState, r: usize) -> Result<Expr, IrError> {
        self.read(st, r).as_expr()
    }

    fn rk(&self, st: &RegState, x: usize) -> Result<Expr, IrError> {
        if x >= self.base {
            self.k(x - self.base)
        } else {
            self.read_expr(st, x)
        }
    }

    fn materialize(&self, st: &mut RegState, out: &mut Vec<Stmt>, regs: &[usize]) -> Result<(), IrError> {
        let mut sorted: Vec<usize> = regs.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        for r in sorted {
            let Some(v) = st.get(&r) else { continue };
            if *v == RegVal::Expr(Expr::Local(r as u16)) {
                continue;
            }
            if let RegVal::Expr(e) = v {
                out.push(Stmt::SetLocal(r as u16, e.clone()));
                st.insert(r, RegVal::Expr(Expr::Local(r as u16)));
            }
        }
        Ok(())
    }

    fn before_effect(
        &self,
        st: &mut RegState,
        out: &mut Vec<Stmt>,
        pc: usize,
        keep: impl Fn(usize) -> bool,
    ) -> Result<(), IrError> {
        let regs: Vec<usize> = self.live_out[pc]
            .iter()
            .copied()
            .filter(|&r| {
                !keep(r)
                    && matches!(
                        st.get(&r),
                        Some(RegVal::Expr(e))
                            if *e != Expr::Local(r as u16) && refs_volatile(e)
                    )
            })
            .collect();
        self.materialize(st, out, &regs)
    }

    fn set_reg(
        &self,
        st: &mut RegState,
        out: &mut Vec<Stmt>,
        pc: usize,
        r: usize,
        e: Expr,
    ) -> Result<(), IrError> {
        let dep: Vec<usize> = self.live_out[pc]
            .iter()
            .copied()
            .filter(|&q| q != r && st.get(&q).is_some_and(|v| reads_local(v, r as u16)))
            .collect();
        self.materialize(st, out, &dep)?;
        if self.captured.contains(&r) {
            out.push(Stmt::SetLocal(r as u16, e));
            st.insert(r, RegVal::Expr(Expr::Local(r as u16)));
        } else {
            st.insert(r, RegVal::Expr(e));
        }
        Ok(())
    }

    fn take_table(
        &self,
        st: &mut RegState,
        out: &mut Vec<Stmt>,
        pc: usize,
        r: usize,
    ) -> Result<Expr, IrError> {
        let e = self.read_expr(st, r)?;
        if matches!(e, Expr::Table(_)) && self.live_out[pc].contains(&r) {
            self.materialize(st, out, &[r])?;
            return Ok(Expr::Local(r as u16));
        }
        Ok(e)
    }

    fn join_state(
        &self,
        out_a: &mut Vec<Stmt>,
        st_a: &RegState,
        out_b: &mut Vec<Stmt>,
        st_b: &RegState,
        join: usize,
    ) -> Result<RegState, IrError> {
        let empty = BTreeSet::new();
        let live = self.live_in.get(join).unwrap_or(&empty);
        let mut merged = RegState::new();
        for &r in live {
            let ea = st_a.get(&r);
            let eb = st_b.get(&r);
            if ea == eb && ea.is_some() {
                merged.insert(r, ea.unwrap().clone());
                continue;
            }
            if let Some(RegVal::Expr(e)) = ea {
                if *e != Expr::Local(r as u16) {
                    out_a.push(Stmt::SetLocal(r as u16, e.clone()));
                }
            }
            if let Some(RegVal::Expr(e)) = eb {
                if *e != Expr::Local(r as u16) {
                    out_b.push(Stmt::SetLocal(r as u16, e.clone()));
                }
            }
            merged.insert(r, RegVal::Expr(Expr::Local(r as u16)));
        }
        Ok(merged)
    }

    fn materialize_live_in(
        &self,
        st: &mut RegState,
        out: &mut Vec<Stmt>,
        target_pc: usize,
    ) -> Result<(), IrError> {
        if let Some(live) = self.live_in.get(target_pc) {
            let regs: Vec<usize> = live
                .iter()
                .copied()
                .filter(|r| {
                    matches!(st.get(r), Some(RegVal::Expr(e)) if *e != Expr::Local(*r as u16))
                })
                .collect();
            self.materialize(st, out, &regs)?;
        }
        Ok(())
    }

    fn translate(mut self) -> Result<FnDef, IrError> {
        let mut st = RegState::new();
        for i in 0..(usize::from(self.p.numparams) + usize::from(self.vararg)) {
            st.insert(i, RegVal::Expr(Expr::Local(i as u16)));
        }
        let (body, _) = self.emit(0, self.n, st, LoopCtx::default(), 0)?;
        Ok(FnDef {
            name: self.name,
            params: u16::from(self.p.numparams),
            vararg: self.vararg,
            locals: self.nlocals,
            body,
        })
    }

    fn next_is_tforprep(&self, pc: usize, a: usize) -> bool {
        self.code
            .get(pc)
            .is_some_and(|d| d.op == Op50::TForPrep && d.a == a)
    }

    fn emit(
        &mut self,
        mut pc: usize,
        stop: usize,
        mut st: RegState,
        lctx: LoopCtx,
        depth: usize,
    ) -> Result<(Vec<Stmt>, Option<RegState>), IrError> {
        if depth > 200 {
            return Err(IrError::Unsupported(format!(
                "{}: nesting too deep",
                self.name
            )));
        }
        let mut out = Vec::new();
        while pc != stop {
            if pc >= self.n {
                return Ok((out, None));
            }
            // Check for a `repeat ... until` / top-tested loop header at `pc`
            // (when we are not already emitting the body of this loop).
            if lctx.continue_target != Some(pc) {
                if let Some(backs) = self.back_jmps_to.get(&pc) {
                    if let Some(&last_back) = backs.iter().filter(|&&b| b >= pc && b < stop).max() {
                        let exit_pc = last_back + 1;
                        self.before_effect(&mut st, &mut out, pc, |_| false)?;
                        self.materialize_live_in(&mut st, &mut out, pc)?;
                        let body_st: RegState = self.live_in[pc]
                            .iter()
                            .map(|&r| (r, RegVal::Expr(Expr::Local(r as u16))))
                            .collect();
                        let inner = LoopCtx {
                            break_target: Some(exit_pc),
                            continue_target: Some(pc),
                            cond_ok: None,
                        };
                        let (mut lbody, end_st) =
                            self.emit(pc, exit_pc, body_st, inner, depth + 1)?;
                        if let Some(mut est) = end_st {
                            self.materialize_live_in(&mut est, &mut lbody, exit_pc)?;
                            lbody.push(Stmt::Break);
                        }
                        out.push(Stmt::Loop(lbody));
                        st = if exit_pc < self.n {
                            self.live_in[exit_pc]
                                .iter()
                                .map(|&r| (r, RegVal::Expr(Expr::Local(r as u16))))
                                .collect()
                        } else {
                            RegState::new()
                        };
                        pc = exit_pc;
                        continue;
                    }
                }
            }

            let d = self.code[pc];
            match d.op {
                Op50::Move => {
                    let v = self.take_table(&mut st, &mut out, pc, d.b)?;
                    self.set_reg(&mut st, &mut out, pc, d.a, v)?;
                }
                Op50::LoadK => {
                    let v = self.k(d.bx)?;
                    self.set_reg(&mut st, &mut out, pc, d.a, v)?;
                }
                Op50::LoadBool => {
                    self.set_reg(&mut st, &mut out, pc, d.a, Expr::Bool(d.b != 0))?;
                    if d.c != 0 {
                        pc += 2;
                        continue;
                    }
                }
                Op50::LoadNil => {
                    for r in d.a..=d.b {
                        self.set_reg(&mut st, &mut out, pc, r, Expr::Nil)?;
                    }
                }
                Op50::GetUpval => {
                    self.set_reg(&mut st, &mut out, pc, d.a, Expr::Upval(d.b as u16))?;
                }
                Op50::GetGlobal => {
                    let g = self.k_str(d.bx)?;
                    self.set_reg(&mut st, &mut out, pc, d.a, Expr::Global(g))?;
                }
                Op50::GetTable => {
                    let t = self.take_table(&mut st, &mut out, pc, d.b)?;
                    let k = self.rk(&st, d.c)?;
                    self.set_reg(&mut st, &mut out, pc, d.a, Expr::Index(Box::new(t), Box::new(k)))?;
                }
                Op50::SetGlobal => {
                    let g = self.k_str(d.bx)?;
                    let v = self.take_table(&mut st, &mut out, pc, d.a)?;
                    self.before_effect(&mut st, &mut out, pc, |_| false)?;
                    out.push(Stmt::SetGlobal(g, v));
                }
                Op50::SetUpval => {
                    let v = self.take_table(&mut st, &mut out, pc, d.a)?;
                    self.before_effect(&mut st, &mut out, pc, |_| false)?;
                    out.push(Stmt::SetUpval(d.b as u16, v));
                }
                Op50::SetTable => {
                    let mut t = self.read_expr(&st, d.a)?;
                    let key = self.rk(&st, d.b)?;
                    let val = if d.c < self.base {
                        self.take_table(&mut st, &mut out, pc, d.c)?
                    } else {
                        self.rk(&st, d.c)?
                    };
                    if let Expr::Table(mut pairs) = t {
                        if is_const(&key) {
                            pairs.push((key, val));
                            st.insert(d.a, RegVal::Expr(Expr::Table(pairs)));
                            pc += 1;
                            continue;
                        }
                        self.materialize(&mut st, &mut out, &[d.a])?;
                        t = Expr::Local(d.a as u16);
                    }
                    self.before_effect(&mut st, &mut out, pc, |r| r == d.a)?;
                    out.push(Stmt::SetIndex(t, key, val));
                }
                Op50::NewTable => {
                    self.set_reg(&mut st, &mut out, pc, d.a, Expr::Table(Vec::new()))?;
                }
                Op50::SetList | Op50::SetListO => {
                    let t = self.read_expr(&st, d.a)?;
                    let nvals = d.bx % 32 + 1;
                    let first = d.bx - d.bx % 32;
                    let mut items = Vec::with_capacity(nvals);
                    for i in 1..=nvals {
                        let v = self.take_table(&mut st, &mut out, pc, d.a + i)?;
                        items.push((Expr::Num((first + i) as f32), v));
                    }
                    if let Expr::Table(mut pairs) = t {
                        pairs.extend(items);
                        st.insert(d.a, RegVal::Expr(Expr::Table(pairs)));
                    } else {
                        for (kk, vv) in items {
                            out.push(Stmt::SetIndex(t.clone(), kk, vv));
                        }
                    }
                }
                Op50::SelfOp => {
                    let mut obj = self.take_table(&mut st, &mut out, pc, d.b)?;
                    if !matches!(obj, Expr::Local(_) | Expr::Global(_) | Expr::Upval(_))
                        && !is_const(&obj)
                    {
                        self.materialize(&mut st, &mut out, &[d.b])?;
                        obj = Expr::Local(d.b as u16);
                    }
                    let meth = self.rk(&st, d.c)?;
                    st.insert(d.a + 1, RegVal::Expr(obj.clone()));
                    st.insert(d.a, RegVal::SelfFn(obj, meth));
                }
                Op50::Add | Op50::Sub | Op50::Mul | Op50::Div | Op50::Pow => {
                    let op = binop(d.op).unwrap();
                    let lhs = self.rk(&st, d.b)?;
                    let rhs = self.rk(&st, d.c)?;
                    self.set_reg(
                        &mut st,
                        &mut out,
                        pc,
                        d.a,
                        Expr::Bin(op, Box::new(lhs), Box::new(rhs)),
                    )?;
                }
                Op50::Unm => {
                    let v = self.read_expr(&st, d.b)?;
                    self.set_reg(&mut st, &mut out, pc, d.a, Expr::Neg(Box::new(v)))?;
                }
                Op50::Not => {
                    let v = self.read_expr(&st, d.b)?;
                    self.set_reg(&mut st, &mut out, pc, d.a, Expr::Not(Box::new(v)))?;
                }
                Op50::Concat => {
                    let mut e = self.read_expr(&st, d.b)?;
                    for r in (d.b + 1)..=d.c {
                        let rhs = self.read_expr(&st, r)?;
                        e = Expr::Bin(BinOp::Concat, Box::new(e), Box::new(rhs));
                    }
                    self.set_reg(&mut st, &mut out, pc, d.a, e)?;
                }
                Op50::Call | Op50::TailCall => {
                    let fn_val = self.read(&st, d.a);
                    let mut args = Vec::new();
                    let arg_end = if d.b == 0 {
                        st.keys().copied().filter(|&r| r > d.a).max().unwrap_or(d.a) + 1
                    } else {
                        d.a + d.b
                    };
                    for r in (d.a + 1)..arg_end {
                        args.push(self.take_table(&mut st, &mut out, pc, r)?);
                    }
                    let call = match &fn_val {
                        RegVal::SelfFn(obj, meth) => {
                            let Expr::Str(mname) = meth else {
                                return Err(IrError::Unsupported(format!(
                                    "{} pc {pc}: dynamic method name",
                                    self.name
                                )));
                            };
                            let (recv, rest) = if args.is_empty() {
                                (obj.clone(), Vec::new())
                            } else {
                                (args[0].clone(), args[1..].to_vec())
                            };
                            Expr::Method(Box::new(recv), mname.clone(), rest)
                        }
                        RegVal::Expr(f) => Expr::Call(Box::new(f.clone()), args.clone()),
                        RegVal::Iter(_, _) => {
                            return Err(IrError::Unsupported(format!(
                                "{} pc {pc}: calling Iter",
                                self.name
                            )));
                        }
                    };
                    let nres = d.c as isize - 1;
                    if let RegVal::Expr(Expr::Global(ref g)) = fn_val {
                        if matches!(g.as_str(), "ipairs" | "pairs")
                            && nres >= 3
                            && self.next_is_tforprep(pc + 1, d.a)
                        {
                            st.retain(|&r, _| r <= d.a);
                            let kind = if g == "ipairs" {
                                IterKind::Ipairs
                            } else {
                                IterKind::Pairs
                            };
                            let coll = args.into_iter().next().unwrap_or(Expr::Nil);
                            st.insert(d.a, RegVal::Iter(kind, coll));
                            pc += 1;
                            continue;
                        }
                    }
                    let keep_end = d.a + nres.max(1) as usize;
                    self.before_effect(&mut st, &mut out, pc, |r| r >= d.a && r < keep_end)?;
                    st.retain(|&r, _| r < d.a);
                    if d.op == Op50::TailCall {
                        out.push(Stmt::Return(vec![call]));
                        return Ok((out, None));
                    }
                    if nres == 0 {
                        out.push(Stmt::Do(call));
                    } else if nres == 1 {
                        out.push(Stmt::SetLocal(d.a as u16, call));
                        st.insert(d.a, RegVal::Expr(Expr::Local(d.a as u16)));
                    } else if nres > 1 {
                        let regs: Vec<u16> = (d.a..d.a + nres as usize).map(|r| r as u16).collect();
                        out.push(Stmt::SetLocals(regs, call));
                        for r in d.a..d.a + nres as usize {
                            st.insert(r, RegVal::Expr(Expr::Local(r as u16)));
                        }
                    } else {
                        st.insert(d.a, RegVal::Expr(call));
                    }
                }
                Op50::Return => {
                    if d.b == 0 {
                        return Err(IrError::Unsupported(format!(
                            "{} pc {pc}: open return",
                            self.name
                        )));
                    }
                    let mut vals = Vec::with_capacity(d.b.saturating_sub(1));
                    for r in d.a..d.a + d.b - 1 {
                        vals.push(self.take_table(&mut st, &mut out, pc, r)?);
                    }
                    out.push(Stmt::Return(vals));
                    return Ok((out, None));
                }
                Op50::Closure => {
                    let child = self.p.protos.get(d.bx).ok_or_else(|| {
                        IrError::Unsupported(format!("{} pc {pc}: bad proto {}", self.name, d.bx))
                    })?;
                    let nups = usize::from(child.nups);
                    let mut caps = Vec::with_capacity(nups);
                    for j in 1..=nups {
                        let q = *self.code.get(pc + j).ok_or_else(|| {
                            IrError::Unsupported(format!("{} pc {pc}: truncated closure", self.name))
                        })?;
                        match q.op {
                            Op50::Move => {
                                self.materialize(&mut st, &mut out, &[q.b])?;
                                self.captured.insert(q.b);
                                caps.push(Expr::Local(q.b as u16));
                            }
                            Op50::GetUpval => {
                                caps.push(Expr::Upval(q.b as u16));
                            }
                            other => {
                                return Err(IrError::Unsupported(format!(
                                    "{} pc {pc}: closure capture {other:?}",
                                    self.name
                                )));
                            }
                        }
                    }
                    let fid = self.unit.add_proto(child, format!("{}.{}", self.name, d.bx));
                    self.set_reg(&mut st, &mut out, pc, d.a, Expr::Closure(fid, caps))?;
                    pc += 1 + nups;
                    continue;
                }
                Op50::Close => {
                    self.before_effect(&mut st, &mut out, pc, |_| false)?;
                    out.push(Stmt::Close(d.a as u16));
                }
                Op50::Eq | Op50::Lt | Op50::Le | Op50::Test => {
                    let next = *self.code.get(pc + 1).ok_or_else(|| {
                        IrError::Unsupported(format!("{} pc {pc}: {:?} at end", self.name, d.op))
                    })?;
                    if next.op != Op50::Jmp {
                        return Err(IrError::Unsupported(format!(
                            "{} pc {pc}: {:?} without JMP",
                            self.name, d.op
                        )));
                    }
                    let taken = jump_target(pc + 1, next.sbx).ok_or_else(|| {
                        IrError::Unsupported(format!("{} pc {pc}: negative jump target", self.name))
                    })?;
                    let fall = pc + 2;
                    let cond_fall = if d.op == Op50::Test {
                        let v = self.read_expr(&st, d.b)?;
                        if d.c == 0 {
                            v
                        } else {
                            Expr::Not(Box::new(v))
                        }
                    } else {
                        let x = self.rk(&st, d.b)?;
                        let y = self.rk(&st, d.c)?;
                        let cmp = Expr::Bin(cmp_binop(d.op).unwrap(), Box::new(x), Box::new(y));
                        if d.a == 0 {
                            cmp
                        } else {
                            Expr::Not(Box::new(cmp))
                        }
                    };
                    let st_f = st.clone();
                    let mut st_t = st.clone();
                    if d.op == Op50::Test && d.a != d.b {
                        let v = self.read(&st, d.b);
                        st_t.insert(d.a, v);
                    }

                    // Check if the taken branch is a loop-control jump (while cond ok,
                    // continue, or break).
                    if let Some((arm_t, is_loop_ctl)) =
                        self.try_loop_jump(taken, &mut st_t, lctx)?
                    {
                        if is_loop_ctl {
                            let (arm_f, end_f) = self.emit(fall, stop, st_f, lctx, depth + 1)?;
                            out.push(Stmt::If(cond_fall, arm_f, arm_t));
                            return Ok((out, end_f));
                        }
                    }

                    let mut join = self.ipdom[pc];
                    if join == pc + 1 {
                        join = self.ipdom[pc + 1];
                    }
                    if join > stop && stop != self.n {
                        join = stop;
                    }
                    let (mut arm_f, end_f) = self.emit(fall, join, st_f, lctx, depth + 1)?;
                    let (mut arm_t, end_t) = self.emit(taken, join, st_t, lctx, depth + 1)?;
                    out.push(Stmt::If(cond_fall, arm_f.clone(), arm_t.clone()));
                    if end_f.is_none() && end_t.is_none() {
                        return Ok((out, None));
                    }
                    if join >= self.n {
                        return Ok((out, None));
                    }
                    st = match (end_f, end_t) {
                        (None, Some(et)) => et,
                        (Some(ef), None) => ef,
                        (Some(ef), Some(et)) => {
                            let merged =
                                self.join_state(&mut arm_f, &ef, &mut arm_t, &et, join)?;
                            // Update the last If with any join SetLocals appended.
                            if let Some(Stmt::If(_, f_ref, t_ref)) = out.last_mut() {
                                *f_ref = arm_f;
                                *t_ref = arm_t;
                            }
                            merged
                        }
                        (None, None) => unreachable!(),
                    };
                    pc = join;
                    continue;
                }
                Op50::Jmp => {
                    let target = jump_target(pc, d.sbx).ok_or_else(|| {
                        IrError::Unsupported(format!("{} pc {pc}: negative JMP", self.name))
                    })?;
                    // Numeric `for` (`SUB A A A+2; JMP -> FORLOOP A`)
                    if target > pc {
                        if let Some(tgt) = self.code.get(target) {
                            if tgt.op == Op50::ForLoop
                                && jump_target(target, tgt.sbx) == Some(pc + 1)
                            {
                                pc = self.emit_fornum(pc, target, &mut st, &mut out, depth)?;
                                continue;
                            }
                        }
                    }
                    // Bottom-tested `while` (`JMP -> cond_pc`, body at `pc+1..cond_pc`,
                    // backward `JMP -> pc+1` at `>= cond_pc`).
                    if target > pc {
                        let body_pc = pc + 1;
                        if let Some(backs) = self.back_jmps_to.get(&body_pc) {
                            if let Some(&last_back) =
                                backs.iter().filter(|&&b| b >= target && b < stop).max()
                            {
                                pc = self.emit_while(
                                    pc, target, last_back, &mut st, &mut out, depth,
                                )?;
                                continue;
                            }
                        }
                    }
                    // Loop control jumps (`break`, `continue`, `cond_ok`).
                    if let Some((stmts, true)) = self.try_loop_jump(target, &mut st, lctx)? {
                        out.extend(stmts);
                        return Ok((out, None));
                    }
                    if target <= pc {
                        return Err(IrError::Unsupported(format!(
                            "{} pc {pc}: unhandled backward jump -> {target}",
                            self.name
                        )));
                    }
                    if stop < self.n && target > stop {
                        return Err(IrError::Unsupported(format!(
                            "{} pc {pc}: jump out of block ({target} > {stop})",
                            self.name
                        )));
                    }
                    pc = target;
                    continue;
                }
                Op50::TForPrep => {
                    pc = self.emit_forin(pc, d.sbx, d.a, &mut st, &mut out, depth)?;
                    continue;
                }
                other => {
                    return Err(IrError::Unsupported(format!(
                        "{} pc {pc}: opcode {other:?}",
                        self.name
                    )));
                }
            }
            pc += 1;
        }
        Ok((out, Some(st)))
    }

    /// If `target` is a loop control target (`cond_ok`, `continue`, or `break`),
    /// materialize live-ins and return the terminal statements.
    fn try_loop_jump(
        &self,
        target: usize,
        st: &mut RegState,
        lctx: LoopCtx,
    ) -> Result<Option<(Vec<Stmt>, bool)>, IrError> {
        if let Some((body_pc, flag)) = lctx.cond_ok {
            if target == body_pc {
                let mut stmts = Vec::new();
                self.materialize_live_in(st, &mut stmts, body_pc)?;
                stmts.push(Stmt::SetLocal(flag, Expr::Bool(true)));
                return Ok(Some((stmts, true)));
            }
            if lctx.break_target == Some(target) {
                let mut stmts = Vec::new();
                self.materialize_live_in(st, &mut stmts, target)?;
                return Ok(Some((stmts, true)));
            }
        }
        if lctx.continue_target == Some(target) {
            let mut stmts = Vec::new();
            self.materialize_live_in(st, &mut stmts, target)?;
            stmts.push(Stmt::Continue);
            return Ok(Some((stmts, true)));
        }
        if lctx.break_target == Some(target) {
            let mut stmts = Vec::new();
            self.materialize_live_in(st, &mut stmts, target)?;
            stmts.push(Stmt::Break);
            return Ok(Some((stmts, true)));
        }
        Ok(None)
    }

    fn emit_while(
        &mut self,
        jmp_pc: usize,
        cond_pc: usize,
        last_back: usize,
        st: &mut RegState,
        out: &mut Vec<Stmt>,
        depth: usize,
    ) -> Result<usize, IrError> {
        let body_pc = jmp_pc + 1;
        let exit_pc = last_back + 1;
        self.before_effect(st, out, jmp_pc, |_| false)?;
        self.materialize_live_in(st, out, cond_pc)?;
        self.materialize_live_in(st, out, body_pc)?;
        let flag = self.alloc_local();

        let cond_st: RegState = self.live_in[cond_pc]
            .iter()
            .map(|&r| (r, RegVal::Expr(Expr::Local(r as u16))))
            .collect();
        let cond_ctx = LoopCtx {
            break_target: Some(exit_pc),
            continue_target: None,
            cond_ok: Some((body_pc, flag)),
        };
        let (cond_stmts, _) = self.emit(cond_pc, exit_pc, cond_st, cond_ctx, depth + 1)?;

        let body_st: RegState = self.live_in[body_pc]
            .iter()
            .map(|&r| (r, RegVal::Expr(Expr::Local(r as u16))))
            .collect();
        let body_ctx = LoopCtx {
            break_target: Some(exit_pc),
            continue_target: None,
            cond_ok: None,
        };
        let (mut body_stmts, end_body) =
            self.emit(body_pc, cond_pc, body_st, body_ctx, depth + 1)?;
        if let Some(mut eb) = end_body {
            self.materialize_live_in(&mut eb, &mut body_stmts, cond_pc)?;
        }

        let mut loop_body = Vec::with_capacity(cond_stmts.len() + body_stmts.len() + 2);
        loop_body.push(Stmt::SetLocal(flag, Expr::Bool(false)));
        loop_body.extend(cond_stmts);
        loop_body.push(Stmt::If(
            Expr::Not(Box::new(Expr::Local(flag))),
            vec![Stmt::Break],
            Vec::new(),
        ));
        loop_body.extend(body_stmts);
        out.push(Stmt::Loop(loop_body));

        *st = if exit_pc < self.n {
            self.live_in[exit_pc]
                .iter()
                .map(|&r| (r, RegVal::Expr(Expr::Local(r as u16))))
                .collect()
        } else {
            RegState::new()
        };
        Ok(exit_pc)
    }

    fn emit_fornum(
        &mut self,
        jmp_pc: usize,
        loop_pc: usize,
        st: &mut RegState,
        out: &mut Vec<Stmt>,
        depth: usize,
    ) -> Result<usize, IrError> {
        if jmp_pc == 0 {
            return Err(IrError::Unsupported(format!(
                "{} pc {jmp_pc}: unrecognised numeric for",
                self.name
            )));
        }
        let prev = self.code[jmp_pc - 1];
        let fl = self.code[loop_pc];
        if prev.op != Op50::Sub
            || prev.a != fl.a
            || jump_target(loop_pc, fl.sbx) != Some(jmp_pc + 1)
        {
            return Err(IrError::Unsupported(format!(
                "{} pc {jmp_pc}: unrecognised numeric for",
                self.name
            )));
        }
        let fa = fl.a;
        let step = self.read_expr(st, fa + 2)?;
        let start = Expr::Bin(
            BinOp::Add,
            Box::new(self.read_expr(st, fa)?),
            Box::new(step.clone()),
        );
        let limit = self.read_expr(st, fa + 1)?;
        self.before_effect(st, out, jmp_pc, |_| false)?;
        st.retain(|&r, _| r < fa);
        self.materialize_live_in(st, out, jmp_pc + 1)?;
        let mut body_st: RegState = self.live_in[jmp_pc + 1]
            .iter()
            .map(|&r| (r, RegVal::Expr(Expr::Local(r as u16))))
            .collect();
        for (&r, v) in st.iter() {
            if *v == RegVal::Expr(Expr::Local(r as u16)) {
                body_st.insert(r, v.clone());
            }
        }
        let lctx = LoopCtx {
            break_target: Some(loop_pc + 1),
            continue_target: None,
            cond_ok: None,
        };
        let (mut body, end) = self.emit(jmp_pc + 1, loop_pc, body_st, lctx, depth + 1)?;
        if let Some(mut e) = end {
            self.materialize_live_in(&mut e, &mut body, loop_pc)?;
        }
        out.push(Stmt::ForNum(fa as u16, start, limit, step, body));
        Ok(loop_pc + 1)
    }

    fn emit_forin(
        &mut self,
        prep_pc: usize,
        sbx: isize,
        a: usize,
        st: &mut RegState,
        out: &mut Vec<Stmt>,
        depth: usize,
    ) -> Result<usize, IrError> {
        let loop_pc = jump_target(prep_pc, sbx).ok_or_else(|| {
            IrError::Unsupported(format!("{} pc {prep_pc}: bad TFORPREP target", self.name))
        })?;
        let lop = *self.code.get(loop_pc).ok_or_else(|| {
            IrError::Unsupported(format!("{} pc {prep_pc}: TFORLOOP out of range", self.name))
        })?;
        let jop = *self.code.get(loop_pc + 1).ok_or_else(|| {
            IrError::Unsupported(format!("{} pc {prep_pc}: TFORLOOP JMP missing", self.name))
        })?;
        if lop.op != Op50::TForLoop
            || lop.a != a
            || jop.op != Op50::Jmp
            || jump_target(loop_pc + 1, jop.sbx) != Some(prep_pc + 1)
        {
            return Err(IrError::Unsupported(format!(
                "{} pc {prep_pc}: unrecognised generic for",
                self.name
            )));
        }
        let (kind, coll) = match self.read(st, a) {
            RegVal::Iter(k, c) => (k, c),
            other => (IterKind::Pairs, other.as_expr()?),
        };
        let vars: Vec<u16> = ((a + 2)..(a + 3 + lop.c)).map(|r| r as u16).collect();
        self.before_effect(st, out, prep_pc, |_| false)?;
        st.retain(|&r, _| r < a);
        self.materialize_live_in(st, out, prep_pc + 1)?;
        let mut body_st = st.clone();
        for &r in &vars {
            body_st.insert(usize::from(r), RegVal::Expr(Expr::Local(r)));
        }
        let lctx = LoopCtx {
            break_target: Some(loop_pc + 2),
            continue_target: None,
            cond_ok: None,
        };
        let (mut body, end) = self.emit(prep_pc + 1, loop_pc, body_st, lctx, depth + 1)?;
        if let Some(mut e) = end {
            self.materialize_live_in(&mut e, &mut body, loop_pc)?;
        }
        out.push(Stmt::ForIn(kind, coll, vars, body));
        Ok(loop_pc + 2)
    }
}

fn name_functions(fns: &mut [FnDef], parent: &[Option<u32>]) {
    let mut named: BTreeMap<u32, String> = BTreeMap::new();
    named.insert(0, "main".to_owned());

    fn visit_stmts(stmts: &[Stmt], named: &mut BTreeMap<u32, String>) {
        for s in stmts {
            match s {
                Stmt::SetGlobal(g, Expr::Closure(fid, _)) => {
                    named.entry(*fid).or_insert_with(|| g.clone());
                }
                Stmt::SetIndex(Expr::Global(g), Expr::Str(field), Expr::Closure(fid, _)) => {
                    named.entry(*fid).or_insert_with(|| format!("{g}.{field}"));
                }
                Stmt::If(_, a, b) => {
                    visit_stmts(a, named);
                    visit_stmts(b, named);
                }
                Stmt::ForNum(_, _, _, _, b) | Stmt::ForIn(_, _, _, b) | Stmt::Loop(b) => {
                    visit_stmts(b, named);
                }
                _ => {}
            }
        }
    }

    for f in fns.iter() {
        visit_stmts(&f.body, &mut named);
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for fid in 0..fns.len() {
        let fid_u32 = fid as u32;
        if let Some(n) = named.get(&fid_u32) {
            fns[fid].name = n.clone();
        } else {
            let par = parent[fid]
                .map(|p| fns[p as usize].name.clone())
                .unwrap_or_else(|| "main".to_owned());
            let c = counts.entry(par.clone()).or_insert(0);
            *c += 1;
            fns[fid].name = format!("{par}.{c}");
        }
    }
}

/// Translate a parsed Lua 5.0 prototype tree into openbf's [`ScriptDef`] IR.
pub fn translate(root: &Proto) -> Result<ScriptDef, IrError> {
    let mut unit = Unit {
        base: rk_base(root),
        fns: Vec::new(),
        ids: BTreeMap::new(),
        queue: VecDeque::new(),
        parent: Vec::new(),
        current: None,
    };
    unit.add_proto(root, "main".to_owned());
    while let Some((p, fid, name)) = unit.queue.pop_front() {
        unit.current = Some(fid);
        let def = FnTranslator::new(&mut unit, p, name)?.translate()?;
        unit.fns[fid as usize] = Some(def);
    }
    let mut fns: Vec<FnDef> = unit
        .fns
        .into_iter()
        .map(|f| f.expect("queued fn must be translated"))
        .collect();
    name_functions(&mut fns, &unit.parent);
    Ok(ScriptDef { main: 0, fns })
}

/// Parse and translate raw Lua 5.0 bytecode bytes into a [`ScriptDef`].
pub fn translate_bytes(body: &[u8]) -> Result<ScriptDef, IrError> {
    let root = super::undump(body)?;
    translate(&root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_mod_mission_scripts_translate_when_present() {
        let mods_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../dist/openbf_data/mods");
        if !mods_dir.is_dir() {
            return;
        }
        let mut total = 0usize;
        let mut failed = Vec::new();
        fn walk(dir: &std::path::Path, total: &mut usize, failed: &mut Vec<String>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return };
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, total, failed);
                } else if p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case("mission.lvl"))
                {
                    let Ok(bytes) = std::fs::read(&p) else { continue };
                    let Ok(scripts) = crate::lua50::extract_scripts(&bytes) else { continue };
                    for sc in scripts {
                        *total += 1;
                        if let Err(e) = translate_bytes(&sc.body) {
                            failed.push(format!("{}/{}: {e}", p.display(), sc.name));
                        }
                    }
                }
            }
        }
        walk(&mods_dir, &mut total, &mut failed);
        assert!(
            failed.is_empty(),
            "failed {}/{total} mod scripts:\n{}",
            failed.len(),
            failed.join("\n")
        );
        eprintln!("translated {total}/{total} installed mod mission scripts");
    }
}
