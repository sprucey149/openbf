#!/usr/bin/env python3
"""gen_missions.py -- generate engine/missions/*.ron from the PC mod-tools Lua
sources, cross-checked against the PS2 mission bytecode.

Audit A6 (analysis/a6-mission-lua.md) established that the Gametoast
mod-tools .lua files ARE the shipped mission scripts (100 % string-constant
overlap on BF1 tat1r/bes1a), so the readable PC source is the authoring
input and the PS2 bytecode is the referee.  Where the two disagree the PS2
wins (it is the target) and the RON says PROVISIONAL.

Inputs
  PC   community_mods/gametoast_lua/doc/StarWarsBattlefront{,II}/SourceCode/
       Common/{Scripts,scripts}/<PLANET>/<script>.lua        (gitignored)
  PS2  extracted/{bf1,bf2}/lua/mission/<script>.luac         (gitignored)

The PS2 side is read with the two undumpers below rather than the linear
string scan in scripts/extract_lua.py, because a constant-table dump cannot
answer "what did SetUnitCount get called with": BF1's Lua 4.0 compiler puts
small integers in PUSHINT *operands* (never in the constant table at all),
and BF2 hides the numbers inside a `SetupTeams{...}` table constructor.  So
both dumps are abstract-interpreted instead -- enough of each VM (constants,
registers/stack, locals, upvalues, globals, table constructors, calls) to
reconstruct the literal argument list of every engine call.  Mission scripts
are straight-line declarative code, so this recovers them exactly; anything
that does not resolve comes back as `Unknown` and is reported, never guessed.

That turned out to matter: PS2 retail shrinks nearly every BF2 member pool
(tat2c_con is units=20 on the disc against the PC source's 28, matching
re-tickets-spawn.md §1.5), and trims four BF1 pools too.

Without the PC sources (a public checkout has no community_mods; card PACK),
`--ps2-only` builds from the bytecode alone: the same values wherever the PS2
script has them -- which is every value of every shipped mission -- minus the
referee's PC/PS2 notes.

Usage
  scripts/gen_missions.py                 # write engine/missions/*.ron
  scripts/gen_missions.py --ps2-only [--lua-root DIR] [--out DIR]
                                          # scripts/pack.py: from the ISO alone
  scripts/gen_missions.py --check         # report only, write nothing
  scripts/gen_missions.py --dump bf1 tat1r        # PS2 call trace (Lua 4.0)
  scripts/gen_missions.py --dump bf2 tat2g_con    # PS2 call trace (Lua 5.0)
"""

import argparse
import os
import re
import struct
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PC_BF1 = os.path.join(
    ROOT, "community_mods/gametoast_lua/doc/StarWarsBattlefront/SourceCode/Common/Scripts"
)
PC_BF2 = os.path.join(
    ROOT,
    "community_mods/gametoast_lua/doc/StarWarsBattlefrontII/SourceCode/Common/scripts",
)
PS2_BF1 = os.path.join(ROOT, "extracted/bf1/lua/mission")
PS2_BF2 = os.path.join(ROOT, "extracted/bf2/lua/mission")
OUT_DIR = os.path.join(ROOT, "engine/missions")
PS2_ONLY = False


# ===========================================================  Lua 4.0 undump

# lua-4.0/src/lopcodes.h, in order.
OPS40 = """END RETURN CALL TAILCALL PUSHNIL POP PUSHINT PUSHSTRING PUSHNUM
PUSHNEGNUM PUSHUPVALUE GETLOCAL GETGLOBAL GETTABLE GETDOTTED GETINDEXED
PUSHSELF CREATETABLE SETLOCAL SETGLOBAL SETTABLE SETLIST SETMAP ADD ADDI SUB
MULT DIV POW CONCAT MINUS NOT JMPNE JMPEQ JMPLT JMPLE JMPGT JMPGE JMPT JMPF
JMPONT JMPONF JMP PUSHNILJMP FORPREP FORLOOP LFORPREP LFORLOOP CLOSURE""".split()

SIZE_OP40, SIZE_B40 = 6, 9
POS_B40 = SIZE_OP40
POS_A40 = SIZE_OP40 + SIZE_B40
MAXARG_U40 = (1 << (32 - SIZE_OP40)) - 1
MAXARG_S40 = MAXARG_U40 >> 1


class Reader:
    def __init__(self, buf, pos=0):
        self.b, self.p = buf, pos

    def byte(self):
        v = self.b[self.p]
        self.p += 1
        return v

    def int(self):
        (v,) = struct.unpack_from("<i", self.b, self.p)
        self.p += 4
        return v

    def size(self):
        (v,) = struct.unpack_from("<I", self.b, self.p)
        self.p += 4
        return v

    def num(self):
        (v,) = struct.unpack_from("<f", self.b, self.p)
        self.p += 4
        return v

    def string(self):
        n = self.size()
        if n == 0:
            return None
        s = self.b[self.p:self.p + n - 1].decode("latin-1")
        self.p += n
        return s

    def block(self, n):
        v = self.b[self.p:self.p + n]
        self.p += n
        return v


class Proto:
    def __init__(self):
        self.source = None
        self.kstr = []
        self.knum = []
        self.k = []          # 5.0 mixed constant table
        self.protos = []
        self.code = []
        self.nups = 0


def undump40(body):
    """Parse a Lua 4.0 dump (header already validated) -> root Proto."""
    r = Reader(body, 17)  # 4 sig + version + 8 size bytes + 4-byte test number
    return _func40(r)


def _func40(r):
    f = Proto()
    f.source = r.string()
    r.int()          # lineDefined
    r.int()          # numparams
    r.byte()         # is_vararg
    r.int()          # maxstacksize
    for _ in range(r.int()):        # locals
        r.string()
        r.int()
        r.int()
    for _ in range(r.int()):        # line info
        r.int()
    f.kstr = [r.string() for _ in range(r.int())]
    f.knum = [r.num() for _ in range(r.int())]
    f.protos = [_func40(r) for _ in range(r.int())]
    n = r.int()
    raw = r.block(n * 4)
    f.code = list(struct.unpack_from("<%dI" % n, raw, 0)) if n else []
    return f


def _dec40(i):
    op = OPS40[i & 0x3F]
    u = (i >> SIZE_OP40) & MAXARG_U40
    return op, u, u - MAXARG_S40, (i >> POS_B40) & 0x1FF, (i >> POS_A40) & 0x1FFFF


class Call:
    """A reconstructed `name(arg, ...)` with literal arguments."""

    def __init__(self, name, args, prototype=None):
        self.name, self.args = name, args
        # Optional source provenance for census tools; no execution claim.
        self.prototype = prototype

    def __repr__(self):
        return "%s(%s)" % (self.name, ", ".join(repr(a) for a in self.args))


class Unknown:
    """A value the abstract stack could not resolve (register, expression)."""

    def __init__(self, what="?"):
        self.what = what

    def __repr__(self):
        return "<%s>" % self.what


class Closure(Unknown):
    """A `CLOSURE` value: unresolved as a value, but it knows its proto."""

    def __init__(self, proto):
        Unknown.__init__(self, "closure")
        self.proto = proto


def calls40(f, out=None):
    """Abstract-interpret a Lua 4.0 proto and yield the engine calls it makes.

    Mission ScriptInit bodies are a flat sequence of `Foo(literal, ...)`, so
    a value stack that tracks literals and treats everything else as Unknown
    reconstructs them exactly.  Locals are tracked so that `local ATT = 1`
    style era/team indices resolve.
    """
    if out is None:
        out = []
    # In Lua 4.0 the value stack IS the register file: a function's locals
    # occupy slots 0..n-1 of its own frame, so `local ATT = 1` is just the
    # PUSHINT that leaves 1 in slot 2 and GETLOCAL 2 reads it back.
    stack = []

    def push(v):
        stack.append(v)

    def pop(n=1):
        if n:
            del stack[max(0, len(stack) - n):]

    for ins in f.code:
        op, u, s, b, a = _dec40(ins)
        if op == "PUSHINT":
            push(float(s))
        elif op == "PUSHSTRING":
            push(f.kstr[u])
        elif op == "PUSHNUM":
            push(f.knum[u])
        elif op == "PUSHNEGNUM":
            push(-f.knum[u])
        elif op == "PUSHNIL":
            for _ in range(u):
                push(None)
        elif op == "PUSHNILJMP":
            push(None)
        elif op == "GETGLOBAL":
            push(Unknown("global:" + f.kstr[u]))
        elif op == "GETLOCAL":
            push(stack[u] if u < len(stack) else Unknown("local%d" % u))
        elif op == "SETLOCAL":
            v = stack.pop() if stack else Unknown()
            while len(stack) <= u:
                stack.append(Unknown("slot"))
            stack[u] = v
        elif op == "POP":
            pop(u)
        elif op == "CALL":
            # `a` is the stack slot of the function; args are above it.
            fn = stack[a] if a < len(stack) else Unknown()
            args = stack[a + 1:]
            del stack[a:]
            name = fn.what[7:] if isinstance(fn, Unknown) and \
                fn.what.startswith("global:") else "?"
            out.append(Call(name, args, f))
            for _ in range(b):
                push(Unknown("ret"))
        elif op in ("TAILCALL", "RETURN", "END"):
            pass
        elif op == "CREATETABLE":
            push(Unknown("table"))
        elif op == "CLOSURE":
            pop(b)
            push(Unknown("closure"))
        elif op == "PUSHUPVALUE":
            push(Unknown("upval"))
        elif op == "PUSHSELF":
            push(Unknown("self"))
        elif op == "GETTABLE":
            pop(2)
            push(Unknown("index"))
        elif op in ("GETDOTTED", "GETINDEXED", "MINUS", "NOT"):
            pop(1)
            push(Unknown(op))
        elif op == "SETGLOBAL":
            pop(1)
        elif op == "SETTABLE":
            pop(b)
        elif op == "SETLIST":
            pop(b)
        elif op == "SETMAP":
            pop(2 * u)
        elif op == "CONCAT":
            pop(u)
            push(Unknown("concat"))
        elif op in ("ADD", "SUB", "MULT", "DIV", "POW"):
            pop(2)
            push(Unknown("arith"))
        elif op == "ADDI":
            pop(1)
            push(Unknown("arith"))
        elif op in ("JMPNE", "JMPEQ", "JMPLT", "JMPLE", "JMPGT", "JMPGE"):
            pop(2)
        elif op in ("JMPT", "JMPF"):
            pop(1)
        # JMP / JMPONT / JMPONF / FOR* leave the depth alone for our purposes.
    for p in f.protos:
        calls40(p, out)
    return out


# ===========================================================  Lua 5.0 undump

OPS50 = """MOVE LOADK LOADBOOL LOADNIL GETUPVAL GETGLOBAL GETTABLE SETGLOBAL
SETUPVAL SETTABLE NEWTABLE SELF ADD SUB MUL DIV POW UNM NOT CONCAT JMP EQ LT
LE TEST CALL TAILCALL RETURN FORLOOP TFORLOOP TFORPREP SETLIST SETLISTO CLOSE
CLOSURE""".split()


def undump50(body):
    r = Reader(body, 18)  # 4 sig + version + 9 size bytes + 4-byte test number
    return _func50(r, None)


def _func50(r, parent_source):
    f = Proto()
    f.source = r.string() or parent_source
    r.int()          # lineDefined
    f.nups = r.byte()
    r.byte()         # numparams
    r.byte()         # is_vararg
    r.byte()         # maxstacksize
    for _ in range(r.int()):        # line info
        r.int()
    for _ in range(r.int()):        # locals
        r.string()
        r.int()
        r.int()
    for _ in range(r.int()):        # upvalues
        r.string()
    n = r.int()
    for _ in range(n):
        t = r.byte()
        if t == 3:      # LUA_TNUMBER
            f.k.append(r.num())
        elif t == 4:    # LUA_TSTRING
            f.k.append(r.string())
        elif t == 1:    # LUA_TBOOLEAN
            f.k.append(bool(r.byte()))
        else:           # LUA_TNIL
            f.k.append(None)
    f.protos = [_func50(r, f.source) for _ in range(r.int())]
    n = r.int()
    raw = r.block(n * 4)
    f.code = list(struct.unpack_from("<%dI" % n, raw, 0)) if n else []
    return f


def walk(f):
    yield f
    for p in f.protos:
        yield from walk(p)


# Lua 5.0 splits the RK operand at MAXSTACK; stock 5.0.2 uses 250, but the PS2
# build was RE'd as a 128-register build (analysis/re-tickets-spawn.md §1.3,
# "RK-base 128 build of Lua 5.0.2").  `_rk_base` picks whichever base keeps
# every RK constant index inside the script's own constant table, so a wrong
# guess cannot silently mis-decode.
def _rk_base(f):
    for base in (128, 250, 256):
        ok = True
        for p in walk(f):
            n = len(p.k)
            for ins in p.code:
                op = OPS50[ins & 0x3F]
                if op not in ("SETTABLE", "GETTABLE", "SELF", "ADD", "SUB",
                              "MUL", "DIV", "POW", "EQ", "LT", "LE"):
                    continue
                for v in ((ins >> 15) & 0x1FF, (ins >> 6) & 0x1FF):
                    if v >= base and v - base >= n:
                        ok = False
                        break
                if not ok:
                    break
            if not ok:
                break
        if ok:
            return base
    return 128


class Table(dict):
    pass


def _snapshot_tables(value, memo=None):
    """Copy literal VM state while retaining opaque closure/prototype identity."""
    if memo is None:
        memo = {}
    if not isinstance(value, (dict, list)):
        return value
    if id(value) in memo:
        return memo[id(value)]
    result = type(value)()
    memo[id(value)] = result
    if isinstance(value, dict):
        result.update((key, _snapshot_tables(child, memo)) for key, child in value.items())
    else:
        result.extend(_snapshot_tables(child, memo) for child in value)
    return result


def calls50(f, out=None, globs=None, base=None, ups=None, recurse=True,
            isolate_prototypes=False):
    """Abstract-interpret a Lua 5.0 proto tree and yield its engine calls.

    Enough of the VM to follow the mission scripts: constants, registers,
    globals, table constructors (so `SetupTeams{ all = { units = 32 } }`
    resolves) and calls.  Control flow is ignored -- the scripts are straight
    line and the fall-through order is the source order.
    """
    if out is None:
        out = []
    if globs is None:
        globs = {}
    if base is None:
        base = _rk_base(f)
    if ups is None:
        ups = []
    R = {}
    # Which parent values each nested closure captures.  BF2 mission scripts
    # put `ALL = 2` style era indices in the chunk's locals and read them from
    # ScriptInit as upvalues, so `SetupTeams{ all = { team = ALL } }` only
    # resolves if the CLOSURE pseudo-instructions are followed.
    closure_ups = {}

    def rk(x):
        if x >= base:
            i = x - base
            return f.k[i] if i < len(f.k) else Unknown("k%d" % i)
        return R.get(x, Unknown("r%d" % x))

    skip = 0
    for idx, ins in enumerate(f.code):
        if skip:
            skip -= 1
            continue
        # Lua 5.0 field layout (lopcodes.h): OP:0..5, C:6..14, B:15..23,
        # A:24..31, Bx = C|B.  (5.1 later reordered these to A,C,B.)
        op = OPS50[ins & 0x3F]
        a = (ins >> 24) & 0xFF
        c = (ins >> 6) & 0x1FF
        b = (ins >> 15) & 0x1FF
        bx = (ins >> 6) & 0x3FFFF
        if op == "LOADK":
            R[a] = f.k[bx]
        elif op == "MOVE":
            R[a] = R.get(b, Unknown("r%d" % b))
        elif op == "LOADNIL":
            for i in range(a, b + 1):
                R[i] = None
        elif op == "LOADBOOL":
            R[a] = bool(b)
        elif op == "GETGLOBAL":
            name = f.k[bx]
            R[a] = globs.get(name, Unknown("global:" + str(name)))
        elif op == "SETGLOBAL":
            globs[f.k[bx]] = R.get(a, Unknown("r%d" % a))
        elif op == "NEWTABLE":
            R[a] = Table()
        elif op == "SETTABLE":
            t = R.get(a)
            k, v = rk(b), rk(c)
            if isinstance(t, Table) and isinstance(k, (str, float, int)):
                t[k] = v
        elif op in ("SETLIST", "SETLISTO"):
            t = R.get(a)
            if isinstance(t, Table):
                n = bx % 32 + 1
                start = bx - bx % 32
                for i in range(1, n + 1):
                    t[float(start + i)] = R.get(a + i, Unknown())
        elif op == "GETTABLE":
            t = R.get(b)
            k = rk(c)
            R[a] = t.get(k, Unknown("field")) if isinstance(t, Table) else \
                Unknown("field:%s" % k)
        elif op == "SELF":
            R[a + 1] = R.get(b, Unknown("r%d" % b))
            recv = R.get(b)
            meth = rk(c)
            name = recv.what[7:] if isinstance(recv, Unknown) and \
                recv.what.startswith("global:") else "?"
            R[a] = Unknown("global:%s:%s" % (name, meth))
        elif op == "CLOSURE":
            R[a] = Closure(f.protos[bx]) if bx < len(f.protos) \
                else Unknown("closure")
            n = f.protos[bx].nups if bx < len(f.protos) else 0
            caps = []
            for j in range(n):
                if idx + 1 + j >= len(f.code):
                    break
                q = f.code[idx + 1 + j]
                qop = OPS50[q & 0x3F]
                qb = (q >> 15) & 0x1FF
                if qop == "MOVE":
                    caps.append(R.get(qb, Unknown("r%d" % qb)))
                elif qop == "GETUPVAL":
                    caps.append(ups[qb] if qb < len(ups) else Unknown("upval"))
                else:
                    caps.append(Unknown("upval"))
            closure_ups[bx] = caps
            skip = n
        elif op == "CALL":
            fn = R.get(a, Unknown())
            nargs = (max(R) + 1 - (a + 1)) if b == 0 and R else b - 1
            args = [R.get(a + 1 + i, Unknown()) for i in range(max(0, nargs))]
            nm = "?"
            if isinstance(fn, Unknown) and fn.what.startswith("global:"):
                nm = fn.what[7:]
            out.append(Call(nm, args, f))
            for i in range(max(0, c - 1)):
                R[a + i] = Unknown("ret")
        elif op == "GETUPVAL":
            R[a] = ups[b] if b < len(ups) else Unknown("upval")
        elif op in ("ADD", "SUB", "MUL", "DIV", "POW"):
            x, y = rk(b), rk(c)
            R[a] = Unknown("arith")
            if isinstance(x, float) and isinstance(y, float):
                try:
                    if op == "ADD":
                        R[a] = x + y
                    elif op == "SUB":
                        R[a] = x - y
                    elif op == "MUL":
                        R[a] = x * y
                    elif op == "DIV":
                        R[a] = x / y if y else 0.0
                    else:
                        R[a] = x ** y
                except (OverflowError, ZeroDivisionError):
                    pass
        elif op in ("UNM", "NOT", "CONCAT"):
            R[a] = Unknown(op)
    if recurse:
        for i, p in enumerate(f.protos):
            # A census must not let a dormant sibling callback mutate the
            # next prototype's globals. Preserve historical bake behavior unless
            # the caller explicitly requests independent declaration snapshots.
            child_globals, child_ups = globs, closure_ups.get(i, [])
            if isolate_prototypes:
                child_globals, child_ups = _snapshot_tables([child_globals, child_ups])
            calls50(p, out, child_globals, base, child_ups,
                    isolate_prototypes=isolate_prototypes)
    return out


def global_functions50(f):
    """Global closures registered by the chunk, in instruction order."""
    top = {}
    reg = {}
    for ins in f.code:
        op = OPS50[ins & 0x3F]
        a, bx = (ins >> 24) & 0xFF, (ins >> 6) & 0x3FFFF
        if op == "CLOSURE" and bx < len(f.protos):
            reg[a] = f.protos[bx]
        elif op == "SETGLOBAL" and a in reg and isinstance(f.k[bx], str):
            top[f.k[bx]] = reg.pop(a)
    return top


def startup_calls50(f, verbs):
    """Replay startup verb calls, following only zero-argument globals."""
    top = global_functions50(f)
    base = _rk_base(f)
    out = []

    def run(name, depth):
        p = top.get(name)
        if p is None or depth > 4:
            return
        for c in calls50(p, base=base, recurse=False):
            if c.name in verbs:
                for arg in c.args:
                    if isinstance(arg, str):
                        out.append((arg, c.name == verbs[0]))
            elif c.name in top and not c.args:
                run(c.name, depth + 1)

    run("ScriptPostLoad", 0)
    return out


ARC_VERBS = ("BlockPlanningGraphArcs", "UnblockPlanningGraphArcs")


def arc_calls50(f):
    """The `plnp` group calls a BF2 mission makes as it starts, in order.

    `ScriptPostLoad`'s own calls, following the calls it makes into the
    script's other global functions (DEA1's `PlayAnimExtend()` unblocks
    `Connection122` that way) -- but not the event callbacks it only
    REGISTERS (`OnObjectKillName(CompactorConnectionOn, ...)` passes the
    function, it does not call it).  Each is `(arc name, True = block)`;
    `UnblockPlanningGraphArcs` of a group that is not blocked is a no-op on
    the PS2 (the mask starts at 0, `core::route_patch`), but it is kept so the
    list replays the script.
    """
    return startup_calls50(f, ARC_VERBS)


BARRIER_VERBS = ("EnableBarriers", "DisableBarriers")


def barrier_calls50(f):
    """The barriers a BF2 mission opens and closes as it starts, in order.

    `ScriptPostLoad`'s `EnableBarriers(name)` / `DisableBarriers(name)`, with
    the calls it makes into the script's own global functions -- the same
    walk as `arc_calls50`.  Each is `(barrier name, True = enabled)`.  Every
    barrier LOADS enabled (analysis/RE_LOG.md "Barriers are the PS2's
    Obstacles": the BF2 loader ORs the enabled bit in), so a `DisableBarriers`
    here is what opens a door's barrier for the round (BF2 KAM1's FRONTDOOR*,
    `camp`, `frog`).  64 BF2 mission scripts call one; no BF1 script does.
    """
    return startup_calls50(f, BARRIER_VERBS)


def death_drains50(f):
    """The unconditional per-death ticket drains a BF2 mission registers.

    `ScriptPostLoad`'s `OnCharacterDeathTeam(handler, team)` whose handler's
    whole body is `AddReinforcements(<team>, -<n>)` with literals -- DEA1's
    conquest scripts (analysis/re-time-limit.md §6 [V][O]).  This runs ON
    TOP of `Objective:Start`'s own ticket per death.  Each is `(Lua team of
    the dying character, Lua team drained, n)`; a handler with anything else
    in it (a condition, another call) is not a plain drain and is skipped.
    """
    top = global_functions50(f)
    p = top.get("ScriptPostLoad")
    if p is None:
        return []
    base = _rk_base(f)
    out = []
    for c in calls50(p, base=base, recurse=False):
        if c.name != "OnCharacterDeathTeam" or len(c.args) < 2 or \
                not isinstance(c.args[0], Closure) or \
                not isinstance(c.args[1], float):
            continue
        body = calls50(c.args[0].proto, base=base, recurse=False)
        # A conditional body shows up as extra instructions, not extra calls:
        # demand straight-line code (no jumps or tests) as well.
        branchy = any(OPS50[i & 0x3F] in ("JMP", "EQ", "LT", "LE", "TEST",
                                          "FORLOOP", "TFORLOOP")
                      for i in c.args[0].proto.code)
        if branchy or len(body) != 1 or body[0].name != "AddReinforcements":
            continue
        ba = body[0].args
        if len(ba) == 2 and isinstance(ba[0], float) and \
                isinstance(ba[1], float) and ba[1] < 0:
            out.append((int(c.args[1]), int(ba[0]), int(-ba[1])))
    return out


def consts(f, want=None):
    """Every constant in a proto tree, optionally filtered by python type."""
    out = []
    for p in walk(f):
        for c in (p.k or []) + list(p.kstr) + list(p.knum):
            if want is None or isinstance(c, want):
                out.append(c)
    return out


def load_ps2(game, script):
    path = os.path.join(PS2_BF1 if game == "bf1" else PS2_BF2, script + ".luac")
    if not os.path.exists(path):
        return None
    with open(path, "rb") as fh:
        body = fh.read()
    if body[:4] != b"\x1bLua":
        return None
    return undump40(body) if body[4] == 0x40 else undump50(body)


# ===========================================================  PC .lua scanning

FACTION = {
    "alliance": "all", "empire": "imp", "republic": "rep", "cis": "cis",
    "all": "all", "imp": "imp", "rep": "rep", "cis_": "cis",
    "locals": "locals", "local": "locals", "gungans": "locals",
    "naboo guard": "locals", "wookie": "locals",
}


def strip_comments(src):
    """Drop Lua line comments so commented-out calls are never harvested."""
    out = []
    for line in src.splitlines():
        i = line.find("--")
        out.append(line if i < 0 else line[:i])
    return "\n".join(out)


def num(tok):
    tok = tok.strip()
    return float(tok)


# `AddUnitClass(team, "class", count)` -- the count is the team's MINIMUM of
# that class (re-tickets-spawn.md 1.5: the counts do not sum to the member
# pool).  A few PC sources omit it; the engine's own default there is one.
ADD_UNIT_CLASS = re.compile(
    r"AddUnitClass\s*\(\s*([^,]+),\s*\"([^\"]+)\"\s*(?:,\s*([-\d.]+))?")


def count_of(tok, default=1):
    """A unit-class count token as an int (`None`/absent -> `default`)."""
    if tok is None:
        return default
    try:
        return int(num(tok))
    except (TypeError, ValueError):
        return default


def scan_bf1(src):
    """Extract the conquest parameters from a BF1 mission .lua."""
    s = strip_comments(src)
    env = {}
    for m in re.finditer(r"^\s*(?:local\s+)?([A-Z]{2,4})\s*=\s*(\d+)\s*;?\s*$", s, re.M):
        env[m.group(1)] = int(m.group(2))

    def team(tok):
        tok = tok.strip()
        if tok.isdigit():
            return int(tok)
        return env.get(tok)

    d = {"units": {}, "tickets": {}, "names": {}, "death_regions": [],
         "attacking": None, "spawn": None, "classes": {}, "layers": []}
    for m in re.finditer(r"SetTeamName\s*\(\s*([^,]+),\s*\"([^\"]*)\"", s):
        t = team(m.group(1))
        if t:
            d["names"][t] = FACTION.get(m.group(2).strip().lower(), m.group(2))
    for m in re.finditer(r"SetUnitCount\s*\(\s*([^,]+),\s*([-\d.]+)", s):
        t = team(m.group(1))
        if t:
            d["units"][t] = int(num(m.group(2)))
    for m in re.finditer(r"SetReinforcementCount\s*\(\s*([^,]+),\s*([-\d.]+)", s):
        t = team(m.group(1))
        if t:
            d["tickets"][t] = int(num(m.group(2)))
    for m in re.finditer(ADD_UNIT_CLASS, s):
        t = team(m.group(1))
        if t:
            d["classes"].setdefault(t, []).append(
                (m.group(2), count_of(m.group(3)), None))
    m = re.search(r"SetAttackingTeam\s*\(\s*([^)]+)\)", s)
    if m:
        d["attacking"] = team(m.group(1))
    m = re.search(r"SetSpawnDelay\s*\(\s*([-\d.]+)\s*,\s*([-\d.]+)", s)
    if m:
        d["spawn"] = (num(m.group(1)), num(m.group(2)))
    d["death_regions"] = re.findall(r"AddDeathRegion\s*\(\s*\"([^\"]+)\"", s)
    for m in re.finditer(r"ReadDataFile\s*\(\s*\"[A-Za-z]+\\\\[a-z0-9]+\.lvl\"\s*,"
                         r"\s*\"([^\"]+)\"", s):
        d["layers"].append(m.group(1))
    return d


SETUP_KEY = re.compile(r"(\w+)\s*=\s*\{", re.S)


def scan_bf2(src):
    """Extract the conquest parameters from a BF2 <map><era>_con .lua."""
    s = strip_comments(src)
    env = {}
    for m in re.finditer(r"^\s*(?:local\s+)?([A-Z]{2,4})\s*=\s*(\d+)\s*;?\s*$", s, re.M):
        env[m.group(1)] = int(m.group(2))

    def team(tok):
        tok = tok.strip()
        if tok.isdigit():
            return int(tok)
        return env.get(tok)

    d = {"units": {}, "tickets": {}, "names": {}, "death_regions": [],
         "attacking": None, "spawn": None, "classes": {}, "cps": [],
         "posts": [], "killed": [], "layers": [], "numbots": None}

    # SetupTeams{ <faction> = { team = X, units = N, reinforcements = M, ... } }
    # Some missions (mus1c_con) call it once per side, so take every call.
    for setup in re.finditer(r"SetupTeams\s*\{", s):
        body = _balanced(s, setup.end() - 1)
        for km in SETUP_KEY.finditer(body):
            side = _balanced(body, km.end() - 1)
            tm = re.search(r"\bteam\s*=\s*(\w+)", side)
            un = re.search(r"\bunits\s*=\s*([-\d.]+)", side)
            rf = re.search(r"\breinforcements\s*=\s*([-\d.]+)", side)
            if not tm:
                continue
            t = team(tm.group(1))
            if not t:
                continue
            d["names"][t] = FACTION.get(km.group(1).strip().lower(), km.group(1))
            if un:
                d["units"][t] = int(num(un.group(1)))
            if rf:
                d["tickets"][t] = int(num(rf.group(1)))
            for cm in re.finditer(
                    r"=\s*\{\s*\"([^\"]+)\"\s*(?:,\s*([-\d.]+)\s*"
                    r"(?:,\s*([-\d.]+))?)?", side):
                d["classes"].setdefault(t, []).append(
                    (cm.group(1), count_of(cm.group(2)),
                     None if cm.group(3) is None else count_of(cm.group(3))))

    # A locals team, when present, is still set up the BF1 way.
    for m in re.finditer(r"SetTeamName\s*\(\s*([^,]+),\s*\"([^\"]*)\"", s):
        t = team(m.group(1))
        if t and t not in d["names"]:
            d["names"][t] = FACTION.get(m.group(2).strip().lower(), m.group(2))
    for m in re.finditer(r"SetUnitCount\s*\(\s*([^,]+),\s*([-\d.]+)", s):
        t = team(m.group(1))
        if t and t not in d["units"]:
            d["units"][t] = int(num(m.group(2)))
    for m in re.finditer(ADD_UNIT_CLASS, s):
        t = team(m.group(1))
        if t:
            d["classes"].setdefault(t, []).append(
                (m.group(2), count_of(m.group(3)), None))

    m = re.search(r"SetAttackingTeam\s*\(\s*([^)]+)\)", s)
    if m:
        d["attacking"] = team(m.group(1))
    m = re.search(r"SetSpawnDelay\s*\(\s*([-\d.]+)\s*,\s*([-\d.]+)", s)
    if m:
        d["spawn"] = (num(m.group(1)), num(m.group(2)))
    m = re.search(r"ScriptCB_SetNumBots\s*\(\s*(\d+)", s)
    if m:
        d["numbots"] = int(m.group(1))
    d["death_regions"] = re.findall(r"AddDeathRegion\s*\(\s*\"([^\"]+)\"", s)

    # --- per-command-post initial ownership (the two mechanisms in the corpus)
    for m in re.finditer(r"SetProperty\s*\(\s*\"([^\"]+)\"\s*,\s*\"[Tt]eam\"\s*,"
                         r"\s*\"?(\d+)\"?", s):
        d["cps"].append((m.group(1), int(m.group(2))))
    for m in re.finditer(r"SetObjectTeam\s*\(\s*\"([^\"]+)\"\s*,\s*(\w+)", s):
        t = team(m.group(2))
        if t is not None:
            d["cps"].append((m.group(1), t))
    d["posts"] = re.findall(r"CommandPost:New\s*\{\s*name\s*=\s*\"([^\"]+)\"", s)
    d["killed"] = re.findall(r"KillObject\s*\(\s*\"([^\"]+)\"", s)
    for m in re.finditer(r"ReadDataFile\s*\(\s*\"[A-Za-z]+\\\\[a-z0-9]+\.lvl\"\s*,"
                         r"\s*\"([^\"]+)\"", s):
        d["layers"].append(m.group(1))
    return d


def _balanced(s, open_idx):
    """Return the text inside the {...} whose '{' sits at open_idx."""
    depth, i = 0, open_idx
    while i < len(s):
        if s[i] == "{":
            depth += 1
        elif s[i] == "}":
            depth -= 1
            if depth == 0:
                return s[open_idx + 1:i]
        i += 1
    return s[open_idx + 1:]


# ===========================================================  the map table

# key -> (source_game, planet dir, {era: (pc file stem, ps2 script)})
# BF1 mission names encode the ATTACKING faction, not the mode: `a`=Alliance
# and `i`=Imperial are the GCW pairing, `r`=Republic and `c`=CIS the Clone
# Wars one.  Every non-`_h` BF1 mission is a conquest mission (the `_h`
# twins are the historical-campaign variants; `SetHistorical` and mission
# objectives are what separate them), so the "which one is conquest" choice
# is really "which era", and both are emitted.
MAPS = [
    # ---- BF1
    ("tat1",     "bf1", "TAT", {"gcw": ("tat1i", "tat1i"), "cw": ("tat1r", "tat1r")}),
    ("bf1_tat2", "bf1", "TAT", {"gcw": ("tat2i", "tat2i"), "cw": ("tat2r", "tat2r")}),
    ("bf1_bes1", "bf1", "BES", {"gcw": ("bes1a", "bes1a"), "cw": ("bes1r", "bes1r")}),
    ("bf1_bes2", "bf1", "BES", {"gcw": ("bes2a", "bes2a"), "cw": ("bes2r", "bes2r")}),
    ("bf1_end1", "bf1", "END", {"gcw": ("end1a", "end1a")}),
    ("bf1_geo1", "bf1", "GEO", {"cw": ("GEO1r", "geo1r")}),
    ("bf1_hot1", "bf1", "HOT", {"gcw": ("hot1i", "hot1i")}),
    ("bf1_kam1", "bf1", "KAM", {"cw": ("kam1c", "kam1c")}),
    ("bf1_kas1", "bf1", "KAS", {"gcw": ("kas1i", "kas1i"), "cw": ("kas1c", "kas1c")}),
    ("bf1_kas2", "bf1", "KAS", {"gcw": ("kas2i", "kas2i"), "cw": ("kas2c", "kas2c")}),
    ("bf1_nab1", "bf1", "NAB", {"gcw": ("NAB1i", "nab1i"), "cw": ("NAB1c", "nab1c")}),
    ("bf1_nab2", "bf1", "NAB", {"gcw": ("nab2a", "nab2a"), "cw": ("nab2c", "nab2c")}),
    ("bf1_rhn1", "bf1", "RHN", {"gcw": ("rhn1i", "rhn1i"), "cw": ("rhn1r", "rhn1r")}),
    ("bf1_rhn2", "bf1", "RHN", {"gcw": ("rhn2a", "rhn2a"), "cw": ("rhn2c", "rhn2c")}),
    ("bf1_yav1", "bf1", "YAV", {"gcw": ("yav1i", "yav1i"), "cw": ("yav1c", "yav1c")}),
    ("bf1_yav2", "bf1", "YAV", {"gcw": ("yav2i", "yav2i"), "cw": ("yav2r", "yav2r")}),
    # ---- BF2 (`g` = galactic civil war, `c` = clone wars)
    ("tat2",     "bf2", "tat", {"gcw": ("tat2g_con", "tat2g_con"),
                                "cw": ("tat2c_con", "tat2c_con")}),
    ("bf2_cor1", "bf2", "COR", {"gcw": ("cor1g_con", "cor1g_con"),
                                "cw": ("cor1c_con", "cor1c_con")}),
    ("bf2_dag1", "bf2", "DAG", {"gcw": ("dag1g_con", "dag1g_con"),
                                "cw": ("dag1c_con", "dag1c_con")}),
    ("bf2_dea1", "bf2", "DEA", {"gcw": ("dea1g_con", "dea1g_con"),
                                "cw": ("dea1c_con", "dea1c_con")}),
    ("bf2_end1", "bf2", "end", {"gcw": ("end1g_con", "end1g_con")}),
    ("bf2_fel1", "bf2", "FEL", {"gcw": ("fel1g_con", "fel1g_con"),
                                "cw": ("fel1c_con", "fel1c_con")}),
    ("bf2_geo1", "bf2", "geo", {"cw": ("geo1c_con", "geo1c_con")}),
    ("bf2_hot1", "bf2", "hot", {"gcw": ("hot1g_con", "hot1g_con")}),
    ("bf2_kam1", "bf2", "kam", {"gcw": ("KAM1g_con", "kam1g_con"),
                                "cw": ("kam1c_con", "kam1c_con")}),
    ("bf2_kas2", "bf2", "KAS", {"gcw": ("kas2g_con", "kas2g_con"),
                                "cw": ("kas2c_con", "kas2c_con")}),
    ("bf2_mus1", "bf2", "MUS", {"gcw": ("mus1g_con", "mus1g_con"),
                                "cw": ("mus1c_con", "mus1c_con")}),
    ("bf2_myg1", "bf2", "MYG", {"gcw": ("myg1g_con", "myg1g_con"),
                                "cw": ("myg1c_con", "myg1c_con")}),
    ("bf2_nab2", "bf2", "NAB", {"gcw": ("nab2g_con", "nab2g_con"),
                                "cw": ("nab2c_con", "nab2c_con")}),
    ("bf2_pol1", "bf2", "POL", {"gcw": ("pol1g_con", "pol1g_con"),
                                "cw": ("pol1c_con", "pol1c_con")}),
    ("bf2_tan1", "bf2", "TAN", {"gcw": ("tan1g_con", "tan1g_con"),
                                "cw": ("tan1c_con", "tan1c_con")}),
    ("bf2_tat3", "bf2", "tat", {"gcw": ("tat3g_con", "tat3g_con"),
                                "cw": ("tat3c_con", "tat3c_con")}),
    ("bf2_uta1", "bf2", "UTA", {"gcw": ("uta1g_con", "uta1g_con"),
                                "cw": ("uta1c_con", "uta1c_con")}),
    ("bf2_yav1", "bf2", "YAV", {"gcw": ("yav1g_con", "yav1g_con"),
                                "cw": ("yav1c_con", "yav1c_con")}),
]

PRETTY = {
    "tat1": "Tatooine: Dune Sea", "bf1_tat2": "Tatooine: Mos Eisley",
    "bf1_bes1": "Bespin: Platforms",
    "bf1_bes2": "Bespin: Cloud City", "bf1_end1": "Endor: Bunker",
    "bf1_geo1": "Geonosis: Spire", "bf1_hot1": "Hoth: Echo Base",
    "bf1_kam1": "Kamino: Tipoca City", "bf1_kas1": "Kashyyyk: Islands",
    "bf1_kas2": "Kashyyyk: Docks", "bf1_nab1": "Naboo: Plains",
    "bf1_nab2": "Naboo: Theed", "bf1_rhn1": "Rhen Var: Harbor",
    "bf1_rhn2": "Rhen Var: Citadel", "bf1_yav1": "Yavin 4: Temple",
    "bf1_yav2": "Yavin 4: Arena",
    "tat2": "Tatooine: Mos Eisley", "bf2_cor1": "Coruscant: Jedi Temple",
    "bf2_tat3": "Tatooine: Jabba's Palace",
    "bf2_dag1": "Dagobah: Swamp", "bf2_dea1": "Death Star: Interior",
    "bf2_end1": "Endor: Bunker", "bf2_fel1": "Felucia: Marshland",
    "bf2_geo1": "Geonosis: Dust Plains", "bf2_hot1": "Hoth: Echo Base",
    "bf2_kam1": "Kamino: Tipoca City", "bf2_kas2": "Kashyyyk: Docks",
    "bf2_mus1": "Mustafar: Refinery", "bf2_myg1": "Mygeeto: War-Torn City",
    "bf2_nab2": "Naboo: Theed", "bf2_pol1": "Polis Massa: Medical Facility",
    "bf2_tan1": "Tantive IV: Interior", "bf2_uta1": "Utapau: Sinkhole",
    "bf2_yav1": "Yavin 4: Temple",
}

ERA_NAME = {"gcw": "GCW", "cw": "CW"}




# ===========================================================  build + emit

def facts_from_calls(seq):
    """Fold a reconstructed PS2 call sequence into mission facts.

    Both games drive the same engine API from the mission script; BF2 just
    routes the per-team numbers through the shared `SetupTeams` helper
    (setup_teams.lua: `SetTeamName(team, lower(key))`, `SetUnitCount`,
    `SetReinforcementCount`), so the side table's key IS the faction name.
    """
    d = {"units": {}, "tickets": {}, "names": {}, "classes": {},
         "death_regions": [], "attacking": None, "spawn": None,
         "cps": [], "posts": [], "killed": [], "layers": [], "numbots": None,
         "vo_bleed": [], "vo_low": [], "vo_oob": [],
         "mus_amb": [], "mus_vic": [], "mus_def": [], "sound": [],
         "shots": [], "ai_view": None, "ai_aggr": {}, "ai_snipe": None,
         "ai_capture": {}, "ai_damage": [], "fly": None,
         "bird_count": 0, "bird_types": {}, "heroes": {}, "class_load_order": []}

    def ti(v):
        return int(v) if isinstance(v, float) and v == int(v) else None

    for c in seq:
        a = c.args
        if c.name == "SetHeroClass" and len(a) >= 2 and ti(a[0]) is not None and isinstance(a[1], str):
            d["heroes"][ti(a[0])] = a[1]
        elif c.name == "SetNumBirdTypes" and a and ti(a[0]) is not None:
            d["bird_count"] = min(3, max(0, ti(a[0])))
        elif c.name == "SetBirdType" and len(a) >= 3 and ti(a[0]) is not None and isinstance(a[2], str):
            d["bird_types"][ti(a[0])] = (a[1], a[2])
        elif c.name == "SetTeamName" and len(a) >= 2 and isinstance(a[1], str):
            t = ti(a[0])
            if t:
                d["names"][t] = FACTION.get(a[1].strip().lower(), a[1])
        elif c.name == "SetUnitCount" and len(a) >= 2:
            t, n = ti(a[0]), ti(a[1])
            if t and n is not None:
                d["units"][t] = n
        elif c.name == "SetReinforcementCount" and len(a) >= 2:
            t, n = ti(a[0]), ti(a[1])
            if t and n is not None:
                d["tickets"][t] = n
        elif c.name == "AddUnitClass" and len(a) >= 2 and isinstance(a[1], str):
            t = ti(a[0])
            if t:
                n = ti(a[2]) if len(a) >= 3 else None
                d["classes"].setdefault(t, []).append(
                    (a[1], 1 if n is None else n, None))
        elif c.name == "SetupTeams" and a and isinstance(a[0], dict):
            for key, side in a[0].items():
                if not isinstance(side, dict):
                    continue
                t = ti(side.get("team"))
                if not t:
                    continue
                d["names"][t] = FACTION.get(str(key).strip().lower(), str(key))
                if ti(side.get("units")) is not None:
                    d["units"][t] = ti(side["units"])
                if ti(side.get("reinforcements")) is not None:
                    d["tickets"][t] = ti(side["reinforcements"])
                # Slot entries in TABLE ORDER (soldier, assault, engineer,
                # sniper, officer, special on the disc): {class, min, max}.
                for slot, ent in side.items():
                    if isinstance(ent, dict) and isinstance(ent.get(1.0), str):
                        mn, mx = ti(ent.get(2.0)), ti(ent.get(3.0))
                        d["classes"].setdefault(t, []).append(
                            (ent[1.0], 1 if mn is None else mn, mx))
        elif c.name == "SetSpawnDelay" and len(a) >= 2 and \
                isinstance(a[0], float) and isinstance(a[1], float):
            d["spawn"] = (a[0], a[1])
        elif c.name == "SetAttackingTeam" and a:
            d["attacking"] = ti(a[0])
        elif c.name == "AddDeathRegion" and a and isinstance(a[0], str):
            d["death_regions"].append(a[0])
        # The soldiers' fly-height ceiling (card JETP, analysis/re-jetpack.md
        # 5.3): BF2's SetMaxPlayerFlyHeight (B2:32250), the last call wins.
        # SetMaxFlyHeight is the AI flyers' and is not read.
        elif c.name == "SetMaxPlayerFlyHeight" and a and \
                isinstance(a[0], float):
            d["fly"] = a[0]
        elif c.name == "ScriptCB_SetNumBots" and a and ti(a[0]) is not None:
            d["numbots"] = ti(a[0])
        elif c.name == "SetProperty" and len(a) >= 3 and \
                isinstance(a[0], str) and isinstance(a[1], str) and \
                a[1].lower() == "team":
            v = a[2]
            t = int(v) if isinstance(v, str) and v.strip().isdigit() else ti(v)
            if t is not None:
                d["cps"].append((a[0], t))
        elif c.name == "SetObjectTeam" and len(a) >= 2 and isinstance(a[0], str):
            t = ti(a[1])
            if t is not None:
                d["cps"].append((a[0], t))
        elif c.name == "CommandPost:New" and len(a) >= 2 and \
                isinstance(a[1], dict) and isinstance(a[1].get("name"), str):
            d["posts"].append(a[1]["name"])
        elif c.name == "KillObject" and a and isinstance(a[0], str):
            d["killed"].append(a[0])
        # The AI knobs (card MAIS, analysis/re-ai-difficulty.md 3): the last
        # call wins, as each one is a plain store on the PS2.
        elif c.name == "SetAIViewMultiplier" and a and \
                isinstance(a[0], float):
            d["ai_view"] = a[0]
        elif c.name == "SetTeamAggressiveness" and len(a) >= 2 and \
                isinstance(a[1], float):
            t = ti(a[0])
            if t:
                d["ai_aggr"][t] = a[1]
        elif c.name == "AISnipeSuitabilityDist" and a and \
                isinstance(a[0], float):
            d["ai_snipe"] = a[0]
        elif c.name == "AICanCaptureCP" and len(a) >= 3 and \
                isinstance(a[0], str) and isinstance(a[2], bool):
            t = ti(a[1])
            if t:
                d["ai_capture"][(a[0], t)] = a[2]
        elif c.name == "SetAIDamageThreshold" and len(a) >= 2 and \
                isinstance(a[0], str):
            d["ai_damage"].append((a[0], a[1]))
        # The commander's lines (card ANNC, analysis/re-announcer.md 2.2-2.4):
        # the script's own team numbers and names, in call order.
        elif c.name == "SetBleedingVoiceOver" and len(a) >= 3 and \
                isinstance(a[2], str):
            l, t = ti(a[0]), ti(a[1])
            if l and t:
                d["vo_bleed"].append(
                    (l, t, a[2], bool(a[3]) if len(a) >= 4 else False))
        elif c.name == "SetLowReinforcementsVoiceOver" and len(a) >= 4 and \
                isinstance(a[2], str) and isinstance(a[3], float):
            l, t = ti(a[0]), ti(a[1])
            if l and t:
                d["vo_low"].append(
                    (l, t, a[2], a[3], bool(a[4]) if len(a) >= 5 else False))
        elif c.name == "SetOutOfBoundsVoiceOver" and len(a) >= 2 and \
                isinstance(a[1], str):
            t = ti(a[0])
            if t:
                d["vo_oob"].append((t, a[1]))
        # The music (card AMBM, analysis/re-ambience-music.md 3.1): the
        # script's own team numbers and names, in call order.
        elif c.name == "SetAmbientMusic" and len(a) >= 4 and \
                isinstance(a[1], float) and isinstance(a[2], str):
            t, st = ti(a[0]), ti(a[3])
            if t and st is not None:
                d["mus_amb"].append(
                    (t, a[1], a[2], st, bool(a[4]) if len(a) >= 5 else False))
        elif c.name in ("SetVictoryMusic", "SetDefeatMusic") and \
                len(a) >= 2 and isinstance(a[1], str):
            t = ti(a[0])
            if t:
                d["mus_vic" if c.name == "SetVictoryMusic"
                  else "mus_def"].append((t, a[1]))
        # The spawn screens' backdrops (card SSELB, re-spawn-flow 1.1):
        # AddCameraShot(qw, qx, qy, qz, x, y, z); the PS2 keeps the first 16
        # (B2:31916).
        elif c.name == "AddCameraShot" and len(a) >= 7 and \
                all(isinstance(v, float) for v in a[:7]):
            if len(d["shots"]) < 16:
                d["shots"].append(tuple(a[:7]))
        # Preserve SIDE requests independently of SetupTeams and hero/team
        # order: native shared animation definitions are first-loaded wins.
        # Keep original spelling and repeated requests, not a sorted roster.
        elif c.name == "ReadDataFile" and a and isinstance(a[0], str) and \
                a[0].lower().startswith("side\\"):
            filename = a[0].split("\\")[-1]
            if not filename.lower().endswith(".lvl"):
                raise ValueError("unsupported SIDE filename: %r" % a[0])
            side = filename[:-4]
            d["class_load_order"].extend((side, v) for v in a[1:]
                                         if isinstance(v, str))
        # The sound sub-levels the script loads (card SNDE):
        # ReadDataFile("sound\\kas.lvl;kas2gcw") -> ("kas", "kas2gcw").
        elif c.name == "ReadDataFile" and a and isinstance(a[0], str) and \
                re.match(r"(?i)sound\\(\w+)\.lvl;(\w+)$", a[0]):
            g = re.match(r"(?i)sound\\(\w+)\.lvl;(\w+)$", a[0])
            d["sound"].append((g.group(1).lower(), g.group(2).lower()))
        elif c.name == "ReadDataFile" and len(a) >= 2 and \
                isinstance(a[0], str) and isinstance(a[1], str) and \
                a[0].lower().endswith(".lvl") and "\\" in a[0] and \
                "side" not in a[0].lower() and "sound" not in a[0].lower():
            d["layers"].append(a[1])
    return d


class Mission:
    pass


def build(key, game, planet, era, pc_stem, ps2_name):
    """PS2 bytecode is the source of truth; the PC .lua is the referee.

    A6 proved the PC sources ARE the shipped scripts, but PS2 retail carries
    per-platform tuning (unit lists, member pools), and PLAN's rule is that
    where they differ the PS2 wins -- it is the target.  So every emitted
    number comes from the bytecode when the bytecode has it, and any PC/PS2
    difference is recorded and marked PROVISIONAL.
    """
    m = Mission()
    m.key, m.game, m.era, m.ps2 = key, game, era, ps2_name
    m.prov, m.dis = [], []

    if PS2_ONLY:
        m.pc, src = None, ""
    else:
        pc_dir = PC_BF1 if game == "bf1" else PC_BF2
        pc_path = os.path.join(pc_dir, planet, pc_stem + ".lua")
        m.pc = os.path.relpath(pc_path, ROOT)
        with open(pc_path, "r", errors="replace") as fh:
            src = fh.read()
    pc = scan_bf1(src) if game == "bf1" else scan_bf2(src)

    f = load_ps2(game, ps2_name)
    m.checked = f is not None
    if f is None:
        m.prov.append("every value (no PS2 script %s.luac to check against)"
                      % ps2_name)
        ps2 = None
        d = pc
    else:
        ps2 = facts_from_calls(calls40(f) if game == "bf1" else calls50(f))
        d = ps2
    m.d = d
    # The voice-over setters exist only in the bytecode scan (the PC scan
    # never read them); a mission with no PS2 script carries none.
    m.vo_bleed = (ps2 or {}).get("vo_bleed", [])
    m.vo_low = (ps2 or {}).get("vo_low", [])
    m.vo_oob = (ps2 or {}).get("vo_oob", [])
    m.mus_amb = (ps2 or {}).get("mus_amb", [])
    m.mus_vic = (ps2 or {}).get("mus_vic", [])
    m.mus_def = (ps2 or {}).get("mus_def", [])
    m.birds = [(ps2 or {}).get("bird_types", {}).get(i, (1.0, ""))
               for i in range((ps2 or {}).get("bird_count", 0))]
    m.sound = (ps2 or {}).get("sound", [])
    m.shots = (ps2 or {}).get("shots", [])
    m.heroes = sorted((ps2 or {}).get("heroes", {}).items()) if game == "bf2" else []
    m.class_load_order = (ps2 or {}).get("class_load_order", []) if game == "bf2" else []
    # Card MAIS: the script's AI knobs (the PS2 bytecode only).
    p2 = ps2 or {}
    m.ai_view = p2.get("ai_view")
    m.ai_aggr = sorted(p2.get("ai_aggr", {}).items())
    m.ai_snipe = p2.get("ai_snipe")
    m.ai_no_capture = [k for k, allow in p2.get("ai_capture", {}).items()
                       if not allow]
    # SetAIDamageThreshold's meaning is not known (re-ai-difficulty 3: read
    # only as a flag != 0); every conquest call writes the default, 0.
    # Anything else is not carried and is said so.
    for name, v in p2.get("ai_damage", []):
        if v != 0.0:
            m.prov.append("SetAIDamageThreshold(%r, %s) is not carried"
                          % (name, v))
    m.fly = (ps2 or {}).get("fly")
    m.strings = set(consts(f, str)) if f is not None else set()
    m.arc_blocks = arc_calls50(f) if f is not None and game == "bf2" else []
    m.barriers = barrier_calls50(f) if f is not None and game == "bf2" else []
    m.death_drains = death_drains50(f) if f is not None and game == "bf2" \
        else []

    def pick(field, t, label, default):
        """PS2 value if it has one, else the PC value; record disagreements."""
        pv = pc.get(field, {}).get(t)
        sv = (ps2 or {}).get(field, {}).get(t)
        if sv is not None and pv is not None and sv != pv:
            m.dis.append("team %d %s: PC %s vs PS2 %s (PS2 wins)"
                         % (t, label, pv, sv))
            return sv
        if sv is not None:
            return sv
        if pv is not None:
            if ps2 is not None:
                m.prov.append("team %d %s: only the PC source has it "
                              "(no such call in the PS2 script)" % (t, label))
            return pv
        m.prov.append("team %d %s: neither source has it; engine default used"
                      % (t, label))
        return default

    m.teams = []
    for t in (1, 2):
        fac = pick("names", t, "faction", None)
        if fac is None:
            fac = "unknown"
        m.teams.append((fac, pick("units", t, "units", 20 if game == "bf2" else 25),
                        pick("tickets", t, "tickets", 150 if game == "bf2" else 200)))

    # --- the per-team class MIX, in the order the script names it (BF1
    # AddUnitClass call order, BF2 SetupTeams slot-table order).  The counts
    # are MINIMUMS and do not sum to the member pool
    # (analysis/re-tickets-spawn.md 1.5); BF2 additionally carries a per-class
    # maximum.  Names are lowercased at emit time (the bytecode is mixed case:
    # "CIS_inf_rifleman").
    m.classes = [d["classes"].get(t, []) for t in (1, 2)]

    # --- locals team (3+), when the mission fields one.  Some missions
    # declare an empty team 3 and put the real locals on team 4 (BF1 hot1i:
    # team 3 "Local" with no units, team 4 = one imp_droid_probe), so take
    # the lowest team above the two mains that actually has a member pool.
    m.locals = None
    m.locals_team = None
    for t in sorted(k for k in set(list(d["names"]) + list(d["units"])) if k >= 3):
        if d["units"].get(t, 0) <= 0:
            continue
        cls = d["classes"].get(t, [])
        m.locals_team = t
        m.locals = (cls[0][0] if cls else str(d["names"].get(t, "locals")),
                    d["units"][t])
        break

    # --- alive-AI cap.  Censused: no BF1 mission .lua and no BF2 *_con .lua
    # calls ScriptCB_SetNumBots, so BF1's cap is the member pool itself
    # (re-tickets-spawn.md §2.5/§2.7) and BF2's is the 16 that
    # ObjectiveConquest:Start installs via ScriptCB_GetCONNumBots (§1.5).
    if d.get("numbots"):
        m.cap = d["numbots"]
    elif game == "bf1":
        m.cap = max(m.teams[0][1], m.teams[1][1])
    else:
        m.cap = 16

    for field, label, default in (("spawn", "spawn delay/interval", (10.0, 0.25)),
                                  ("attacking", "attacking team", None)):
        pv, sv = pc.get(field), (ps2 or {}).get(field)
        if sv is not None and pv is not None and sv != pv:
            m.dis.append("%s: PC %s vs PS2 %s (PS2 wins)" % (label, pv, sv))
        val = sv if sv is not None else pv
        if val is None and default is not None:
            m.prov.append("%s: neither source has it; mission default used"
                          % label)
            val = default
        setattr(m, "spawn" if field == "spawn" else "attacking", val)

    for field, label in (("death_regions", "death regions"), ("cps", "cp teams"),
                         ("posts", "command posts"), ("killed", "KillObject")):
        pv, sv = pc.get(field, []), (ps2 or {}).get(field, [])
        if ps2 is not None and not PS2_ONLY and \
                sorted(map(str, pv)) != sorted(map(str, sv)):
            m.dis.append("%s: PC %s vs PS2 %s (PS2 wins)" % (label, pv, sv))
        setattr(m, field, sv if ps2 is not None else pv)

    # --- (b) every string the RON carries must exist in the PS2 constant table
    m.missing = []
    for s in list(m.death_regions) + [c[0] for c in m.cps] + \
            list(m.posts) + list(m.killed) + \
            ([m.locals[0]] if m.locals else []):
        if m.checked and s not in m.strings:
            m.missing.append(s)
            m.prov.append("string %r is not in the PS2 %s constant table"
                          % (s, ps2_name))
    for d_ in m.dis:
        m.prov.append("PC/PS2 disagreement -- " + d_)
    return m


def fmtf(v):
    s = "%g" % v
    return s if ("." in s or "e" in s) else s + ".0"


def f32_text(x):
    """The shortest decimal that reads back as the same f32 (the bytecode's
    numbers are floats: 0.10000000149011612 is 0.1f)."""
    import struct
    f = struct.unpack("<f", struct.pack("<f", x))[0]
    for nd in range(1, 10):
        s = "%.*g" % (nd, f)
        if struct.unpack("<f", struct.pack("<f", float(s)))[0] == f:
            return s
    return repr(f)


def ron(m):
    pretty = PRETTY.get(m.key, m.key)
    L = []
    L.append("// %s %s — Conquest (%s). GENERATED by scripts/gen_missions.py:"
             % ("BF1" if m.game == "bf1" else "BF2", pretty, ERA_NAME[m.era]))
    L.append("// every value is read out of the PS2 mission bytecode (the")
    L.append("// target), with the PC mod-tools .lua of the same name as the")
    L.append("// readable referee (audit A6). Do not hand-edit — regenerate.")
    L.append("//   PC  : %s" % (m.pc or "(not read: --ps2-only)"))
    L.append("//   PS2 : extracted/%s/lua/mission/%s.luac%s"
             % (m.game, m.ps2, "" if m.checked else "   (ABSENT)"))
    if m.posts:
        L.append("//   conquest command posts (CommandPost:New order, all of")
        L.append("//   which the mission hands to ObjectiveConquest): %s"
                 % ", ".join(m.posts))
    if m.killed:
        L.append("//   the mission KillObject()s: %s" % ", ".join(m.killed))
    if m.d.get("layers"):
        L.append("//   world layer(s) the mission loads: %s"
                 % ", ".join(sorted(set(m.d["layers"]))))
    if m.prov:
        L.append("// PROVISIONAL:")
        for p in m.prov:
            L.append("//   - %s" % p)
    L.append("(")
    L.append('    map: "%s",' % m.key)
    L.append('    mode: "con",')
    L.append('    source_game: "%s",' % m.game)
    if m.heroes:
        L.append('    heroes: [%s],' % ", ".join(
            '(%d, "%s")' % (t, name) for t, name in m.heroes))
    if m.game == "bf2":
        L.append('    class_load_order: [%s],' % ", ".join(
            '(side: "%s", request: "%s")' % (side, request)
            for side, request in m.class_load_order))
    L.append("    teams: [")
    for (fac, units, tickets), cls in zip(m.teams, m.classes):
        if not cls:
            L.append('        (faction: "%s", units: %d, tickets: %d, '
                     'classes: []),' % (fac, units, tickets))
            continue
        L.append('        (faction: "%s", units: %d, tickets: %d, classes: ['
                 % (fac, units, tickets))
        for name, mn, mx in cls:
            L.append('            ("%s", %d, %s),'
                     % (name.lower(), mn,
                        "None" if mx is None else "Some(%d)" % mx))
        L.append("        ]),")
    L.append("    ],")
    L.append("    ai_alive_cap: %d," % m.cap)
    L.append("    spawn_delay: %s," % fmtf(m.spawn[0]))
    L.append("    spawn_interval: %s," % fmtf(m.spawn[1]))
    L.append('    era: Some("%s"),' % m.era)
    L.append("    attacking_team: %s,"
             % ("Some(%d)" % m.attacking if m.attacking else "None"))
    if m.cps:
        L.append("    cps: [")
        for name, team in m.cps:
            L.append('        ("%s", %d),' % (name, team))
        L.append("    ],")
    else:
        L.append("    cps: [],")
    L.append("    death_regions: [%s],"
             % ", ".join('"%s"' % r for r in m.death_regions))
    L.append("    posts: [%s]," % ", ".join('"%s"' % r for r in m.posts))
    L.append("    killed: [%s]," % ", ".join('"%s"' % r for r in m.killed))
    if m.fly is not None:
        L.append("    // SetMaxPlayerFlyHeight: the soldiers' fly-height "
                 "ceiling (world y).")
        L.append("    max_player_fly_height: Some(%s)," % fmtf(m.fly))
    if m.birds:
        L.append("    birds: [%s]," % ", ".join('(%s, "%s")' % (fmtf(size), name) for size, name in m.birds))
    if m.sound:
        L.append("    // The sound sub-levels the script loads, in order "
                 "(ReadDataFile")
        L.append("    // \"sound\\<lvl>.lvl;<sub-level>\"): the battle's "
                 "sound library.")
        L.append("    sound: [%s],"
                 % ", ".join('("%s", "%s")' % s for s in m.sound))
    if m.arc_blocks:
        L.append("    // ScriptPostLoad's Block/UnblockPlanningGraphArcs, in "
                 "order (true = block).")
        L.append("    arc_blocks: [%s],"
                 % ", ".join('("%s", %s)' % (n, "true" if b else "false")
                             for n, b in m.arc_blocks))
    if m.barriers:
        L.append("    // ScriptPostLoad's Enable/DisableBarriers, in order "
                 "(true = enabled).")
        L.append("    barriers: [%s],"
                 % ", ".join('("%s", %s)' % (n, "true" if b else "false")
                             for n, b in m.barriers))
    if m.death_drains:
        L.append("    // ScriptPostLoad's OnCharacterDeathTeam -> AddReinforcements: "
                 "a death on")
        L.append("    // `on` also costs `team` these tickets, on top of the "
                 "objective's one.")
        L.append("    death_drains: [%s],"
                 % ", ".join("(on: %d, team: %d, tickets: %d)"
                             % (on - 1, t - 1, n)
                             for on, t, n in m.death_drains))
    if m.vo_bleed or m.vo_low or m.vo_oob:
        L.append("    // The commander's lines, as the script sets them (Lua "
                 "team numbers, call")
        L.append("    // order): bleeding (listener, team, line, 4th arg), "
                 "low reinforcements")
        L.append("    // (listener, team, line, x, 5th arg), out of bounds "
                 "(team, line).")
        L.append("    voice: (")
        L.append("        bleeding: [")
        for l, t, n, f in m.vo_bleed:
            L.append('            (%d, %d, "%s", %s),'
                     % (l, t, n, "true" if f else "false"))
        L.append("        ],")
        L.append("        low_reinforcements: [")
        for l, t, n, x, f in m.vo_low:
            L.append('            (%d, %d, "%s", %s, %s),'
                     % (l, t, n, fmtf(float(f32_text(x))),
                        "true" if f else "false"))
        L.append("        ],")
        L.append("        out_of_bounds: [%s],"
                 % ", ".join('(%d, "%s")' % (t, n) for t, n in m.vo_oob))
        L.append("    ),")
    if m.mus_amb or m.mus_vic or m.mus_def:
        L.append("    // The music, as the script sets it (Lua team numbers, "
                 "call order):")
        L.append("    // ambient (team, x, Music, state 0 start / 1 middle / "
                 "2 end, 5th arg),")
        L.append("    // victory and defeat (team, Music).")
        L.append("    music: (")
        L.append("        ambient: [")
        for t, x, n, st, f in m.mus_amb:
            L.append('            (%d, %s, "%s", %d, %s),'
                     % (t, fmtf(float(f32_text(x))), n, st,
                        "true" if f else "false"))
        L.append("        ],")
        for key, rows in (("victory", m.mus_vic), ("defeat", m.mus_def)):
            L.append("        %s: [%s],"
                     % (key, ", ".join('(%d, "%s")' % (t, n)
                                       for t, n in rows)))
        L.append("    ),")
    if m.shots:
        L.append("    // The spawn screens' backdrops: AddCameraShot(qw, qx, qy, qz, "
                 "x, y, z) in call")
        L.append("    // order, the first 16 (the PS2's cap).")
        L.append("    camera_shots: [")
        for sh in m.shots:
            L.append("        (%s)," % ", ".join(f32_text(v) for v in sh))
        L.append("    ],")
    if m.ai_view is not None or m.ai_aggr or m.ai_snipe is not None or \
            m.ai_no_capture:
        L.append("    // The script's AI settings (Lua team numbers): "
                 "SetAIViewMultiplier,")
        L.append("    // SetTeamAggressiveness, AISnipeSuitabilityDist and "
                 "the AICanCaptureCP")
        L.append("    // bans in force (post, team).")
        L.append("    ai: (")
        if m.ai_view is not None:
            L.append("        view_mult: Some(%s),"
                     % fmtf(float(f32_text(m.ai_view))))
        if m.ai_aggr:
            L.append("        aggressiveness: [%s],"
                     % ", ".join("(%d, %s)" % (t, fmtf(float(f32_text(v))))
                                 for t, v in m.ai_aggr))
        if m.ai_snipe is not None:
            L.append("        snipe_suitability: Some(%s)," % fmtf(m.ai_snipe))
        if m.ai_no_capture:
            L.append("        no_capture: [%s],"
                     % ", ".join('("%s", %d)' % (n, t)
                                 for n, t in m.ai_no_capture))
        L.append("    ),")
    if m.locals:
        L.append("    // locals = (first AddUnitClass of team %d, its "
                 "SetUnitCount)." % m.locals_team)
    L.append("    locals: %s"
             % ('Some(("%s", %d)),' % m.locals if m.locals else "None,"))
    L.append('    source: "%sPS2 %s.luac",' % ("PC %s; " % m.pc if m.pc else "", m.ps2))
    L.append(")")
    return "\n".join(L) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="report only")
    ap.add_argument("--ps2-only", action="store_true",
                    help="no PC sources: build from the PS2 bytecode alone")
    ap.add_argument("--lua-root", metavar="DIR",
                    help="the extracted Lua (DIR/bf1/mission, DIR/bf2/mission;"
                         " default extracted/<game>/lua/mission)")
    ap.add_argument("--out", metavar="DIR", help="default engine/missions")
    ap.add_argument("--dump", nargs=2, metavar=("GAME", "SCRIPT"),
                    help="print a PS2 script's reconstructed call sequence")
    args = ap.parse_args()
    global PS2_ONLY, PS2_BF1, PS2_BF2, OUT_DIR
    PS2_ONLY = args.ps2_only
    if args.lua_root:
        PS2_BF1 = os.path.join(args.lua_root, "bf1/mission")
        PS2_BF2 = os.path.join(args.lua_root, "bf2/mission")
    if args.out:
        OUT_DIR = args.out
        os.makedirs(OUT_DIR, exist_ok=True)

    if args.dump:
        game, name = args.dump
        f = load_ps2(game, name)
        if f is None:
            sys.exit("no such PS2 script")
        for c in (calls40(f) if game == "bf1" else calls50(f)):
            print(c)
        return

    rows, written = [], 0
    for key, game, planet, eras in MAPS:
        for era in ("gcw", "cw"):
            if era not in eras:
                continue
            stem, ps2 = eras[era]
            m = build(key, game, planet, era, stem, ps2)
            suffix = "_con.ron" if era == "gcw" or "gcw" not in eras \
                else "_con_cw.ron"
            out = os.path.join(OUT_DIR, key + suffix)
            if not args.check:
                # UTF-8 on every OS: the header comments carry an em dash, and Windows' default (cp1252) wrote
                # bytes the game's UTF-8 read refused, so every mission failed to load there.
                with open(out, "w", encoding="utf-8", newline="\n") as fh:
                    fh.write(ron(m))
                written += 1
            rows.append((os.path.basename(out), m))

    for name, m in rows:
        print("%-24s %-3s %-3s %-8s %-8s %2d/%-2d %4d/%-4d cap%-3d att%-5s "
              "cps%-2d dr%-2d loc%-4s %s"
              % (name, m.game, m.era, m.teams[0][0], m.teams[1][0],
                 m.teams[0][1], m.teams[1][1], m.teams[0][2], m.teams[1][2],
                 m.cap, m.attacking, len(m.cps), len(m.death_regions),
                 m.locals[1] if m.locals else "-",
                 "PROVISIONAL:%d" % len(m.prov) if m.prov else ""))
        for p in m.prov:
            print("      ! %s" % p)
    print("\n%d missions, %d written, %d with PROVISIONAL notes"
          % (len(rows), written, sum(1 for _, m in rows if m.prov)))


if __name__ == "__main__":
    main()
