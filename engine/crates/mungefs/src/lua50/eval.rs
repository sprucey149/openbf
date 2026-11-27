use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{
    rk_base, LuaConst, Op50, Proto, MAX_CALL_DEPTH, MAX_EVAL_STEPS, MAX_STRING_BYTES, MAX_TABLES,
    MAX_TABLE_ENTRIES,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LuaKey {
    Str(String),
    Int(i64),
    Other(String),
}

impl LuaKey {
    pub fn from_val(v: &LuaVal) -> Option<Self> {
        match v {
            LuaVal::Str(s) => Some(Self::Str(s.clone())),
            LuaVal::Num(n) if n.is_finite() => {
                let rounded = n.round();
                if (*n - rounded).abs() < 1e-9
                    && rounded >= i64::MIN as f64
                    && rounded <= i64::MAX as f64
                {
                    Some(Self::Int(rounded as i64))
                } else {
                    Some(Self::Other(format!("{n}")))
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LuaVal {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
    Table(usize),
    Global(String),
    Method(String, String),
    Object(String),
    Builtin(String),
    Effect(String),
    MatrixObject(String),
    MatrixPath(String, usize),
    Closure { path: Vec<usize>, ups: Vec<LuaVal> },
    Unknown(String),
}

impl LuaVal {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Num(n) if n.is_finite() => Some(*n),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        let n = self.as_f64()?;
        let r = n.round();
        if (n - r).abs() < 1e-6 && r >= i64::MIN as f64 && r <= i64::MAX as f64 {
            Some(r as i64)
        } else {
            None
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::Num(n) => Some(*n != 0.0),
            _ => None,
        }
    }

    pub fn is_truthy(&self) -> Option<bool> {
        match self {
            Self::Nil => Some(false),
            Self::Bool(b) => Some(*b),
            Self::Num(n) => Some(*n != 0.0),
            Self::Str(_) | Self::Table(_) | Self::Closure { .. } => Some(true),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LuaTable {
    pub entries: Vec<(LuaKey, LuaVal)>,
}

impl LuaTable {
    pub fn set(&mut self, key: LuaKey, val: LuaVal) {
        if let Some((_, existing)) = self.entries.iter_mut().find(|(k, _)| k == &key) {
            *existing = val;
        } else if self.entries.len() < MAX_TABLE_ENTRIES {
            self.entries.push((key, val));
        }
    }

    pub fn get(&self, key: &LuaKey) -> Option<&LuaVal> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_str(&self, key: &str) -> Option<&LuaVal> {
        self.get(&LuaKey::Str(key.to_owned()))
    }

    pub fn get_str_ci(&self, key: &str) -> Option<&LuaVal> {
        self.entries.iter().find_map(|(k, v)| match k {
            LuaKey::Str(s) if s.eq_ignore_ascii_case(key) => Some(v),
            _ => None,
        })
    }

    pub fn get_int(&self, idx: i64) -> Option<&LuaVal> {
        self.get(&LuaKey::Int(idx))
    }

    pub fn array_len(&self) -> usize {
        let mut n = 0usize;
        while self.get_int((n + 1) as i64).is_some() {
            n += 1;
        }
        n
    }

    pub fn array_items(&self) -> Vec<&LuaVal> {
        let mut items: Vec<(i64, &LuaVal)> = self
            .entries
            .iter()
            .filter_map(|(k, v)| match k {
                LuaKey::Int(i) if *i >= 1 => Some((*i, v)),
                _ => None,
            })
            .collect();
        items.sort_by_key(|(i, _)| *i);
        items.into_iter().map(|(_, v)| v).collect()
    }
}

#[derive(Debug, Clone)]
pub struct LuaCall {
    pub func: String,
    pub args: Vec<LuaVal>,
}

/// Callback / dynamic feature detected statically during analysis that cannot be
/// translated into declarative OpenBF configuration automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedScriptFeature {
    pub api: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct EvalEnv {
    pub root: Proto,
    pub base: usize,
    pub globals: BTreeMap<String, LuaVal>,
    pub tables: Vec<LuaTable>,
    pub calls: Vec<LuaCall>,
    pub sp_listbox_id: usize,
    pub mp_listbox_id: usize,
    pub steps: usize,
    pub unsupported: Vec<UnsupportedScriptFeature>,
    pub warnings: Vec<String>,
    trace: Option<StandaloneTrace>,
}

/// Evaluation inputs for the standalone environment. Random
/// choices are enumerated by the importer, never made by the analyzer.
#[derive(Debug, Clone, Default)]
struct StandaloneTrace {
    multiplayer: bool,
    random_pick: Option<i64>,
    random_range: Option<(i64, i64)>,
    scripts: Arc<BTreeMap<String, Proto>>,
    loaded_scripts: BTreeSet<String>,
}

impl EvalEnv {
    /// The value the run's `math.random` calls took, when the script made a
    /// bounded one: the enumerated pick, else the first range's low end.
    pub fn script_pick(&self) -> Option<i64> {
        let trace = self.trace.as_ref()?;
        let (lo, hi) = trace.random_range?;
        Some(
            trace
                .random_pick
                .filter(|n| (lo..=hi).contains(n))
                .unwrap_or(lo),
        )
    }

    pub fn table(&self, id: usize) -> Option<&LuaTable> {
        self.tables.get(id)
    }

    fn alloc_table(&mut self) -> Option<usize> {
        if self.tables.len() >= MAX_TABLES {
            self.warnings
                .push("Lua static analyzer table limit reached".into());
            return None;
        }
        let id = self.tables.len();
        self.tables.push(LuaTable::default());
        Some(id)
    }

    fn proto_at<'a>(&'a self, path: &[usize]) -> Option<&'a Proto> {
        let mut cur = &self.root;
        for &idx in path {
            cur = cur.protos.get(idx)?;
        }
        Some(cur)
    }

    fn record_unsupported(&mut self, api: &str, detail: String) {
        if self
            .unsupported
            .iter()
            .any(|u| u.api == api && u.detail == detail)
        {
            return;
        }
        self.unsupported.push(UnsupportedScriptFeature {
            api: api.to_owned(),
            detail,
        });
    }

    fn check_dynamic_call(&mut self, func: &str, args: &[LuaVal]) {
        const DYNAMIC_APIS: &[&str] = &[
            "OnCharacterDeath",
            "OnCharacterDeathName",
            "OnCharacterDeathTeam",
            "OnCharacterSpawn",
            "OnCharacterSpawnName",
            "OnCharacterSpawnTeam",
            "OnObjectKill",
            "OnObjectKillName",
            "OnObjectRespawn",
            "OnObjectRespawnName",
            "OnObjectDamageName",
            "OnEnterRegion",
            "OnLeaveRegion",
            "OnTimerElapse",
            "CreateTimer",
            "StartTimer",
            "AddAIGoal",
            "SetPropertyTimer",
        ];
        if DYNAMIC_APIS
            .iter()
            .any(|&api| api.eq_ignore_ascii_case(func))
        {
            let summary = args
                .first()
                .and_then(|v| match v {
                    LuaVal::Str(s) => Some(s.clone()),
                    LuaVal::Global(g) => Some(g.clone()),
                    LuaVal::Num(n) => Some(format!("{n}")),
                    _ => None,
                })
                .unwrap_or_else(|| "...".into());
            self.record_unsupported(func, format!("{func}({summary})"));
        } else if func.eq_ignore_ascii_case("ScriptCB_DoFile") {
            let target = args.first().and_then(|v| v.as_str()).unwrap_or("");
            if !is_known_standard_dofile(target) && !target.is_empty() {
                self.record_unsupported(func, format!("ScriptCB_DoFile(\"{target}\")"));
            }
        }
    }
}

fn is_known_standard_dofile(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "setup_teams"
        || lower.starts_with("objective")
        || lower.starts_with("gametype_")
        || lower.starts_with("soundevent_")
        || lower.starts_with("linked")
        || lower == "ambush"
        || lower == "playmoviewithtransition"
}

fn new_env(proto: Proto) -> EvalEnv {
    let base = rk_base(&proto);
    let mut env = EvalEnv {
        root: proto,
        base,
        globals: BTreeMap::new(),
        tables: Vec::new(),
        calls: Vec::new(),
        sp_listbox_id: 0,
        mp_listbox_id: 1,
        steps: 0,
        unsupported: Vec::new(),
        warnings: Vec::new(),
        trace: None,
    };
    let sp = env.alloc_table().unwrap();
    let mp = env.alloc_table().unwrap();
    let tbl_lib = env.alloc_table().unwrap();
    let str_lib = env.alloc_table().unwrap();
    env.sp_listbox_id = sp;
    env.mp_listbox_id = mp;

    if let Some(t) = env.tables.get_mut(tbl_lib) {
        t.set(
            LuaKey::Str("getn".into()),
            LuaVal::Builtin("table.getn".into()),
        );
        t.set(
            LuaKey::Str("insert".into()),
            LuaVal::Builtin("table.insert".into()),
        );
    }
    if let Some(t) = env.tables.get_mut(str_lib) {
        t.set(
            LuaKey::Str("lower".into()),
            LuaVal::Builtin("string.lower".into()),
        );
        t.set(
            LuaKey::Str("format".into()),
            LuaVal::Builtin("string.format".into()),
        );
    }

    env.globals.insert(
        "sp_missionselect_listbox_contents".into(),
        LuaVal::Table(sp),
    );
    env.globals.insert(
        "mp_missionselect_listbox_contents".into(),
        LuaVal::Table(mp),
    );
    env.globals.insert("table".into(), LuaVal::Table(tbl_lib));
    env.globals.insert("string".into(), LuaVal::Table(str_lib));
    env.globals
        .insert("getn".into(), LuaVal::Builtin("table.getn".into()));
    env.globals
        .insert("strlower".into(), LuaVal::Builtin("string.lower".into()));
    env.globals
        .insert("format".into(), LuaVal::Builtin("string.format".into()));
    env
}

fn key_to_val(key: LuaKey) -> LuaVal {
    match key {
        LuaKey::Str(s) => LuaVal::Str(s),
        LuaKey::Int(i) => LuaVal::Num(i as f64),
        LuaKey::Other(s) => s.parse().map_or_else(|_| LuaVal::Unknown(s), LuaVal::Num),
    }
}

/// Lua's `type`, for the values whose type is known.
fn type_name(value: &LuaVal) -> Option<LuaVal> {
    let name = match value {
        LuaVal::Nil => "nil",
        LuaVal::Bool(_) => "boolean",
        LuaVal::Num(_) => "number",
        LuaVal::Str(_) => "string",
        LuaVal::Table(_) => "table",
        LuaVal::Closure { .. } => "function",
        _ => return None,
    };
    Some(LuaVal::Str(name.to_owned()))
}

fn jump_target(next: usize, bx: usize, end: usize) -> usize {
    next.checked_add_signed(bx as isize - 131_071)
        .filter(|&target| target < end)
        .unwrap_or(end)
}

fn comparison(op: Op50, lhs: &LuaVal, rhs: &LuaVal) -> Option<bool> {
    match (lhs, rhs) {
        (LuaVal::Global(_) | LuaVal::Unknown(_), _)
        | (_, LuaVal::Global(_) | LuaVal::Unknown(_)) => None,
        _ if op == Op50::Eq => Some(lhs == rhs),
        (LuaVal::Num(l), LuaVal::Num(r)) => Some(if op == Op50::Lt { l < r } else { l <= r }),
        (LuaVal::Str(l), LuaVal::Str(r)) => Some(if op == Op50::Lt { l < r } else { l <= r }),
        _ => None,
    }
}

fn const_to_val(c: &LuaConst) -> LuaVal {
    match c {
        LuaConst::Nil => LuaVal::Nil,
        LuaConst::Bool(b) => LuaVal::Bool(*b),
        LuaConst::Num(n) => LuaVal::Num(*n),
        LuaConst::Str(s) => LuaVal::Str(s.clone()),
    }
}

fn eval_proto(
    env: &mut EvalEnv,
    proto_path: &[usize],
    args: &[LuaVal],
    ups: &[LuaVal],
    call_local_closures: bool,
    depth: usize,
) {
    if depth > MAX_CALL_DEPTH {
        return;
    }
    let Some(proto) = env.proto_at(proto_path).cloned() else {
        return;
    };
    let mut regs: BTreeMap<usize, LuaVal> = BTreeMap::new();
    for (i, v) in args.iter().enumerate() {
        regs.insert(i, v.clone());
    }
    let exact = env.trace.is_some();
    // The next entry of each `for ... in` over a known table, by loop register.
    let mut cursors: BTreeMap<usize, usize> = BTreeMap::new();
    let mut pc = 0;
    let mut open_top = None;
    while let Some(&ins) = proto.code.get(pc) {
        let idx = pc;
        pc += 1;
        env.steps += 1;
        if env.steps > MAX_EVAL_STEPS {
            env.warnings
                .push("Lua static analyzer step budget reached".into());
            return;
        }
        let Ok(op) = Op50::from_u32(ins) else {
            return;
        };
        let a = ((ins >> 24) & 0xFF) as usize;
        let b = ((ins >> 15) & 0x1FF) as usize;
        let c = ((ins >> 6) & 0x1FF) as usize;
        let bx = ((ins >> 6) & 0x3FFFF) as usize;

        let rk = |x: usize, r: &BTreeMap<usize, LuaVal>| -> LuaVal {
            if x >= env.base {
                proto
                    .constants
                    .get(x - env.base)
                    .map(const_to_val)
                    .unwrap_or_else(|| LuaVal::Unknown(format!("k{}", x - env.base)))
            } else {
                r.get(&x)
                    .cloned()
                    .unwrap_or_else(|| LuaVal::Unknown(format!("r{x}")))
            }
        };

        match op {
            Op50::LoadK => {
                let v = proto
                    .constants
                    .get(bx)
                    .map(const_to_val)
                    .unwrap_or_else(|| LuaVal::Unknown(format!("k{bx}")));
                regs.insert(a, v);
            }
            Op50::Move => {
                let v = regs
                    .get(&b)
                    .cloned()
                    .unwrap_or_else(|| LuaVal::Unknown(format!("r{b}")));
                regs.insert(a, v);
            }
            Op50::LoadNil => {
                for slot in a..=b {
                    regs.insert(slot, LuaVal::Nil);
                }
            }
            Op50::LoadBool => {
                regs.insert(a, LuaVal::Bool(b != 0));
                if exact && c != 0 {
                    pc += 1;
                }
            }
            Op50::GetGlobal => {
                if let Some(LuaConst::Str(name)) = proto.constants.get(bx) {
                    let val = env
                        .globals
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| LuaVal::Global(name.clone()));
                    regs.insert(a, val);
                }
            }
            Op50::SetGlobal => {
                if let Some(LuaConst::Str(name)) = proto.constants.get(bx) {
                    let val = regs
                        .get(&a)
                        .cloned()
                        .unwrap_or_else(|| LuaVal::Unknown(format!("r{a}")));
                    env.globals.insert(name.clone(), val);
                }
            }
            Op50::GetUpval => {
                let val = ups
                    .get(b)
                    .cloned()
                    .unwrap_or_else(|| LuaVal::Unknown(format!("upval{b}")));
                regs.insert(a, val);
            }
            Op50::NewTable => {
                let v = env
                    .alloc_table()
                    .map(LuaVal::Table)
                    .unwrap_or_else(|| LuaVal::Unknown("table".into()));
                regs.insert(a, v);
            }
            Op50::SetTable => {
                if let Some(LuaVal::Table(tid)) = regs.get(&a).cloned() {
                    let k = rk(b, &regs);
                    let v = rk(c, &regs);
                    if let Some(key) = LuaKey::from_val(&k) {
                        if let Some(tbl) = env.tables.get_mut(tid) {
                            tbl.set(key, v);
                        }
                    }
                }
            }
            Op50::SetList | Op50::SetListO => {
                if let Some(LuaVal::Table(tid)) = regs.get(&a).cloned() {
                    let count = (bx % 32) + 1;
                    let start = bx - (bx % 32);
                    for i in 1..=count {
                        let v = regs
                            .get(&(a + i))
                            .cloned()
                            .unwrap_or_else(|| LuaVal::Unknown("slot".into()));
                        if let Some(tbl) = env.tables.get_mut(tid) {
                            tbl.set(LuaKey::Int((start + i) as i64), v);
                        }
                    }
                }
            }
            Op50::GetTable => {
                let t = regs.get(&b).cloned();
                let k = rk(c, &regs);
                let out = match (t, LuaKey::from_val(&k)) {
                    (Some(LuaVal::Table(tid)), Some(key)) => env
                        .tables
                        .get(tid)
                        .and_then(|tbl| tbl.get(&key).cloned())
                        .unwrap_or_else(|| LuaVal::Unknown(format!("field:{key:?}"))),
                    (Some(LuaVal::Global(g)), Some(LuaKey::Str(s))) => {
                        LuaVal::Global(format!("{g}.{s}"))
                    }
                    _ => LuaVal::Unknown("index".into()),
                };
                regs.insert(a, out);
            }
            Op50::SelfOp => {
                let recv = regs
                    .get(&b)
                    .cloned()
                    .unwrap_or_else(|| LuaVal::Unknown(format!("r{b}")));
                regs.insert(a + 1, recv.clone());
                let meth = rk(c, &regs).as_str().unwrap_or("?").to_owned();
                let recv_name = match recv {
                    LuaVal::Global(g) => g,
                    LuaVal::Object(cls) => cls,
                    LuaVal::Method(g, m) => format!("{g}:{m}"),
                    LuaVal::Unknown(u) if u.starts_with("global:") => u[7..].to_owned(),
                    _ => "?".to_owned(),
                };
                regs.insert(a, LuaVal::Method(recv_name, meth));
            }
            Op50::Add | Op50::Sub | Op50::Mul | Op50::Div | Op50::Pow => {
                let lhs = rk(b, &regs).as_f64();
                let rhs = rk(c, &regs).as_f64();
                let res = match (lhs, rhs) {
                    (Some(x), Some(y)) => {
                        let val = match op {
                            Op50::Add => x + y,
                            Op50::Sub => x - y,
                            Op50::Mul => x * y,
                            Op50::Div => {
                                if y == 0.0 {
                                    0.0
                                } else {
                                    x / y
                                }
                            }
                            Op50::Pow => x.powf(y),
                            _ => unreachable!(),
                        };
                        if val.is_finite() {
                            LuaVal::Num(val)
                        } else {
                            LuaVal::Unknown("arith".into())
                        }
                    }
                    _ => LuaVal::Unknown("arith".into()),
                };
                regs.insert(a, res);
            }
            Op50::Unm => {
                let val = rk(b, &regs)
                    .as_f64()
                    .filter(|n| n.is_finite())
                    .map(|n| LuaVal::Num(-n))
                    .unwrap_or_else(|| LuaVal::Unknown("unm".into()));
                regs.insert(a, val);
            }
            Op50::Not => {
                let operand = rk(b, &regs);
                let truth = if exact && matches!(operand, LuaVal::Num(_)) {
                    Some(true)
                } else {
                    operand.is_truthy()
                };
                let val = truth
                    .map(|t| LuaVal::Bool(!t))
                    .unwrap_or_else(|| LuaVal::Unknown("not".into()));
                regs.insert(a, val);
            }
            Op50::Concat => {
                let mut joined = String::new();
                let mut ok = true;
                for slot in b..=c {
                    let text = match regs.get(&slot) {
                        Some(LuaVal::Str(s)) => std::borrow::Cow::Borrowed(s.as_str()),
                        Some(LuaVal::Num(n)) => std::borrow::Cow::Owned(format!("{n}")),
                        _ => {
                            ok = false;
                            break;
                        }
                    };
                    if joined.len() + text.len() > MAX_STRING_BYTES {
                        ok = false;
                        break;
                    }
                    joined.push_str(&text);
                }
                regs.insert(
                    a,
                    if ok {
                        LuaVal::Str(joined)
                    } else {
                        LuaVal::Unknown("concat".into())
                    },
                );
            }
            Op50::Closure => {
                let mut child_path = proto_path.to_vec();
                child_path.push(bx);
                let nups = usize::from(proto.protos.get(bx).map_or(0, |p| p.nups));
                let mut caps = Vec::with_capacity(nups);
                for j in 0..nups {
                    let Some(&q) = proto.code.get(idx + 1 + j) else {
                        break;
                    };
                    let Ok(qop) = Op50::from_u32(q) else {
                        break;
                    };
                    let qb = ((q >> 15) & 0x1FF) as usize;
                    match qop {
                        Op50::Move => {
                            caps.push(
                                regs.get(&qb)
                                    .cloned()
                                    .unwrap_or_else(|| LuaVal::Unknown(format!("r{qb}"))),
                            );
                        }
                        Op50::GetUpval => {
                            caps.push(
                                ups.get(qb)
                                    .cloned()
                                    .unwrap_or_else(|| LuaVal::Unknown("upval".into())),
                            );
                        }
                        _ => caps.push(LuaVal::Unknown("upval".into())),
                    }
                }
                pc += nups;
                regs.insert(
                    a,
                    LuaVal::Closure {
                        path: child_path,
                        ups: caps,
                    },
                );
            }
            Op50::Call | Op50::TailCall => {
                let func_val = regs
                    .get(&a)
                    .cloned()
                    .unwrap_or_else(|| LuaVal::Unknown(format!("r{a}")));
                let nargs = if b == 0 {
                    if exact {
                        open_top.take().unwrap_or(a + 1).saturating_sub(a + 1)
                    } else {
                        regs.keys().copied().max().unwrap_or(a).saturating_sub(a)
                    }
                } else {
                    b.saturating_sub(1)
                };
                let call_args: Vec<LuaVal> = (0..nargs)
                    .map(|i| {
                        regs.get(&(a + 1 + i))
                            .cloned()
                            .unwrap_or_else(|| LuaVal::Unknown("arg".into()))
                    })
                    .collect();

                let mut ret_override: Option<LuaVal> = None;
                match &func_val {
                    LuaVal::Builtin(name) => match name.as_str() {
                        "table.getn" => {
                            let len = match call_args.first() {
                                Some(LuaVal::Table(tid)) => {
                                    env.tables.get(*tid).map_or(0, LuaTable::array_len)
                                }
                                _ => 0,
                            };
                            ret_override = Some(LuaVal::Num(len as f64));
                        }
                        "table.insert" => {
                            if let Some(LuaVal::Table(tid)) = call_args.first() {
                                if let Some(val) = call_args.get(1) {
                                    if let Some(tbl) = env.tables.get_mut(*tid) {
                                        let next = tbl.array_len() as i64 + 1;
                                        tbl.set(LuaKey::Int(next), val.clone());
                                    }
                                }
                            }
                        }
                        "string.lower" => {
                            if let Some(s) = call_args.first().and_then(|v| v.as_str()) {
                                ret_override = Some(LuaVal::Str(s.to_ascii_lowercase()));
                            }
                        }
                        _ => {}
                    },
                    LuaVal::Closure {
                        path,
                        ups: child_ups,
                    } => {
                        if call_local_closures {
                            eval_proto(env, path, &call_args, child_ups, true, depth + 1);
                        }
                    }
                    LuaVal::Global(name) => {
                        let loaded = if exact && name == "ScriptCB_DoFile" {
                            load_addon_script(env, &call_args, depth)
                        } else {
                            false
                        };
                        if !loaded {
                            env.check_dynamic_call(name, &call_args);
                        }
                        if let Some(trace) = env.trace.as_mut() {
                            ret_override = match name.as_str() {
                                // A table is its own iterator for `TForPrep`.
                                "pairs" | "ipairs" => call_args
                                    .first()
                                    .filter(|v| matches!(v, LuaVal::Table(_)))
                                    .cloned(),
                                "type" => call_args.first().and_then(type_name),
                                // Registration sees an installation with no separate addons.
                                "ScriptCB_IsFileExist" => Some(LuaVal::Num(0.0)),
                                "ScriptCB_InMultiplayer" => Some(LuaVal::Bool(trace.multiplayer)),
                                "CreateTimer" => call_args.first().cloned(),
                                "CreateEffect" => call_args
                                    .first()
                                    .and_then(LuaVal::as_str)
                                    .map(|name| LuaVal::Effect(name.to_owned())),
                                "GetEntityMatrix" => call_args
                                    .first()
                                    .and_then(LuaVal::as_str)
                                    .map(|name| LuaVal::MatrixObject(name.to_owned())),
                                "GetPathPoint" => call_args
                                    .first()
                                    .and_then(LuaVal::as_str)
                                    .zip(
                                        call_args
                                            .get(1)
                                            .and_then(LuaVal::as_i64)
                                            .and_then(|i| usize::try_from(i).ok()),
                                    )
                                    .map(|(path, index)| {
                                        LuaVal::MatrixPath(path.to_owned(), index)
                                    }),
                                "math.random" | "random" => {
                                    let range = match call_args.as_slice() {
                                        [hi] => hi.as_i64().map(|hi| (1, hi)),
                                        [lo, hi] => lo.as_i64().zip(hi.as_i64()),
                                        _ => None,
                                    };
                                    if trace.random_range.is_none() {
                                        trace.random_range = range;
                                    }
                                    range.map(|(lo, hi)| {
                                        LuaVal::Num(
                                            trace
                                                .random_pick
                                                .filter(|n| (lo..=hi).contains(n))
                                                .unwrap_or(lo)
                                                as f64,
                                        )
                                    })
                                }
                                _ => None,
                            };
                        }
                        env.calls.push(LuaCall {
                            func: name.clone(),
                            args: call_args,
                        });
                    }
                    LuaVal::Method(recv, meth) => {
                        let full = format!("{recv}:{meth}");
                        env.check_dynamic_call(&full, &call_args);
                        env.calls.push(LuaCall {
                            func: full,
                            args: call_args,
                        });
                        if meth.eq_ignore_ascii_case("New") {
                            ret_override = Some(LuaVal::Object(recv.clone()));
                        }
                    }
                    _ => {}
                }

                if op == Op50::Call {
                    let nrets = if c == 0 { 1 } else { c.saturating_sub(1) };
                    if exact {
                        open_top = (c == 0).then_some(a + nrets);
                    }
                    for i in 0..nrets {
                        let v = if i == 0 {
                            ret_override
                                .clone()
                                .unwrap_or_else(|| LuaVal::Unknown("ret".into()))
                        } else {
                            LuaVal::Unknown("ret".into())
                        };
                        regs.insert(a + i, v);
                    }
                } else if exact {
                    return;
                }
            }
            Op50::Jmp if exact => pc = jump_target(pc, bx, proto.code.len()),
            Op50::Eq | Op50::Lt | Op50::Le if exact => {
                let compare = comparison(op, &rk(b, &regs), &rk(c, &regs));
                if compare != Some(a != 0) {
                    pc += 1;
                }
            }
            Op50::Test if exact => {
                let val = regs.get(&b).cloned().unwrap_or(LuaVal::Nil);
                let truth = matches!(val, LuaVal::Num(_)) || val.is_truthy().unwrap_or(false);
                if truth == (c != 0) {
                    regs.insert(a, val);
                } else {
                    pc += 1;
                }
            }
            // Lua 5.0 enters a generic `for` at its `TForLoop`, which either
            // fills the loop variables and takes the jump back that follows it
            // or skips that jump. An unknown iterator gets one pass.
            Op50::TForPrep if exact => {
                if matches!(regs.get(&a), Some(LuaVal::Table(_))) {
                    cursors.insert(a, 0);
                    pc = jump_target(pc, bx, proto.code.len());
                } else {
                    cursors.remove(&a);
                }
            }
            Op50::TForLoop if exact => {
                let next = match (regs.get(&a), cursors.get(&a)) {
                    (Some(LuaVal::Table(tid)), Some(&at)) => env
                        .tables
                        .get(*tid)
                        .and_then(|tbl| tbl.entries.get(at).cloned()),
                    _ => None,
                };
                if let Some((key, value)) = next {
                    *cursors.entry(a).or_default() += 1;
                    regs.insert(a + 2, key_to_val(key));
                    regs.insert(a + 3, value);
                } else {
                    pc += 1;
                }
            }
            Op50::Return if exact => return,
            Op50::Return
            | Op50::SetUpval
            | Op50::Jmp
            | Op50::Eq
            | Op50::Lt
            | Op50::Le
            | Op50::Test
            | Op50::ForLoop
            | Op50::TForLoop
            | Op50::TForPrep
            | Op50::Close => {}
        }
    }
}

fn load_addon_script(env: &mut EvalEnv, args: &[LuaVal], depth: usize) -> bool {
    let Some(name) = args.first().and_then(LuaVal::as_str) else {
        return false;
    };
    if is_known_standard_dofile(name) || depth >= MAX_CALL_DEPTH {
        return false;
    }
    let Some(trace) = env.trace.as_mut() else {
        return false;
    };
    let key = name.to_ascii_lowercase();
    let Some(script) = trace.scripts.get(&key) else {
        return false;
    };
    if trace.loaded_scripts.insert(key) {
        let index = env.root.protos.len();
        env.root.protos.push(script.clone());
        eval_proto(env, &[index], &[], &[], true, depth + 1);
    }
    true
}

/// Evaluate top-level statements (ideal for `addme.script`), allowing calls to
/// local helper closures defined in the chunk up to `MAX_CALL_DEPTH`.
pub fn evaluate_toplevel(proto: Proto) -> EvalEnv {
    let mut env = new_env(proto);
    eval_proto(&mut env, &[], &[], &[], true, 0);
    env
}

/// The maps of Battlefront II's own Instant Action lists. Each entry's
/// `mapluafile` is `<code>%s_%s`: era letter, then mode.
const STOCK_MENU_MAPS: [&str; 24] = [
    "cor1", "dag1", "dea1", "end1", "fel1", "geo1", "hot1", "tat3", "kam1", "kas2", "tat2", "mus1",
    "myg1", "nab2", "pol1", "spa7", "spa8", "spa3", "spa6", "spa9", "spa1", "tan1", "uta1", "yav1",
];

/// The stock map whose menu entry has the `mapluafile` `template`.
pub(crate) fn stock_menu_map(template: &str) -> Option<&'static str> {
    let code = template.to_ascii_lowercase();
    let code = code.strip_suffix("%s_%s")?;
    STOCK_MENU_MAPS.into_iter().find(|map| *map == code)
}

/// The game's lists already hold its own maps when an `addme.script` runs, and
/// an addon adds eras and modes to one by editing its entry
/// (`AddNewGameModes`).
fn seed_stock_menu(env: &mut EvalEnv) {
    for list in [env.sp_listbox_id, env.mp_listbox_id] {
        for (index, code) in STOCK_MENU_MAPS.into_iter().enumerate() {
            let Some(id) = env.alloc_table() else {
                return;
            };
            if let Some(entry) = env.tables.get_mut(id) {
                entry.set(
                    LuaKey::Str("mapluafile".into()),
                    LuaVal::Str(format!("{code}%s_%s")),
                );
            }
            if let Some(table) = env.tables.get_mut(list) {
                table.set(LuaKey::Int(index as i64 + 1), LuaVal::Table(id));
            }
        }
    }
}

pub(crate) fn evaluate_standalone_registration(proto: Proto) -> EvalEnv {
    let mut env = new_env(proto);
    env.trace = Some(StandaloneTrace::default());
    seed_stock_menu(&mut env);
    eval_proto(&mut env, &[], &[], &[], true, 0);
    env
}

/// Structured analysis of a BF2 mission script: top-level globals, `ScriptInit`
/// setup calls, and `ScriptPostLoad` startup calls (plus zero-arg helper
/// functions called by either), ignoring dormant runtime event callbacks.
#[derive(Debug, Clone)]
pub struct MissionScriptAnalysis {
    pub env: EvalEnv,
    pub init_calls: Vec<LuaCall>,
    pub postload_calls: Vec<LuaCall>,
}

impl MissionScriptAnalysis {
    pub(crate) fn callback_calls(&self, callback: &LuaVal) -> Vec<LuaCall> {
        let LuaVal::Closure { path, ups } = callback else {
            return Vec::new();
        };
        let mut env = self.env.clone();
        let steps = env.steps;
        env.calls.clear();
        eval_proto(&mut env, path, &[], ups, true, 0);
        if steps <= MAX_EVAL_STEPS && env.steps <= MAX_EVAL_STEPS {
            env.calls
        } else {
            Vec::new()
        }
    }
}

/// Analyze a BF2 mission script (`ScriptInit` + `ScriptPostLoad`).
pub fn analyze_mission_script(proto: Proto) -> MissionScriptAnalysis {
    let mut env = new_env(proto);
    // Top-level defines globals/constants and helper closures (such as
    // `ScriptInit` and `ScriptPostLoad`).
    eval_proto(&mut env, &[], &[], &[], false, 0);
    let top_calls_end = env.calls.len();

    let mut visited = BTreeSet::new();
    if let Some(LuaVal::Closure { path, ups }) = env.globals.get("ScriptInit").cloned() {
        visited.insert(path.clone());
        eval_proto(&mut env, &path, &[], &ups, false, 0);
        // Also inline calls to helper closures registered inside this chunk.
        let snapshot = env.calls[top_calls_end..].to_vec();
        follow_local_helpers(&mut env, &snapshot, &mut visited, 1);
    }
    let init_end = env.calls.len();

    if let Some(LuaVal::Closure { path, ups }) = env.globals.get("ScriptPostLoad").cloned() {
        let mut post_visited = BTreeSet::new();
        post_visited.insert(path.clone());
        eval_proto(&mut env, &path, &[], &ups, false, 0);
        let snapshot = env.calls[init_end..].to_vec();
        follow_local_helpers(&mut env, &snapshot, &mut post_visited, 1);
    }

    let init_calls = env.calls[0..init_end].to_vec();
    let postload_calls = env.calls[init_end..].to_vec();
    MissionScriptAnalysis {
        env,
        init_calls,
        postload_calls,
    }
}

/// Analyze a bounded first random choice and the multiplayer setup.
/// Equivalent world selections are coalesced by the addon inspector.
pub(crate) fn analyze_standalone_variants(
    proto: &Proto,
    scripts: &Arc<BTreeMap<String, Proto>>,
) -> (Vec<MissionScriptAnalysis>, MissionScriptAnalysis) {
    let run = |random_pick, multiplayer| {
        let mut env = new_env(proto.clone());
        env.trace = Some(StandaloneTrace {
            random_pick,
            multiplayer,
            scripts: Arc::clone(scripts),
            ..Default::default()
        });
        eval_proto(&mut env, &[], &[], &[], true, 0);
        let mut run_hook = |name| {
            if let Some(LuaVal::Closure { path, ups }) = env.globals.get(name).cloned() {
                eval_proto(&mut env, &path, &[], &ups, true, 0);
            }
            env.calls.len()
        };
        let init_end = run_hook("ScriptInit");
        run_hook("ScriptPostLoad");
        let init_calls = env.calls[..init_end].to_vec();
        let postload_calls = env.calls[init_end..].to_vec();
        MissionScriptAnalysis {
            env,
            init_calls,
            postload_calls,
        }
    };
    let first = run(None, false);
    let multiplayer = run(None, true);
    let Some((lo, hi)) = first
        .env
        .trace
        .as_ref()
        .and_then(|trace| trace.random_range)
    else {
        return (vec![first], multiplayer);
    };
    if !matches!(hi.checked_sub(lo), Some(1..=15)) {
        return (vec![first], multiplayer);
    }
    let single_player = std::iter::once(first)
        .chain((lo + 1..=hi).map(|pick| run(Some(pick), false)))
        .collect();
    (single_player, multiplayer)
}

fn follow_local_helpers(
    env: &mut EvalEnv,
    calls: &[LuaCall],
    visited: &mut BTreeSet<Vec<usize>>,
    depth: usize,
) {
    if depth > 4 {
        return;
    }
    for call in calls {
        if let Some(LuaVal::Closure { path, ups }) = env.globals.get(&call.func).cloned() {
            if visited.insert(path.clone()) {
                let start = env.calls.len();
                eval_proto(env, &path, &call.args, &ups, false, depth);
                let nested = env.calls[start..].to_vec();
                follow_local_helpers(env, &nested, visited, depth + 1);
            }
        }
    }
}
