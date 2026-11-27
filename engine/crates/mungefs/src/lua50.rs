//! Bounded, non-executing static parser and abstract interpreter for Lua 5.0
//! bytecode (`\x1bLua\x50`) inside legacy Battlefront II `scr_` chunks.
//!
//! Designed strictly for static extraction of declarative addon registration
//! (`addme.script`) and mission scripts (`mission.lvl`) without ever executing
//! untrusted script loops or native hooks.

use thiserror::Error;

use crate::{chunks, Chunk, UcfbError};

const LUA_SIG: &[u8; 4] = b"\x1bLua";
const LUA_VERSION_50: u8 = 0x50;

const MAX_BYTECODE_BYTES: usize = 8 * 1024 * 1024;
const MAX_PROTO_DEPTH: usize = 16;
const MAX_PROTOS: usize = 256;
const MAX_CONSTANTS: usize = 4096;
const MAX_INSTRUCTIONS: usize = 65_536;
const MAX_LOCALS: usize = 4096;
const MAX_UPVALUES: usize = 256;
const MAX_LINES: usize = 131_072;
const MAX_STRING_BYTES: usize = 1024 * 1024;
const MAX_EVAL_STEPS: usize = 100_000;
const MAX_TABLES: usize = 4096;
const MAX_TABLE_ENTRIES: usize = 4096;
const MAX_CALL_DEPTH: usize = 6;
const MAX_UCFB_DEPTH: usize = 8;

/// Lua 5.0 opcodes (`lopcodes.h` order, indices `0..=34`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Op50 {
    Move = 0,
    LoadK = 1,
    LoadBool = 2,
    LoadNil = 3,
    GetUpval = 4,
    GetGlobal = 5,
    GetTable = 6,
    SetGlobal = 7,
    SetUpval = 8,
    SetTable = 9,
    NewTable = 10,
    SelfOp = 11,
    Add = 12,
    Sub = 13,
    Mul = 14,
    Div = 15,
    Pow = 16,
    Unm = 17,
    Not = 18,
    Concat = 19,
    Jmp = 20,
    Eq = 21,
    Lt = 22,
    Le = 23,
    Test = 24,
    Call = 25,
    TailCall = 26,
    Return = 27,
    ForLoop = 28,
    TForLoop = 29,
    TForPrep = 30,
    SetList = 31,
    SetListO = 32,
    Close = 33,
    Closure = 34,
}

impl Op50 {
    fn from_u32(ins: u32) -> Result<Self, LuaError> {
        let code = (ins & 0x3F) as u8;
        match code {
            0 => Ok(Self::Move),
            1 => Ok(Self::LoadK),
            2 => Ok(Self::LoadBool),
            3 => Ok(Self::LoadNil),
            4 => Ok(Self::GetUpval),
            5 => Ok(Self::GetGlobal),
            6 => Ok(Self::GetTable),
            7 => Ok(Self::SetGlobal),
            8 => Ok(Self::SetUpval),
            9 => Ok(Self::SetTable),
            10 => Ok(Self::NewTable),
            11 => Ok(Self::SelfOp),
            12 => Ok(Self::Add),
            13 => Ok(Self::Sub),
            14 => Ok(Self::Mul),
            15 => Ok(Self::Div),
            16 => Ok(Self::Pow),
            17 => Ok(Self::Unm),
            18 => Ok(Self::Not),
            19 => Ok(Self::Concat),
            20 => Ok(Self::Jmp),
            21 => Ok(Self::Eq),
            22 => Ok(Self::Lt),
            23 => Ok(Self::Le),
            24 => Ok(Self::Test),
            25 => Ok(Self::Call),
            26 => Ok(Self::TailCall),
            27 => Ok(Self::Return),
            28 => Ok(Self::ForLoop),
            29 => Ok(Self::TForLoop),
            30 => Ok(Self::TForPrep),
            31 => Ok(Self::SetList),
            32 => Ok(Self::SetListO),
            33 => Ok(Self::Close),
            34 => Ok(Self::Closure),
            _ => Err(LuaError::Malformed(format!(
                "unknown Lua 5.0 opcode {code}"
            ))),
        }
    }
}

#[derive(Error, Debug)]
pub enum LuaError {
    #[error("bytecode too large ({0} bytes)")]
    TooLarge(usize),
    #[error("invalid Lua 5.0 bytecode header: {0}")]
    BadHeader(String),
    #[error("truncated Lua 5.0 bytecode at offset {0}")]
    Truncated(usize),
    #[error("malformed Lua 5.0 bytecode: {0}")]
    Malformed(String),
    #[error("Lua 5.0 safety limit exceeded: {0}")]
    LimitExceeded(String),
}

#[derive(Debug, Clone)]
pub struct ScriptChunk {
    pub name: String,
    pub body: Vec<u8>,
}

/// Extract every `scr_` chunk (`NAME` + `BODY`) from an opened `ucfb`/LVL buffer
/// (or a buffer starting with `ucfb`). Walks `lvl_` and hashed sub-level
/// containers up to `MAX_UCFB_DEPTH`.
pub fn extract_scripts(buf: &[u8]) -> Result<Vec<ScriptChunk>, UcfbError> {
    let slice = if buf.len() >= 8 && &buf[0..4] == b"ucfb" {
        crate::open(buf)?
    } else {
        buf
    };
    let mut out = Vec::new();
    collect_scripts_recursive(slice, 0, &mut out)?;
    Ok(out)
}

fn collect_scripts_recursive(
    buf: &[u8],
    depth: usize,
    out: &mut Vec<ScriptChunk>,
) -> Result<(), UcfbError> {
    if depth > MAX_UCFB_DEPTH {
        return Err(UcfbError::Malformed(
            "script container nesting too deep".into(),
        ));
    }
    for c in chunks(buf) {
        let c = c?;
        collect_from_chunk(&c, depth, out)?;
    }
    Ok(())
}

fn collect_from_chunk(
    c: &Chunk<'_>,
    depth: usize,
    out: &mut Vec<ScriptChunk>,
) -> Result<(), UcfbError> {
    if &c.tag == b"scr_" {
        let name = c.child_string(b"NAME").unwrap_or_default();
        if let Some(body) = c.child(b"BODY") {
            out.push(ScriptChunk {
                name,
                body: body.payload.to_vec(),
            });
        }
        return Ok(());
    }
    if &c.tag == b"lvl_" {
        if depth + 1 > MAX_UCFB_DEPTH {
            return Err(UcfbError::Malformed("lvl_ nesting too deep".into()));
        }
        for sub in c.children() {
            let sub = sub?;
            if &sub.tag == b"scr_" {
                collect_from_chunk(&sub, depth + 1, out)?;
            } else if !crate::CONTENT_TAGS.contains(&&sub.tag) {
                collect_scripts_recursive(sub.payload, depth + 1, out)?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub enum LuaConst {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct Proto {
    pub source: Option<String>,
    pub nups: u8,
    pub numparams: u8,
    pub is_vararg: u8,
    pub maxstack: u8,
    pub constants: Vec<LuaConst>,
    pub protos: Vec<Proto>,
    pub code: Vec<u32>,
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    little_endian: bool,
    int_size: usize,
    size_t_size: usize,
    number_size: usize,
    proto_count: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], LuaError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(LuaError::Truncated(self.pos))?;
        if end > self.buf.len() {
            return Err(LuaError::Truncated(self.pos));
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn byte(&mut self) -> Result<u8, LuaError> {
        Ok(self.take(1)?[0])
    }

    fn read_uint(&mut self, size: usize) -> Result<usize, LuaError> {
        let b = self.take(size)?;
        let val = match size {
            4 => {
                let arr: [u8; 4] = b.try_into().unwrap();
                if self.little_endian {
                    u32::from_le_bytes(arr) as usize
                } else {
                    u32::from_be_bytes(arr) as usize
                }
            }
            8 => {
                let arr: [u8; 8] = b.try_into().unwrap();
                let u = if self.little_endian {
                    u64::from_le_bytes(arr)
                } else {
                    u64::from_be_bytes(arr)
                };
                usize::try_from(u)
                    .map_err(|_| LuaError::Malformed("integer exceeds usize".into()))?
            }
            other => {
                return Err(LuaError::BadHeader(format!("unsupported int size {other}")));
            }
        };
        Ok(val)
    }

    fn int(&mut self) -> Result<usize, LuaError> {
        self.read_uint(self.int_size)
    }

    fn size_t(&mut self) -> Result<usize, LuaError> {
        self.read_uint(self.size_t_size)
    }

    fn number(&mut self) -> Result<f64, LuaError> {
        let b = self.take(self.number_size)?;
        match self.number_size {
            4 => {
                let arr: [u8; 4] = b.try_into().unwrap();
                let v = if self.little_endian {
                    f32::from_le_bytes(arr)
                } else {
                    f32::from_be_bytes(arr)
                };
                Ok(f64::from(v))
            }
            8 => {
                let arr: [u8; 8] = b.try_into().unwrap();
                let v = if self.little_endian {
                    f64::from_le_bytes(arr)
                } else {
                    f64::from_be_bytes(arr)
                };
                Ok(v)
            }
            other => Err(LuaError::BadHeader(format!(
                "unsupported lua_Number size {other}"
            ))),
        }
    }

    fn string(&mut self) -> Result<Option<String>, LuaError> {
        let len = self.size_t()?;
        if len == 0 {
            return Ok(None);
        }
        if len > MAX_STRING_BYTES {
            return Err(LuaError::LimitExceeded(format!(
                "string length {len} exceeds {MAX_STRING_BYTES}"
            )));
        }
        let raw = self.take(len)?;
        let content = &raw[..len.saturating_sub(1)];
        let s = match std::str::from_utf8(content) {
            Ok(utf8) => utf8.to_owned(),
            Err(_) => content.iter().map(|&b| char::from(b)).collect(),
        };
        Ok(Some(s))
    }

    fn func(&mut self, parent_source: Option<&str>, depth: usize) -> Result<Proto, LuaError> {
        if depth > MAX_PROTO_DEPTH {
            return Err(LuaError::LimitExceeded(format!(
                "prototype nesting depth > {MAX_PROTO_DEPTH}"
            )));
        }
        self.proto_count += 1;
        if self.proto_count > MAX_PROTOS {
            return Err(LuaError::LimitExceeded(format!(
                "prototype count > {MAX_PROTOS}"
            )));
        }

        let source = self.string()?.or_else(|| parent_source.map(str::to_owned));
        let _line_defined = self.int()?;
        let nups = self.byte()?;
        let numparams = self.byte()?;
        let is_vararg = self.byte()?;
        let maxstack = self.byte()?;

        let n_lines = self.int()?;
        if n_lines > MAX_LINES {
            return Err(LuaError::LimitExceeded(format!("lineinfo > {MAX_LINES}")));
        }
        self.take(
            n_lines
                .checked_mul(self.int_size)
                .ok_or_else(|| LuaError::Malformed("lineinfo overflow".into()))?,
        )?;

        let n_locals = self.int()?;
        if n_locals > MAX_LOCALS {
            return Err(LuaError::LimitExceeded(format!("locals > {MAX_LOCALS}")));
        }
        for _ in 0..n_locals {
            let _ = self.string()?;
            let _ = self.int()?;
            let _ = self.int()?;
        }

        let n_upvals = self.int()?;
        if n_upvals > MAX_UPVALUES {
            return Err(LuaError::LimitExceeded(format!(
                "upvalues > {MAX_UPVALUES}"
            )));
        }
        for _ in 0..n_upvals {
            let _ = self.string()?;
        }

        let n_consts = self.int()?;
        if n_consts > MAX_CONSTANTS {
            return Err(LuaError::LimitExceeded(format!(
                "constants {n_consts} > {MAX_CONSTANTS}"
            )));
        }
        let mut constants = Vec::with_capacity(n_consts);
        for _ in 0..n_consts {
            let tag = self.byte()?;
            let c = match tag {
                0 => LuaConst::Nil,
                1 => LuaConst::Bool(self.byte()? != 0),
                3 => LuaConst::Num(self.number()?),
                4 => LuaConst::Str(self.string()?.unwrap_or_default()),
                other => {
                    return Err(LuaError::Malformed(format!(
                        "unknown constant type tag {other}"
                    )));
                }
            };
            constants.push(c);
        }

        let n_protos = self.int()?;
        if n_protos > MAX_PROTOS {
            return Err(LuaError::LimitExceeded(format!(
                "child protos {n_protos} > {MAX_PROTOS}"
            )));
        }
        let mut protos = Vec::with_capacity(n_protos);
        for _ in 0..n_protos {
            protos.push(self.func(source.as_deref(), depth + 1)?);
        }

        let n_code = self.int()?;
        if n_code > MAX_INSTRUCTIONS {
            return Err(LuaError::LimitExceeded(format!(
                "instruction count {n_code} > {MAX_INSTRUCTIONS}"
            )));
        }
        let raw_code = self.take(
            n_code
                .checked_mul(4)
                .ok_or_else(|| LuaError::Malformed("code size overflow".into()))?,
        )?;
        let mut code = Vec::with_capacity(n_code);
        for word in raw_code.chunks_exact(4) {
            let arr: [u8; 4] = word.try_into().unwrap();
            let ins = if self.little_endian {
                u32::from_le_bytes(arr)
            } else {
                u32::from_be_bytes(arr)
            };
            // Validate opcode eagerly so invalid instruction streams fail early.
            let _ = Op50::from_u32(ins)?;
            code.push(ins);
        }

        Ok(Proto {
            source,
            nups,
            numparams,
            is_vararg,
            maxstack,
            constants,
            protos,
            code,
        })
    }
}

/// Undump a Lua 5.0 precompiled chunk header + root prototype without executing it.
pub fn undump(body: &[u8]) -> Result<Proto, LuaError> {
    if body.len() > MAX_BYTECODE_BYTES {
        return Err(LuaError::TooLarge(body.len()));
    }
    if body.len() < 14 {
        return Err(LuaError::Truncated(body.len()));
    }
    if &body[0..4] != LUA_SIG {
        return Err(LuaError::BadHeader("missing \\x1bLua signature".into()));
    }
    if body[4] != LUA_VERSION_50 {
        return Err(LuaError::BadHeader(format!(
            "unsupported Lua version 0x{:02x} (expected 0x50)",
            body[4]
        )));
    }
    let little_endian = body[5] != 0;
    let int_size = usize::from(body[6]);
    let size_t_size = usize::from(body[7]);
    let ins_size = usize::from(body[8]);
    let op_bits = body[9];
    let a_bits = body[10];
    let b_bits = body[11];
    let c_bits = body[12];
    let number_size = usize::from(body[13]);

    if !matches!(int_size, 4 | 8)
        || !matches!(size_t_size, 4 | 8)
        || ins_size != 4
        || !matches!(number_size, 4 | 8)
    {
        return Err(LuaError::BadHeader(format!(
            "unsupported ABI sizes int={int_size} size_t={size_t_size} ins={ins_size} num={number_size}"
        )));
    }
    if op_bits != 6 || a_bits != 8 || b_bits != 9 || c_bits != 9 {
        return Err(LuaError::BadHeader(format!(
            "unsupported instruction bit widths ({op_bits},{a_bits},{b_bits},{c_bits})"
        )));
    }

    let mut r = Reader {
        buf: body,
        pos: 14,
        little_endian,
        int_size,
        size_t_size,
        number_size,
        proto_count: 0,
    };
    let _test_num = r.number()?;
    r.func(None, 0)
}

/// Detect the RK constant threshold `(128 | 250 | 256)` matching `scripts/gen_missions.py`.
pub fn rk_base(root: &Proto) -> usize {
    fn check_proto(p: &Proto, base: usize) -> bool {
        let n = p.constants.len();
        for &ins in &p.code {
            let Ok(op) = Op50::from_u32(ins) else {
                return false;
            };
            if matches!(
                op,
                Op50::SetTable
                    | Op50::GetTable
                    | Op50::SelfOp
                    | Op50::Add
                    | Op50::Sub
                    | Op50::Mul
                    | Op50::Div
                    | Op50::Pow
                    | Op50::Eq
                    | Op50::Lt
                    | Op50::Le
            ) {
                let b = ((ins >> 15) & 0x1FF) as usize;
                let c = ((ins >> 6) & 0x1FF) as usize;
                if (b >= base && b - base >= n) || (c >= base && c - base >= n) {
                    return false;
                }
            }
        }
        p.protos.iter().all(|child| check_proto(child, base))
    }

    for base in [128usize, 250, 256] {
        if check_proto(root, base) {
            return base;
        }
    }
    128
}

mod eval;
pub use eval::*;

#[cfg(test)]
mod tests;
