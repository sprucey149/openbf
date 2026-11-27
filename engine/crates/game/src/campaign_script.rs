//! The campaign action tree and its evaluator.
//!
//! `scripts/campaign_ir.py` translates each Rise of the Empire mission's PS2
//! bytecode, and the shared scripts it loads with `ScriptCB_DoFile`
//! (Objective, ObjectiveGoto, MultiObjectiveContainer, Ambush, ...), into a
//! closed tree of statements and expressions over the engine's script
//! bindings (`engine/missions/campaign/*.ron`, `campaign/lib/*.ron`). PLAN.md
//! keeps Lua out of the game ("data-driven missions, no Lua VM"): nothing here
//! reads bytecode; this walks the tree with the value rules the scripts were
//! written against (Lua 5.0.2, float numbers): nil/false are the only false
//! values, tables are shared by identity, `__index` metatables give the
//! framework its classes, a captured local is a cell shared with its closures
//! until its block closes, and a `...` function receives its extra arguments
//! as the `arg` table. Engine bindings are the [`Host`]'s.
//!
//! Every heap value lives in arenas owned by [`Interp`], so a clone is a deep
//! copy (`WalkSim::checkpoint`) and evaluation is deterministic: table
//! traversal is insertion order, there is no hashing of values.

use std::collections::HashMap;
use std::sync::Arc;

pub use manifest::script_ir::{BinOp, Expr, FnDef, IterKind, ScriptDef, Stmt};

/// An engine handle a binding returns (a character, a game object, a timer,
/// an event registration, ...). Lua compares userdata by identity; these
/// compare by value, which is the same thing for handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ud {
    Char(u32),
    Unit(u32, u32),
    Vehicle(u32, u32),
    Station(u16),
    Post(u16),
    Entity(u32),
    Flag(u8),
    Timer(u32),
    Handler(u32),
    Goal(u32),
    Region(u32),
    Point(u32, u32),
    Matrix(u32),
    Stream(u32),
    Class(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FuncId(u32);

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Num(f32),
    Str(Arc<str>),
    Table(TableId),
    Func(FuncId),
    Builtin(Arc<str>),
    Ud(Ud),
}

impl Value {
    pub fn truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    pub fn str(s: &str) -> Value {
        Value::Str(Arc::from(s))
    }

    pub fn num(&self) -> Option<f32> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Str(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "boolean",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::Table(_) => "table",
            Value::Func(_) | Value::Builtin(_) => "function",
            Value::Ud(_) => "userdata",
        }
    }
}

/// lua_number2str for the PS2's float numbers ("%.14g" over a float):
/// integers print without a fraction, which is what the scripts concatenate
/// (`"cp" .. i`).
pub fn num_text(n: f32) -> String {
    if n.fract() == 0.0 && n.abs() < 1.0e9 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[derive(Clone, Debug)]
pub struct ScriptError(pub String);

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(msg: impl Into<String>) -> Result<T, ScriptError> {
    Err(ScriptError(msg.into()))
}

pub type Ret = Result<Vec<Value>, ScriptError>;

/// The engine side: the bindings the scripts call by global name.
pub trait Host {
    /// A global the script reads but never assigned is an engine binding
    /// when this says so, else nil.
    fn known(&self, name: &str) -> bool;
    fn call(&mut self, it: &mut Interp, name: &str, args: &[Value]) -> Ret;
}

#[derive(Clone, Debug, Default)]
struct Table {
    entries: Vec<(Value, Value)>,
    meta: Option<TableId>,
}

#[derive(Clone, Debug)]
struct Closure {
    unit: u16,
    func: u32,
    upvals: Vec<u32>,
}

#[derive(Clone, Debug)]
enum Slot {
    V(Value),
    Cell(u32),
}

struct Frame {
    unit: u16,
    upvals: Vec<u32>,
    locals: Vec<Slot>,
}

enum Flow {
    Next,
    Break,
    Continue,
    Return(Vec<Value>),
}

/// Statements one top-level call may run before it is cut off: the scripts
/// have no unbounded loops, so reaching it means a translation fault, not
/// mission logic.
const STEP_BUDGET: u32 = 2_000_000;
const MAX_DEPTH: u32 = 120;

#[derive(Clone, Debug)]
pub struct Interp {
    units: Vec<Arc<ScriptDef>>,
    unit_names: Vec<String>,
    loaded: Vec<bool>,
    globals: HashMap<String, Value>,
    tables: Vec<Table>,
    closures: Vec<Closure>,
    cells: Vec<Value>,
    steps: u32,
    depth: u32,
    rng: core::rng::SimRng,
    pub printed: Vec<String>,
}

impl Interp {
    /// `main` is the mission; `libs` are the shared scripts by lower-case
    /// name, run on their first `ScriptCB_DoFile`.
    pub fn new(main: Arc<ScriptDef>, libs: &[(String, Arc<ScriptDef>)]) -> Self {
        let mut units = vec![main];
        let mut unit_names = vec![String::new()];
        for (name, def) in libs {
            units.push(def.clone());
            unit_names.push(name.to_ascii_lowercase());
        }
        let loaded = vec![false; units.len()];
        Self {
            units,
            unit_names,
            loaded,
            globals: HashMap::new(),
            tables: Vec::new(),
            closures: Vec::new(),
            cells: Vec::new(),
            steps: 0,
            depth: 0,
            rng: core::rng::SimRng::new(0x5EED_C0DE),
            printed: Vec::new(),
        }
    }

    /// Run the mission chunk (its global definitions).
    pub fn run_main(&mut self, host: &mut dyn Host) -> Ret {
        self.loaded[0] = true;
        let main = self.units[0].main;
        self.steps = 0;
        self.run_fn(host, 0, main, Vec::new(), Vec::new())
    }

    pub fn global(&self, name: &str) -> Value {
        self.globals.get(name).cloned().unwrap_or(Value::Nil)
    }

    pub fn set_global(&mut self, name: &str, v: Value) {
        if v == Value::Nil {
            self.globals.remove(name);
        } else {
            self.globals.insert(name.to_owned(), v);
        }
    }

    /// Call a script value from the engine (an event handler, a timer).
    pub fn call_value(&mut self, host: &mut dyn Host, f: &Value, args: Vec<Value>) -> Ret {
        if self.depth == 0 {
            self.steps = 0;
        }
        self.call(host, f, args)
    }

    // ---- tables

    pub fn new_table(&mut self) -> TableId {
        self.tables.push(Table::default());
        TableId(self.tables.len() as u32 - 1)
    }

    pub fn raw_get(&self, t: TableId, k: &Value) -> Value {
        self.tables[t.0 as usize]
            .entries
            .iter()
            .find(|(key, _)| key_eq(key, k))
            .map_or(Value::Nil, |(_, v)| v.clone())
    }

    pub fn raw_set(&mut self, t: TableId, k: Value, v: Value) {
        let entries = &mut self.tables[t.0 as usize].entries;
        if let Some(i) = entries.iter().position(|(key, _)| key_eq(key, &k)) {
            if v == Value::Nil {
                entries.remove(i);
            } else {
                entries[i].1 = v;
            }
        } else if v != Value::Nil {
            entries.push((k, v));
        }
    }

    pub fn get_field(&self, t: TableId, k: &str) -> Value {
        self.raw_get(t, &Value::str(k))
    }

    pub fn table_meta(&self, t: TableId) -> Option<TableId> {
        self.tables.get(t.0 as usize).and_then(|t| t.meta)
    }

    pub fn table_values(&self, t: TableId) -> Vec<Value> {
        self.tables
            .get(t.0 as usize)
            .map(|t| t.entries.iter().map(|(_, v)| v.clone()).collect())
            .unwrap_or_default()
    }

    /// The array part `t[1..n]`, n the first nil (lua 5.0 `table.getn`
    /// without an `n` field).
    pub fn array(&self, t: TableId) -> Vec<Value> {
        let mut out = Vec::new();
        loop {
            let v = self.raw_get(t, &Value::Num(out.len() as f32 + 1.0));
            if v == Value::Nil {
                return out;
            }
            out.push(v);
        }
    }

    fn getn(&self, t: TableId) -> usize {
        if let Some(n) = self.get_field(t, "n").num() {
            return n as usize;
        }
        self.array(t).len()
    }

    /// `t[k]` with `__index` (lvm.c luaV_gettable).
    pub fn index(
        &mut self,
        host: &mut dyn Host,
        obj: &Value,
        k: &Value,
    ) -> Result<Value, ScriptError> {
        let mut cur = obj.clone();
        for _ in 0..100 {
            match &cur {
                Value::Table(t) => {
                    let v = self.raw_get(*t, k);
                    if v != Value::Nil {
                        return Ok(v);
                    }
                    let Some(meta) = self.tables[t.0 as usize].meta else {
                        return Ok(Value::Nil);
                    };
                    let h = self.raw_get(meta, &Value::str("__index"));
                    match h {
                        Value::Nil => return Ok(Value::Nil),
                        Value::Func(_) | Value::Builtin(_) => {
                            let r = self.call(host, &h, vec![cur.clone(), k.clone()])?;
                            return Ok(r.into_iter().next().unwrap_or(Value::Nil));
                        }
                        other => cur = other,
                    }
                }
                Value::Builtin(name) => {
                    let Some(field) = k.as_str() else {
                        return err(format!("index {name} with {}", k.type_name()));
                    };
                    let full = format!("{name}.{field}");
                    return Ok(if is_lib_field(&full) || host.known(&full) {
                        Value::Builtin(Arc::from(full.as_str()))
                    } else {
                        Value::Nil
                    });
                }
                other => {
                    return err(format!(
                        "attempt to index a {} value (field {})",
                        other.type_name(),
                        self.show(k)
                    ))
                }
            }
        }
        err("__index chain too long")
    }

    fn set_index(&mut self, obj: &Value, k: Value, v: Value) -> Result<(), ScriptError> {
        match obj {
            Value::Table(t) => {
                if k == Value::Nil {
                    return err("table index is nil");
                }
                self.raw_set(*t, k, v);
                Ok(())
            }
            other => err(format!("attempt to index a {} value", other.type_name())),
        }
    }

    pub fn show(&self, v: &Value) -> String {
        match v {
            Value::Nil => "nil".into(),
            Value::Bool(b) => b.to_string(),
            Value::Num(n) => num_text(*n),
            Value::Str(s) => s.to_string(),
            Value::Table(t) => format!("table: {}", t.0),
            Value::Func(f) => format!("function: {}", f.0),
            Value::Builtin(n) => format!("function: {n}"),
            Value::Ud(u) => format!("userdata: {u:?}"),
        }
    }

    // ---- calls

    fn call(&mut self, host: &mut dyn Host, f: &Value, args: Vec<Value>) -> Ret {
        match f {
            Value::Func(id) => {
                let c = self.closures[id.0 as usize].clone();
                self.run_fn(host, c.unit, c.func, c.upvals, args)
            }
            Value::Builtin(name) => {
                let name = name.clone();
                self.builtin(host, &name, args)
            }
            other => err(format!("attempt to call a {} value", other.type_name())),
        }
    }

    fn run_fn(
        &mut self,
        host: &mut dyn Host,
        unit: u16,
        func: u32,
        upvals: Vec<u32>,
        args: Vec<Value>,
    ) -> Ret {
        if self.depth >= MAX_DEPTH {
            return err("stack overflow");
        }
        let def = self.units[usize::from(unit)].clone();
        let Some(fd) = def.fns.get(func as usize) else {
            return err(format!("no function {func}"));
        };
        let n = usize::from(fd.locals.max(fd.params + u16::from(fd.vararg)));
        let mut locals = vec![Slot::V(Value::Nil); n];
        let params = usize::from(fd.params);
        let mut extra = Vec::new();
        for (i, a) in args.into_iter().enumerate() {
            if i < params {
                locals[i] = Slot::V(a);
            } else {
                extra.push(a);
            }
        }
        if fd.vararg {
            let t = self.new_table();
            let count = extra.len();
            for (i, v) in extra.into_iter().enumerate() {
                self.raw_set(t, Value::Num(i as f32 + 1.0), v);
            }
            self.raw_set(t, Value::str("n"), Value::Num(count as f32));
            locals[params] = Slot::V(Value::Table(t));
        }
        let mut frame = Frame {
            unit,
            upvals,
            locals,
        };
        self.depth += 1;
        let r = self.block(host, &mut frame, &fd.body);
        self.depth -= 1;
        match r {
            Ok(Flow::Return(v)) => Ok(v),
            Ok(Flow::Next | Flow::Break | Flow::Continue) => Ok(Vec::new()),
            Err(e) => Err(ScriptError(format!("{}: {}", fd.name, e.0))),
        }
    }

    fn block(
        &mut self,
        host: &mut dyn Host,
        fr: &mut Frame,
        body: &[Stmt],
    ) -> Result<Flow, ScriptError> {
        for s in body {
            self.steps += 1;
            if self.steps > STEP_BUDGET {
                return err("step budget exhausted");
            }
            match self.stmt(host, fr, s)? {
                Flow::Next => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Next)
    }

    fn stmt(&mut self, host: &mut dyn Host, fr: &mut Frame, s: &Stmt) -> Result<Flow, ScriptError> {
        match s {
            Stmt::Do(e) => {
                self.multi(host, fr, e)?;
            }
            Stmt::SetLocal(r, e) => {
                let v = self.eval(host, fr, e)?;
                self.write_local(fr, *r, v);
            }
            Stmt::SetLocals(regs, e) => {
                let mut vals = self.multi(host, fr, e)?.into_iter();
                for r in regs {
                    let v = vals.next().unwrap_or(Value::Nil);
                    self.write_local(fr, *r, v);
                }
            }
            Stmt::SetGlobal(name, e) => {
                let v = self.eval(host, fr, e)?;
                self.set_global(name, v);
            }
            Stmt::SetUpval(i, e) => {
                let v = self.eval(host, fr, e)?;
                let Some(&cell) = fr.upvals.get(usize::from(*i)) else {
                    return err("bad upvalue");
                };
                self.cells[cell as usize] = v;
            }
            Stmt::SetIndex(t, k, v) => {
                let t = self.eval(host, fr, t)?;
                let k = self.eval(host, fr, k)?;
                let v = self.eval(host, fr, v)?;
                self.set_index(&t, k, v)?;
            }
            Stmt::Return(es) => {
                let vals = self.args(host, fr, es)?;
                return Ok(Flow::Return(vals));
            }
            Stmt::If(c, a, b) => {
                let arm = if self.eval(host, fr, c)?.truthy() {
                    a
                } else {
                    b
                };
                return self.block(host, fr, arm);
            }
            Stmt::ForNum(var, start, limit, step, body) => {
                let start = self.eval_num(host, fr, start, "'for' initial value")?;
                let limit = self.eval_num(host, fr, limit, "'for' limit")?;
                let step = self.eval_num(host, fr, step, "'for' step")?;
                // lvm.c OP_FORLOOP (5.0): the loop variable is the counter
                // itself, read back after each pass.
                let mut i = start;
                while if step > 0.0 { i <= limit } else { i >= limit } {
                    self.write_local(fr, *var, Value::Num(i));
                    match self.block(host, fr, body)? {
                        Flow::Next | Flow::Continue => {}
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                    }
                    i = self.read_local(fr, *var).num().unwrap_or(i) + step;
                }
            }
            Stmt::ForIn(kind, coll, vars, body) => {
                let coll = self.eval(host, fr, coll)?;
                let Value::Table(t) = coll else {
                    return err(format!("'for' over a {} value", coll.type_name()));
                };
                match kind {
                    IterKind::Ipairs => {
                        let mut i = 1.0f32;
                        loop {
                            let v = self.raw_get(t, &Value::Num(i));
                            if v == Value::Nil {
                                break;
                            }
                            self.bind_iter(fr, vars, Value::Num(i), v);
                            match self.block(host, fr, body)? {
                                Flow::Next | Flow::Continue => {}
                                Flow::Break => break,
                                Flow::Return(v) => return Ok(Flow::Return(v)),
                            }
                            i += 1.0;
                        }
                    }
                    IterKind::Pairs => {
                        let keys: Vec<Value> = self.tables[t.0 as usize]
                            .entries
                            .iter()
                            .map(|(k, _)| k.clone())
                            .collect();
                        for k in keys {
                            let v = self.raw_get(t, &k);
                            if v == Value::Nil {
                                continue;
                            }
                            self.bind_iter(fr, vars, k, v);
                            match self.block(host, fr, body)? {
                                Flow::Next | Flow::Continue => {}
                                Flow::Break => break,
                                Flow::Return(v) => return Ok(Flow::Return(v)),
                            }
                        }
                    }
                }
            }
            Stmt::Close(a) => {
                for slot in fr.locals.iter_mut().skip(usize::from(*a)) {
                    if let Slot::Cell(c) = slot {
                        *slot = Slot::V(self.cells[*c as usize].clone());
                    }
                }
            }
            Stmt::Loop(body) => loop {
                self.steps += 1;
                if self.steps > STEP_BUDGET {
                    return err("step budget exhausted");
                }
                match self.block(host, fr, body)? {
                    Flow::Next | Flow::Continue => {}
                    Flow::Break => break,
                    Flow::Return(v) => return Ok(Flow::Return(v)),
                }
            },
            Stmt::Break => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),
        }
        Ok(Flow::Next)
    }

    fn bind_iter(&mut self, fr: &mut Frame, vars: &[u16], k: Value, v: Value) {
        let mut vals = [k, v].into_iter();
        for r in vars {
            let x = vals.next().unwrap_or(Value::Nil);
            self.write_local(fr, *r, x);
        }
    }

    fn read_local(&self, fr: &Frame, r: u16) -> Value {
        match fr.locals.get(usize::from(r)) {
            Some(Slot::V(v)) => v.clone(),
            Some(Slot::Cell(c)) => self.cells[*c as usize].clone(),
            None => Value::Nil,
        }
    }

    fn write_local(&mut self, fr: &mut Frame, r: u16, v: Value) {
        let r = usize::from(r);
        if r >= fr.locals.len() {
            fr.locals.resize(r + 1, Slot::V(Value::Nil));
        }
        match &mut fr.locals[r] {
            Slot::V(x) => *x = v,
            Slot::Cell(c) => self.cells[*c as usize] = v,
        }
    }

    fn cell_of(&mut self, fr: &mut Frame, r: u16) -> u32 {
        let r = usize::from(r);
        if r >= fr.locals.len() {
            fr.locals.resize(r + 1, Slot::V(Value::Nil));
        }
        match &fr.locals[r] {
            Slot::Cell(c) => *c,
            Slot::V(v) => {
                self.cells.push(v.clone());
                let c = self.cells.len() as u32 - 1;
                fr.locals[r] = Slot::Cell(c);
                c
            }
        }
    }

    fn eval_num(
        &mut self,
        host: &mut dyn Host,
        fr: &mut Frame,
        e: &Expr,
        what: &str,
    ) -> Result<f32, ScriptError> {
        let v = self.eval(host, fr, e)?;
        v.num()
            .ok_or_else(|| ScriptError(format!("{what} must be a number")))
    }

    /// Argument lists: a trailing call passes on all its results (an open
    /// CALL in the bytecode).
    fn args(&mut self, host: &mut dyn Host, fr: &mut Frame, es: &[Expr]) -> Ret {
        let mut out = Vec::with_capacity(es.len());
        for (i, e) in es.iter().enumerate() {
            if i + 1 == es.len() && matches!(e, Expr::Call(..) | Expr::Method(..)) {
                out.extend(self.multi(host, fr, e)?);
            } else {
                out.push(self.eval(host, fr, e)?);
            }
        }
        Ok(out)
    }

    fn multi(&mut self, host: &mut dyn Host, fr: &mut Frame, e: &Expr) -> Ret {
        match e {
            Expr::Call(f, args) => {
                let f = self.eval(host, fr, f)?;
                let args = self.args(host, fr, args)?;
                self.call(host, &f, args)
            }
            Expr::Method(obj, name, args) => {
                let o = self.eval(host, fr, obj)?;
                let f = self.index(host, &o, &Value::str(name))?;
                if f == Value::Nil {
                    return err(format!("method '{name}' is nil"));
                }
                let mut a = vec![o];
                a.extend(self.args(host, fr, args)?);
                self.call(host, &f, a)
            }
            other => Ok(vec![self.eval(host, fr, other)?]),
        }
    }

    fn eval(
        &mut self,
        host: &mut dyn Host,
        fr: &mut Frame,
        e: &Expr,
    ) -> Result<Value, ScriptError> {
        Ok(match e {
            Expr::Nil => Value::Nil,
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::str(s),
            Expr::Local(r) => self.read_local(fr, *r),
            Expr::Upval(i) => match fr.upvals.get(usize::from(*i)) {
                Some(&c) => self.cells[c as usize].clone(),
                None => return err("bad upvalue"),
            },
            Expr::Global(name) => match self.globals.get(name.as_str()) {
                Some(v) => v.clone(),
                None if is_lib(name) || host.known(name) => {
                    Value::Builtin(Arc::from(name.as_str()))
                }
                None => Value::Nil,
            },
            Expr::Index(t, k) => {
                let t = self.eval(host, fr, t)?;
                let k = self.eval(host, fr, k)?;
                self.index(host, &t, &k)?
            }
            Expr::Call(..) | Expr::Method(..) => self
                .multi(host, fr, e)?
                .into_iter()
                .next()
                .unwrap_or(Value::Nil),
            Expr::Bin(op, a, b) => {
                let a = self.eval(host, fr, a)?;
                let b = self.eval(host, fr, b)?;
                self.bin(*op, &a, &b)?
            }
            Expr::Not(a) => Value::Bool(!self.eval(host, fr, a)?.truthy()),
            Expr::Neg(a) => {
                let v = self.eval(host, fr, a)?;
                Value::Num(
                    -v.num()
                        .ok_or_else(|| ScriptError("unary minus on a non-number".into()))?,
                )
            }
            Expr::Closure(func, caps) => {
                let mut upvals = Vec::with_capacity(caps.len());
                for c in caps {
                    match c {
                        Expr::Local(r) => upvals.push(self.cell_of(fr, *r)),
                        Expr::Upval(i) => match fr.upvals.get(usize::from(*i)) {
                            Some(&c) => upvals.push(c),
                            None => return err("bad upvalue capture"),
                        },
                        _ => return err("bad capture"),
                    }
                }
                self.closures.push(Closure {
                    unit: fr.unit,
                    func: *func,
                    upvals,
                });
                Value::Func(FuncId(self.closures.len() as u32 - 1))
            }
            Expr::Table(items) => {
                let t = self.new_table();
                for (k, v) in items {
                    let k = self.eval(host, fr, k)?;
                    let v = self.eval(host, fr, v)?;
                    if k != Value::Nil {
                        self.raw_set(t, k, v);
                    }
                }
                Value::Table(t)
            }
        })
    }

    fn bin(&self, op: BinOp, a: &Value, b: &Value) -> Result<Value, ScriptError> {
        let arith = |f: fn(f32, f32) -> f32| -> Result<Value, ScriptError> {
            match (a.num(), b.num()) {
                (Some(x), Some(y)) => Ok(Value::Num(f(x, y))),
                _ => err(format!(
                    "attempt to perform arithmetic on a {} value",
                    if a.num().is_none() {
                        a.type_name()
                    } else {
                        b.type_name()
                    }
                )),
            }
        };
        match op {
            BinOp::Add => arith(|x, y| x + y),
            BinOp::Sub => arith(|x, y| x - y),
            BinOp::Mul => arith(|x, y| x * y),
            BinOp::Div => arith(|x, y| x / y),
            BinOp::Pow => arith(core::fmath::powf),
            BinOp::Concat => {
                let part = |v: &Value| match v {
                    Value::Str(s) => Ok(s.to_string()),
                    Value::Num(n) => Ok(num_text(*n)),
                    other => err(format!(
                        "attempt to concatenate a {} value",
                        other.type_name()
                    )),
                };
                Ok(Value::str(&(part(a)? + &part(b)?)))
            }
            BinOp::Eq => Ok(Value::Bool(raw_eq(a, b))),
            BinOp::Lt | BinOp::Le => {
                let r = match (a, b) {
                    (Value::Num(x), Value::Num(y)) => {
                        if op == BinOp::Lt {
                            x < y
                        } else {
                            x <= y
                        }
                    }
                    (Value::Str(x), Value::Str(y)) => {
                        if op == BinOp::Lt {
                            x < y
                        } else {
                            x <= y
                        }
                    }
                    _ => {
                        return err(format!(
                            "attempt to compare {} with {}",
                            a.type_name(),
                            b.type_name()
                        ))
                    }
                };
                Ok(Value::Bool(r))
            }
        }
    }

    // ---- the standard library the scripts use, and ScriptCB_DoFile

    fn builtin(&mut self, host: &mut dyn Host, name: &str, args: Vec<Value>) -> Ret {
        let arg = |i: usize| args.get(i).cloned().unwrap_or(Value::Nil);
        Ok(match name {
            "print" => {
                let line: Vec<String> = args.iter().map(|v| self.show(v)).collect();
                self.printed.push(line.join("\t"));
                vec![]
            }
            "type" => vec![Value::str(arg(0).type_name())],
            "tostring" => vec![Value::str(&self.show(&arg(0)))],
            "tonumber" => vec![arg(0).num().map_or(Value::Nil, Value::Num)],
            "assert" => {
                if !arg(0).truthy() {
                    let msg = arg(1);
                    return err(format!("assertion failed: {}", self.show(&msg)));
                }
                args
            }
            "setmetatable" => {
                let Value::Table(t) = arg(0) else {
                    return err("setmetatable on a non-table");
                };
                self.tables[t.0 as usize].meta = match arg(1) {
                    Value::Table(m) => Some(m),
                    _ => None,
                };
                vec![arg(0)]
            }
            "getmetatable" => match arg(0) {
                Value::Table(t) => vec![self.tables[t.0 as usize]
                    .meta
                    .map_or(Value::Nil, Value::Table)],
                _ => vec![Value::Nil],
            },
            "table.insert" | "tinsert" => {
                let Value::Table(t) = arg(0) else {
                    return err("table.insert on a non-table");
                };
                let n = self.getn(t);
                if args.len() >= 3 {
                    let pos = arg(1).num().unwrap_or(n as f32 + 1.0) as usize;
                    let v = arg(2);
                    for i in (pos..=n).rev() {
                        let x = self.raw_get(t, &Value::Num(i as f32));
                        self.raw_set(t, Value::Num(i as f32 + 1.0), x);
                    }
                    self.raw_set(t, Value::Num(pos as f32), v);
                } else {
                    self.raw_set(t, Value::Num(n as f32 + 1.0), arg(1));
                }
                if self.get_field(t, "n") != Value::Nil {
                    self.raw_set(t, Value::str("n"), Value::Num(n as f32 + 1.0));
                }
                vec![]
            }
            "table.remove" | "tremove" => {
                let Value::Table(t) = arg(0) else {
                    return err("table.remove on a non-table");
                };
                let n = self.getn(t);
                if n == 0 {
                    return Ok(vec![Value::Nil]);
                }
                let pos = arg(1).num().map_or(n, |p| p as usize);
                let out = self.raw_get(t, &Value::Num(pos as f32));
                for i in pos..n {
                    let x = self.raw_get(t, &Value::Num(i as f32 + 1.0));
                    self.raw_set(t, Value::Num(i as f32), x);
                }
                self.raw_set(t, Value::Num(n as f32), Value::Nil);
                vec![out]
            }
            "table.getn" | "getn" => match arg(0) {
                Value::Table(t) => vec![Value::Num(self.getn(t) as f32)],
                _ => return err("table.getn on a non-table"),
            },
            "string.lower" | "string.upper" | "string.len" | "string.sub" => {
                let s = match arg(0) {
                    Value::Str(s) => s.to_string(),
                    Value::Num(n) => num_text(n),
                    other => return err(format!("{name} on a {} value", other.type_name())),
                };
                match name {
                    "string.lower" => vec![Value::str(&s.to_ascii_lowercase())],
                    "string.upper" => vec![Value::str(&s.to_ascii_uppercase())],
                    "string.len" => vec![Value::Num(s.len() as f32)],
                    _ => {
                        let len = s.len() as i64;
                        let fix = |i: i64| {
                            if i < 0 {
                                (len + i + 1).max(1)
                            } else {
                                i.max(1)
                            }
                        };
                        let i = fix(arg(1).num().unwrap_or(1.0) as i64);
                        let j = arg(2).num().map_or(len, |j| {
                            let j = j as i64;
                            if j < 0 {
                                len + j + 1
                            } else {
                                j.min(len)
                            }
                        });
                        let out = if i > j {
                            String::new()
                        } else {
                            s.get((i - 1) as usize..j as usize).unwrap_or("").to_owned()
                        };
                        vec![Value::str(&out)]
                    }
                }
            }
            "math.floor" | "math.ceil" | "math.abs" | "math.sqrt" | "math.sin" | "math.cos"
            | "math.tan" => {
                let x = arg(0)
                    .num()
                    .ok_or_else(|| ScriptError(format!("{name}: number expected")))?;
                vec![Value::Num(match name {
                    "math.floor" => x.floor(),
                    "math.ceil" => x.ceil(),
                    "math.abs" => x.abs(),
                    "math.sin" => core::fmath::sin_cos(x).0,
                    "math.cos" => core::fmath::sin_cos(x).1,
                    "math.tan" => {
                        let (s, c) = core::fmath::sin_cos(x);
                        if c == 0.0 { 0.0 } else { s / c }
                    }
                    _ => x.sqrt(),
                })]
            }
            "math.mod" => {
                let a = arg(0)
                    .num()
                    .ok_or_else(|| ScriptError("math.mod: number expected".into()))?;
                let b = arg(1)
                    .num()
                    .ok_or_else(|| ScriptError("math.mod: number expected".into()))?;
                vec![Value::Num(if b == 0.0 {
                    0.0
                } else {
                    a - (a / b).trunc() * b
                })]
            }
            "math.random" | "random" => {
                let r = self.rng.next_rand() as u64;
                let v = match args.len() {
                    0 => (r as f32) / (u32::MAX as f32 + 1.0),
                    1 => {
                        let u = arg(0).num().unwrap_or(1.0).floor() as i64;
                        if u < 1 {
                            1.0
                        } else {
                            ((r % (u as u64)) + 1) as f32
                        }
                    }
                    _ => {
                        let l = arg(0).num().unwrap_or(1.0).floor() as i64;
                        let u = arg(1).num().unwrap_or(l as f32).floor() as i64;
                        if u <= l {
                            l as f32
                        } else {
                            let span = (u - l + 1) as u64;
                            (l + (r % span) as i64) as f32
                        }
                    }
                };
                vec![Value::Num(v)]
            }
            "math.randomseed" => {
                let seed = arg(0).num().unwrap_or(1.0) as u32;
                self.rng = core::rng::SimRng::new(seed);
                vec![]
            }
            "string.find" => {
                let s = self.show(&arg(0));
                let pat = self.show(&arg(1));
                let init = arg(2).num().map_or(1isize, |n| n.trunc() as isize);
                let start = if init < 0 {
                    (s.len() as isize + init).max(0) as usize
                } else {
                    init.saturating_sub(1).max(0) as usize
                };
                if start <= s.len() {
                    if let Some(pos) = s[start..].find(&pat) {
                        let a = start + pos + 1;
                        let b = a + pat.len().saturating_sub(1);
                        vec![Value::Num(a as f32), Value::Num(b as f32)]
                    } else {
                        vec![Value::Nil]
                    }
                } else {
                    vec![Value::Nil]
                }
            }
            "string.format" => {
                let fmt = self.show(&arg(0));
                let mut out = String::new();
                let mut chars = fmt.chars().peekable();
                let mut ai = 1usize;
                while let Some(ch) = chars.next() {
                    if ch != '%' {
                        out.push(ch);
                        continue;
                    }
                    match chars.next() {
                        Some('%') => out.push('%'),
                        Some('s') => {
                            out.push_str(&self.show(&arg(ai)));
                            ai += 1;
                        }
                        Some('d' | 'i') => {
                            let n = arg(ai).num().unwrap_or(0.0).trunc() as i64;
                            out.push_str(&n.to_string());
                            ai += 1;
                        }
                        Some('f') => {
                            let n = arg(ai).num().unwrap_or(0.0);
                            out.push_str(&format!("{n:.6}"));
                            ai += 1;
                        }
                        Some(other) => {
                            out.push('%');
                            out.push(other);
                        }
                        None => out.push('%'),
                    }
                }
                vec![Value::str(&out)]
            }
            "math.min" | "math.max" => {
                let mut best: Option<f32> = None;
                for v in &args {
                    let x = v
                        .num()
                        .ok_or_else(|| ScriptError(format!("{name}: number expected")))?;
                    best = Some(match best {
                        None => x,
                        Some(b) if name == "math.min" => b.min(x),
                        Some(b) => b.max(x),
                    });
                }
                vec![best.map_or(Value::Nil, Value::Num)]
            }
            "ScriptCB_DoFile" => {
                let file = arg(0).as_str().unwrap_or("").to_ascii_lowercase();
                if let Some(u) = self.unit_names.iter().position(|n| *n == file) {
                    if !self.loaded[u] {
                        self.loaded[u] = true;
                        let main = self.units[u].main;
                        self.run_fn(host, u as u16, main, Vec::new(), Vec::new())?;
                    }
                }
                vec![]
            }
            _ => return host.call(self, name, &args),
        })
    }
}

fn is_lib(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "type"
            | "tostring"
            | "tonumber"
            | "assert"
            | "setmetatable"
            | "getmetatable"
            | "table"
            | "string"
            | "math"
            | "random"
            | "getn"
            | "tinsert"
            | "tremove"
            | "ScriptCB_DoFile"
    )
}

/// The names [`Interp`] answers itself under `table.`, `string.`, `math.`.
pub fn is_lib_field(full: &str) -> bool {
    matches!(
        full,
        "table.insert"
            | "table.remove"
            | "table.getn"
            | "string.lower"
            | "string.upper"
            | "string.len"
            | "string.sub"
            | "string.find"
            | "string.format"
            | "math.floor"
            | "math.ceil"
            | "math.abs"
            | "math.sqrt"
            | "math.min"
            | "math.max"
            | "math.mod"
            | "math.sin"
            | "math.cos"
            | "math.tan"
            | "math.random"
            | "math.randomseed"
    )
}

fn raw_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x == y,
        _ => a == b,
    }
}

fn key_eq(a: &Value, b: &Value) -> bool {
    raw_eq(a, b)
}

/// Read a translated script (`engine/missions/campaign/lib/*.ron`).
pub fn parse(text: &str) -> Result<ScriptDef, ron::error::SpannedError> {
    ron::Options::default()
        .without_recursion_limit()
        .from_str(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost(Vec<String>);

    impl Host for NoHost {
        fn known(&self, name: &str) -> bool {
            is_lib_field(name) || name == "Probe"
        }
        fn call(&mut self, it: &mut Interp, name: &str, args: &[Value]) -> Ret {
            let shown: Vec<String> = args.iter().map(|v| it.show(v)).collect();
            self.0.push(format!("{name}({})", shown.join(",")));
            Ok(vec![])
        }
    }

    fn run(text: &str) -> (Interp, Vec<String>) {
        let def = Arc::new(parse(text).expect("parse"));
        let mut it = Interp::new(def, &[]);
        let mut host = NoHost(Vec::new());
        it.run_main(&mut host).expect("run");
        (it, host.0)
    }

    #[test]
    fn closures_share_cells_until_close() {
        let (_, calls) = run(r#"(main: 0, fns: [
                (name: "main", params: 0, locals: 4, body: [
                    SetLocal(0, Table([])),
                    ForNum(1, Num(1.0), Num(3.0), Num(1.0), [
                        SetLocal(2, Local(1)),
                        SetIndex(Local(0), Local(1), Closure(1, [Local(2)])),
                        Close(2),
                    ]),
                    ForIn(Ipairs, Local(0), [1, 2], [
                        Do(Call(Local(2), [])),
                    ]),
                ]),
                (name: "f", params: 0, locals: 1, body: [
                    Do(Call(Global("Probe"), [Upval(0)])),
                ]),
            ])"#);
        assert_eq!(calls, ["Probe(1)", "Probe(2)", "Probe(3)"]);
    }

    #[test]
    fn index_follows_metatables_and_varargs_pack() {
        let (it, calls) = run(r#"(main: 0, fns: [
                (name: "main", params: 0, locals: 3, body: [
                    SetGlobal("Base", Table([(Str("x"), Num(7.0))])),
                    SetIndex(Global("Base"), Str("__index"), Global("Base")),
                    SetLocal(0, Call(Global("setmetatable"), [Table([]), Global("Base")])),
                    Do(Call(Global("Probe"), [Index(Local(0), Str("x"))])),
                    Do(Call(Closure(1, []), [Num(1.0), Num(2.0), Str("a")])),
                ]),
                (name: "v", params: 1, vararg: true, locals: 2, body: [
                    Do(Call(Global("Probe"), [Index(Local(1), Str("n")), Index(Local(1), Num(2.0)),
                        Bin(Concat, Str("cp"), Local(0))])),
                ]),
            ])"#);
        assert_eq!(calls, ["Probe(7)", "Probe(2,a,cp1)"]);
        assert_eq!(it.global("Missing"), Value::Nil);
    }

    #[test]
    fn every_campaign_script_parses() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions/campaign/lib");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            eprintln!("SKIP: generated campaign missions required");
            return;
        };
        for e in entries.flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap();
            parse(&text).unwrap_or_else(|err| panic!("{}: {err}", e.path().display()));
        }
    }
}
