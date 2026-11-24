use std::collections::{BTreeMap, BTreeSet};

use super::{
    rk_base, LuaConst, Op50, Proto, MAX_CALL_DEPTH, MAX_EVAL_STEPS, MAX_TABLES, MAX_TABLE_ENTRIES,
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
}

impl EvalEnv {
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
    let mut skip = 0usize;

    for (idx, &ins) in proto.code.iter().enumerate() {
        if skip > 0 {
            skip -= 1;
            continue;
        }
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
                let val = rk(b, &regs)
                    .is_truthy()
                    .map(|t| LuaVal::Bool(!t))
                    .unwrap_or_else(|| LuaVal::Unknown("not".into()));
                regs.insert(a, val);
            }
            Op50::Concat => {
                let mut joined = String::new();
                let mut ok = true;
                for slot in b..=c {
                    match regs.get(&slot) {
                        Some(LuaVal::Str(s)) => joined.push_str(s),
                        Some(LuaVal::Num(n)) => joined.push_str(&format!("{n}")),
                        _ => {
                            ok = false;
                            break;
                        }
                    }
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
                skip = nups;
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
                    let top = regs.keys().copied().max().unwrap_or(a);
                    top.saturating_sub(a)
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
                        env.check_dynamic_call(name, &call_args);
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
                }
            }
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

/// Evaluate top-level statements (ideal for `addme.script`), allowing calls to
/// local helper closures defined in the chunk up to `MAX_CALL_DEPTH`.
pub fn evaluate_toplevel(proto: Proto) -> EvalEnv {
    let mut env = new_env(proto);
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
