#!/usr/bin/env python3
"""Translate a BF2 campaign mission's PS2 bytecode into openbf's campaign
action tree (the `script` block of engine/missions/campaign/*.ron).

PLAN.md locks "data-driven missions, no Lua VM", so nothing Lua runs in the
game. The Rise of the Empire scripts are not declarative, though: each one
keeps counters, branches on what killed what (geo1c_c's sniper and lightsaber
counts), registers handlers from inside handlers, and loops over ship lists
(spa1g_c). Guessing a fixed stage kind from that (the old `_infer_bare_objective`)
loses the logic, so this module translates every function of the mission
chunk, offline, into a closed tree of engine calls, variable updates,
conditions and loops. `game::campaign_script` evaluates that tree natively,
and the shared framework the missions build on (`Objective`, `ObjectiveGoto`,
`ObjectiveAssault`, `ObjectiveConquest`, `ObjectiveCTF`, `ObjectiveTDM`,
`MultiObjectiveContainer`, `Ambush`, `SetupAmbushTrigger`) is ported natively
in `game::campaign_objectives` (audit A6: engine logic, not data).

Reading the bytecode (Lua 5.0.2 with float numbers, RK base from
`gen_missions._rk_base`):

* Registers become function locals. Values that are only read once (globals,
  constants, field reads, constructor tables) fold into the expression that
  reads them; a register that is still live across a side effect, a join or a
  loop is written to its local first (liveness below), so the order of reads
  and writes is the bytecode's.
* EQ/LT/LE/TEST + JMP become `If`; the arms run to the branch's immediate
  post-dominator and the code after it continues once. A jump into code that
  another arm also reaches is followed (that code is emitted in both arms), so
  `a and b` chains come out as nested ifs with no guessing.
* `for i = a, b, c` (SUB + JMP + FORLOOP) and `for k, v in f(x)`
  (TFORPREP/TFORLOOP) become `ForNum` / `ForIn`.
* A closure's captured locals are shared cells, so a handler that releases
  itself (`local t; t = OnEnterRegion(function() ReleaseEnterRegion(t) end)`,
  uta1c_c) sees the later assignment.

Anything else (while/repeat loops, break, varargs, SETUPVAL into a loop
variable) raises `Unsupported`, which the generator reports as a failure
rather than writing a partial mission.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_missions as gm  # noqa: E402


class Unsupported(Exception):
    pass


OPS = gm.OPS50
RK_OPS = {"GETTABLE", "SETTABLE", "SELF", "ADD", "SUB", "MUL", "DIV", "POW",
          "EQ", "LT", "LE"}
BINOPS = {"ADD": "Add", "SUB": "Sub", "MUL": "Mul", "DIV": "Div", "POW": "Pow"}


def dec(ins):
    op = OPS[ins & 0x3F]
    a = (ins >> 24) & 0xFF
    c = (ins >> 6) & 0x1FF
    b = (ins >> 15) & 0x1FF
    bx = (ins >> 6) & 0x3FFFF
    return op, a, b, c, bx, bx - 131071


# ------------------------------------------------------------------ CFG

def successors(code, pc):
    op, a, b, c, bx, sbx = dec(code[pc])
    n = len(code)
    if op == "JMP":
        return [pc + 1 + sbx]
    if op in ("EQ", "LT", "LE", "TEST"):
        return [pc + 1, pc + 2]
    if op == "LOADBOOL" and c:
        return [pc + 2]
    if op in ("RETURN", "TAILCALL"):
        return []
    if op == "FORLOOP":
        return [pc + 1 + sbx, pc + 1]
    if op == "TFORPREP":
        return [pc + 1 + sbx]
    if op == "TFORLOOP":
        return [pc + 1, pc + 2]
    return [pc + 1] if pc + 1 < n else []


def use_def(p, base, pc):
    """Registers an instruction reads and writes (for liveness)."""
    op, a, b, c, bx, sbx = dec(p.code[pc])
    top = max(p.maxstack, a + 1)

    def rk(x):
        return [] if x >= base else [x]

    if op == "MOVE":
        return [b], [a]
    if op in ("LOADK", "GETUPVAL", "GETGLOBAL", "NEWTABLE"):
        return [], [a]
    if op == "LOADBOOL":
        return [], [a]
    if op == "LOADNIL":
        return [], list(range(a, b + 1))
    if op == "GETTABLE":
        return [b] + rk(c), [a]
    if op in ("SETGLOBAL", "SETUPVAL"):
        return [a], []
    if op == "SETTABLE":
        return [a] + rk(b) + rk(c), []
    if op == "SELF":
        return [b] + rk(c), [a, a + 1]
    if op in BINOPS:
        return rk(b) + rk(c), [a]
    if op in ("UNM", "NOT"):
        return [b], [a]
    if op == "CONCAT":
        return list(range(b, c + 1)), [a]
    if op in ("EQ", "LT", "LE"):
        return rk(b) + rk(c), []
    if op == "TEST":
        return [b], []
    if op in ("CALL", "TAILCALL"):
        uses = list(range(a, (a + b) if b else top))
        defs = list(range(a, (a + c - 1) if c else top))
        return uses, defs
    if op == "RETURN":
        return list(range(a, (a + b - 1) if b else top)), []
    if op == "FORLOOP":
        return [a, a + 1, a + 2], [a]
    if op == "TFORPREP":
        return [a], [a, a + 1]
    if op == "TFORLOOP":
        return [a, a + 1, a + 2], list(range(a + 2, a + 3 + c))
    if op in ("SETLIST", "SETLISTO"):
        n = bx % 32 + 1
        return list(range(a, a + n + 1)), []
    if op == "CLOSURE":
        return [], [a]
    return [], []


def liveness(p, base):
    """live_out[pc]: registers read later on some path from pc."""
    code = p.code
    n = len(code)
    ud = [use_def(p, base, i) for i in range(n)]
    # The capture pseudo-instructions after CLOSURE read their registers.
    skip = set()
    for i in range(n):
        op, a, b, c, bx, sbx = dec(code[i])
        if op == "CLOSURE":
            nups = p.protos[bx].nups
            caps = []
            for j in range(1, nups + 1):
                qop, qa, qb, _, _, _ = dec(code[i + j])
                skip.add(i + j)
                if qop == "MOVE":
                    caps.append(qb)
            ud[i] = (caps, [a])
    succ = [[] if i in skip else [s for s in successors(code, i) if s < n] for i in range(n)]
    for i in range(n):
        op, a, b, c, bx, sbx = dec(code[i])
        if op == "CLOSURE":
            succ[i] = [i + 1 + p.protos[bx].nups] if i + 1 + p.protos[bx].nups < n else []
    live_in = [set() for _ in range(n)]
    live_out = [set() for _ in range(n)]
    changed = True
    while changed:
        changed = False
        for i in range(n - 1, -1, -1):
            out = set()
            for s in succ[i]:
                out |= live_in[s]
            uses, defs = ud[i]
            inn = set(uses) | (out - set(defs))
            if out != live_out[i] or inn != live_in[i]:
                live_out[i], live_in[i] = out, inn
                changed = True
    return live_in, live_out, succ


def post_dominators(succ, n):
    """Immediate post-dominator per pc (n = the virtual exit)."""
    EXIT = n
    full = set(range(n + 1))
    pdom = [set(full) for _ in range(n)] + [{EXIT}]
    changed = True
    while changed:
        changed = False
        for i in range(n - 1, -1, -1):
            ss = succ[i] or [EXIT]
            new = set.intersection(*(pdom[s] for s in ss)) | {i}
            if new != pdom[i]:
                pdom[i] = new
                changed = True
    # Post-dominator sets form a chain, so the immediate one is the candidate
    # whose own set is exactly one smaller.
    ipdom = []
    for i in range(n):
        want = len(pdom[i]) - 1
        best = next((c for c in pdom[i] if c != i and len(pdom[c]) == want), EXIT)
        ipdom.append(best)
    return ipdom


# ------------------------------------------------------------------ expressions
# Tuples mirror the Rust enums in game::campaign_script:
#   ("Nil",) ("Bool", b) ("Num", f) ("Str", s) ("Local", r) ("Upval", i)
#   ("Global", name) ("Index", t, k) ("Call", f, [args]) ("Method", o, name, [args])
#   ("Bin", op, a, b) ("Not", e) ("Neg", e) ("Closure", fn, [caps]) ("Table", [(k, v)])

def is_const(e):
    return e[0] in ("Nil", "Bool", "Num", "Str", "Closure")


def refs(e, kinds):
    """True when the expression reads anything in `kinds` (volatile state)."""
    t = e[0]
    if t in kinds:
        return True
    if t == "Index":
        return "Index" in kinds or refs(e[1], kinds) or refs(e[2], kinds)
    if t in ("Call", "Method"):
        return True
    if t == "Bin":
        return refs(e[2], kinds) or refs(e[3], kinds)
    if t in ("Not", "Neg"):
        return refs(e[1], kinds)
    if t == "Table":
        return True
    return False


def reads_local(e, r):
    t = e[0]
    if t == "Local":
        return e[1] == r
    if t == "Index":
        return reads_local(e[1], r) or reads_local(e[2], r)
    if t == "Bin":
        return reads_local(e[2], r) or reads_local(e[3], r)
    if t in ("Not", "Neg"):
        return reads_local(e[1], r)
    if t == "Table":
        return any(reads_local(k, r) or reads_local(v, r) for k, v in e[1])
    if t == "Call":
        return reads_local(e[1], r) or any(reads_local(x, r) for x in e[2])
    if t == "Method":
        return reads_local(e[1], r) or any(reads_local(x, r) for x in e[3])
    return False


class FnTranslator:
    def __init__(self, unit, p, fn_id, name):
        self.unit = unit
        self.p = p
        self.fn_id = fn_id
        self.name = name
        self.base = unit.base
        self.code = p.code
        self.n = len(p.code)
        self.live_in, self.live_out, self.succ = liveness(p, self.base)
        self.ipdom = post_dominators(self.succ, self.n)
        self.captured = set()
        # Lua 5.0 `...`: the extra arguments arrive as the `arg` table in
        # register numparams (lfunc.c adjust_varargs).
        self.vararg = bool(p.is_vararg)
        self.nlocals = max(p.maxstack, p.numparams + int(self.vararg))

    # ---- helpers
    def k(self, x):
        v = self.p.k[x]
        return const_expr(v)

    def rk(self, st, x):
        if x >= self.base:
            return self.k(x - self.base)
        return self.read(st, x)

    def read(self, st, r):
        e = st.get(r)
        if e is None:
            return ("Local", r)
        return e

    def materialize(self, st, out, regs):
        for r in sorted(regs):
            e = st.get(r)
            if e is None or e == ("Local", r):
                continue
            out.append(("SetLocal", r, e))
            st[r] = ("Local", r)

    def before_effect(self, st, out, pc, keep=()):
        """Write live registers whose value a side effect could change."""
        live = self.live_out[pc]
        regs = [r for r in live if r not in keep and r in st
                and st[r] != ("Local", r)
                and refs(st[r], ("Global", "Index", "Upval", "Local"))]
        self.materialize(st, out, regs)

    def set_reg(self, st, out, pc, r, e):
        # Anything still reading the old value of local r is written first.
        dep = [q for q in self.live_out[pc] if q != r and q in st and reads_local(st[q], r)]
        self.materialize(st, out, dep)
        if r in self.captured:
            out.append(("SetLocal", r, e))
            st[r] = ("Local", r)
        else:
            st[r] = e

    def take_table(self, st, out, pc, r):
        """Read register r as an operand; a constructor table read by more
        than this instruction is written once so it keeps one identity."""
        e = self.read(st, r)
        if e[0] == "Table" and r in self.live_out[pc]:
            self.materialize(st, out, [r])
            return ("Local", r)
        return e

    # ---- structured emission
    def translate(self):
        st = {}
        for i in range(self.p.numparams + int(self.vararg)):
            st[i] = ("Local", i)
        body, _ = self.emit(0, self.n, st)
        return {
            "name": self.name,
            "params": self.p.numparams,
            "vararg": self.vararg,
            "locals": self.nlocals,
            "body": body,
        }

    def join_state(self, out_a, st_a, out_b, st_b, join):
        """Merge two arms' register states at `join`."""
        live = self.live_in[join] if join < self.n else set()
        merged = {}
        for r in live:
            ea, eb = st_a.get(r), st_b.get(r)
            if ea == eb and ea is not None:
                merged[r] = ea
                continue
            if ea is not None and ea != ("Local", r):
                out_a.append(("SetLocal", r, ea))
            if eb is not None and eb != ("Local", r):
                out_b.append(("SetLocal", r, eb))
            merged[r] = ("Local", r)
        return merged

    def emit(self, pc, stop, st, depth=0):
        """Statements for pc..stop; returns (stmts, state or None if every
        path returned)."""
        if depth > 200:
            raise Unsupported("%s: nesting too deep" % self.name)
        out = []
        code = self.code
        while pc != stop:
            if pc >= self.n:
                return out, None
            op, a, b, c, bx, sbx = dec(code[pc])

            if op == "MOVE":
                self.set_reg(st, out, pc, a, self.take_table(st, out, pc, b))
            elif op == "LOADK":
                self.set_reg(st, out, pc, a, self.k(bx))
            elif op == "LOADBOOL":
                self.set_reg(st, out, pc, a, ("Bool", bool(b)))
                if c:
                    pc += 2
                    continue
            elif op == "LOADNIL":
                for r in range(a, b + 1):
                    self.set_reg(st, out, pc, r, ("Nil",))
            elif op == "GETUPVAL":
                self.set_reg(st, out, pc, a, ("Upval", b))
            elif op == "GETGLOBAL":
                self.set_reg(st, out, pc, a, ("Global", self.p.k[bx]))
            elif op == "GETTABLE":
                self.set_reg(st, out, pc, a, ("Index", self.take_table(st, out, pc, b), self.rk(st, c)))
            elif op == "SETGLOBAL":
                v = self.take_table(st, out, pc, a)
                self.before_effect(st, out, pc)
                out.append(("SetGlobal", self.p.k[bx], v))
            elif op == "SETUPVAL":
                v = self.take_table(st, out, pc, a)
                self.before_effect(st, out, pc)
                out.append(("SetUpval", b, v))
            elif op == "SETTABLE":
                t = self.read(st, a)
                key = self.rk(st, b)
                val = self.rk(st, c)
                if c < self.base:
                    val = self.take_table(st, out, pc, c)
                if t[0] == "Table" and is_const(key):
                    st[a] = ("Table", t[1] + [(key, val)])
                else:
                    if t[0] == "Table":
                        self.materialize(st, out, [a])
                        t = ("Local", a)
                    self.before_effect(st, out, pc, keep=(a,))
                    out.append(("SetIndex", t, key, val))
            elif op == "NEWTABLE":
                self.set_reg(st, out, pc, a, ("Table", []))
            elif op in ("SETLIST", "SETLISTO"):
                t = self.read(st, a)
                nvals = bx % 32 + 1
                first = bx - bx % 32
                items = [(("Num", float(first + i)), self.take_table(st, out, pc, a + i))
                         for i in range(1, nvals + 1)]
                if t[0] == "Table":
                    st[a] = ("Table", t[1] + items)
                else:
                    for kk, vv in items:
                        out.append(("SetIndex", t, kk, vv))
            elif op == "SELF":
                obj = self.take_table(st, out, pc, b)
                if obj[0] not in ("Local", "Global", "Upval") and not is_const(obj):
                    self.materialize(st, out, [b])
                    obj = ("Local", b)
                st[a + 1] = obj
                st[a] = ("SelfFn", obj, self.rk(st, c))
            elif op in BINOPS:
                self.set_reg(st, out, pc, a, ("Bin", BINOPS[op], self.rk(st, b), self.rk(st, c)))
            elif op == "UNM":
                self.set_reg(st, out, pc, a, ("Neg", self.read(st, b)))
            elif op == "NOT":
                self.set_reg(st, out, pc, a, ("Not", self.read(st, b)))
            elif op == "CONCAT":
                e = self.read(st, b)
                for r in range(b + 1, c + 1):
                    e = ("Bin", "Concat", e, self.read(st, r))
                self.set_reg(st, out, pc, a, e)
            elif op in ("CALL", "TAILCALL"):
                stmt_pc = pc
                fn = self.read(st, a)
                if b == 0:
                    last = max((r for r in st if r > a), default=a)
                    args = [self.take_table(st, out, pc, r) for r in range(a + 1, last + 1)]
                else:
                    args = [self.take_table(st, out, pc, r) for r in range(a + 1, a + b)]
                if fn[0] == "SelfFn":
                    obj, meth = fn[1], fn[2]
                    if meth[0] != "Str":
                        raise Unsupported("%s pc %d: dynamic method name" % (self.name, pc))
                    # SELF put the receiver in R(A+1): the call's first
                    # argument is always it, even once spilled to a local.
                    recv = args[0] if args else obj
                    call = ("Method", recv, meth[1], args[1:])
                else:
                    call = ("Call", fn, args)
                nres = c - 1
                if (fn[0] == "Global" and fn[1] in ("ipairs", "pairs") and nres >= 3
                        and self.next_is_tforprep(pc + 1, a)):
                    for r in list(st):
                        if r > a:
                            del st[r]
                    st[a] = ("Iter", fn[1], args[0] if args else ("Nil",))
                    pc += 1
                    continue
                self.before_effect(st, out, pc, keep=range(a, a + max(nres, 1)))
                for r in list(st):
                    if r >= a:
                        del st[r]
                if op == "TAILCALL":
                    out.append(("Return", [call]))
                    return out, None
                if nres == 0:
                    out.append(("Do", call))
                elif nres == 1:
                    out.append(("SetLocal", a, call))
                    st[a] = ("Local", a)
                elif nres > 1:
                    out.append(("SetLocals", list(range(a, a + nres)), call))
                    for r in range(a, a + nres):
                        st[r] = ("Local", r)
                else:
                    # `f(g(x))`: an open result feeds the next call's last
                    # argument; it stays an expression so it runs inside that
                    # call, in order (a side effect in between writes it first).
                    st[a] = call
            elif op == "RETURN":
                if b == 0:
                    raise Unsupported("%s pc %d: open return" % (self.name, pc))
                vals = [self.take_table(st, out, pc, r) for r in range(a, a + b - 1)]
                out.append(("Return", vals))
                return out, None
            elif op == "CLOSURE":
                child = self.p.protos[bx]
                caps = []
                for j in range(1, child.nups + 1):
                    qop, qa, qb, _, _, _ = dec(code[pc + j])
                    if qop == "MOVE":
                        self.materialize(st, out, [qb])
                        self.captured.add(qb)
                        caps.append(("Local", qb))
                    elif qop == "GETUPVAL":
                        caps.append(("Upval", qb))
                    else:
                        raise Unsupported("%s pc %d: closure capture %s" % (self.name, pc, qop))
                fid = self.unit.add_proto(child, "%s.%d" % (self.name, bx))
                self.set_reg(st, out, pc, a, ("Closure", fid, caps))
                pc += 1 + child.nups
                continue
            elif op == "CLOSE":
                # A block that captured its locals ends (lparser.c leaveblock):
                # closures made so far keep their cells, the next pass of a
                # loop gets fresh ones.
                self.before_effect(st, out, pc)
                out.append(("Close", a))
            elif op in ("EQ", "LT", "LE", "TEST"):
                nop, _, _, _, _, nsbx = dec(code[pc + 1])
                if nop != "JMP":
                    raise Unsupported("%s pc %d: %s without JMP" % (self.name, pc, op))
                taken = pc + 2 + nsbx
                fall = pc + 2
                if op == "TEST":
                    v = self.read(st, b)
                    cond_fall = v if c == 0 else ("Not", v)
                else:
                    x, y = self.rk(st, b), self.rk(st, c)
                    cmp = ("Bin", {"EQ": "Eq", "LT": "Lt", "LE": "Le"}[op], x, y)
                    # The jump is taken when the comparison equals A.
                    cond_fall = cmp if a == 0 else ("Not", cmp)
                join = self.ipdom[pc]
                if join == pc + 1:
                    join = self.ipdom[pc + 1]
                st_f = dict(st)
                st_t = dict(st)
                out_t_pre = []
                if op == "TEST" and a != b:
                    st_t[a] = self.read(st, b)
                if join > stop and stop != self.n:
                    join = stop
                arm_f, end_f = self.emit(fall, join, st_f, depth + 1)
                arm_t, end_t = self.emit(taken, join, st_t, depth + 1)
                arm_t = out_t_pre + arm_t
                out.append(("If", cond_fall, arm_f, arm_t))
                if end_f is None and end_t is None:
                    return out, None
                if join >= self.n:
                    return out, None
                if end_f is None:
                    st = end_t
                elif end_t is None:
                    st = end_f
                else:
                    st = self.join_state(arm_f, end_f, arm_t, end_t, join)
                pc = join
                continue
            elif op == "JMP":
                target = pc + 1 + sbx
                tgt = dec(code[target]) if target < self.n else None
                if tgt and tgt[0] == "FORLOOP" and target > pc and target + 1 + tgt[5] == pc + 1:
                    pc = self.emit_fornum(pc, target, st, out, depth)
                    continue
                if target <= pc:
                    raise Unsupported("%s pc %d: backward jump (while/repeat)" % (self.name, pc))
                if stop < self.n and target > stop:
                    raise Unsupported("%s pc %d: jump out of its block (break)" % (self.name, pc))
                pc = target
                continue
            elif op == "TFORPREP":
                pc = self.emit_forin(pc, sbx, a, st, out, depth)
                continue
            else:
                raise Unsupported("%s pc %d: opcode %s" % (self.name, pc, op))
            pc += 1
        return out, st

    def next_is_tforprep(self, pc, a):
        if pc >= self.n:
            return False
        op, na, _, _, _, _ = dec(self.code[pc])
        return op == "TFORPREP" and na == a

    def loop_entry(self, st, out, pc_body):
        regs = [r for r in self.live_in[pc_body] if r in st and st[r] != ("Local", r)]
        self.materialize(st, out, regs)

    def emit_fornum(self, jmp_pc, loop_pc, st, out, depth):
        """`SUB A A A+2; JMP -> FORLOOP A` (lparser.c fornum, 5.0)."""
        prev = dec(self.code[jmp_pc - 1])
        fop, fa, _, _, _, fsbx = dec(self.code[loop_pc])
        if prev[0] != "SUB" or prev[1] != fa or loop_pc + 1 + fsbx != jmp_pc + 1:
            raise Unsupported("%s pc %d: unrecognised numeric for" % (self.name, jmp_pc))
        # The SUB already ran: R(A) holds start - step.
        step = self.read(st, fa + 2)
        start = ("Bin", "Add", self.read(st, fa), step)
        limit = self.read(st, fa + 1)
        self.before_effect(st, out, jmp_pc)
        body_st = {r: ("Local", r) for r in self.live_in[jmp_pc + 1]}
        for r in list(st):
            if r >= fa:
                del st[r]
        self.loop_entry(st, out, jmp_pc + 1)
        body_st.update({r: v for r, v in st.items() if v == ("Local", r)})
        body, end = self.emit(jmp_pc + 1, loop_pc, body_st, depth + 1)
        if end is not None:
            self.loop_entry(end, body, loop_pc)
        out.append(("ForNum", fa, start, limit, step, body))
        return loop_pc + 1

    def emit_forin(self, prep_pc, sbx, a, st, out, depth):
        loop_pc = prep_pc + 1 + sbx
        lop, la, _, lc, _, _ = dec(self.code[loop_pc])
        jop, _, _, _, _, jsbx = dec(self.code[loop_pc + 1])
        if lop != "TFORLOOP" or la != a or jop != "JMP" or loop_pc + 2 + jsbx != prep_pc + 1:
            raise Unsupported("%s pc %d: unrecognised generic for" % (self.name, prep_pc))
        it = self.read(st, a)
        if it[0] == "Iter":
            kind, coll = it[1], it[2]
        else:
            kind, coll = "pairs", it
        vars_ = list(range(a + 2, a + 3 + lc))
        self.before_effect(st, out, prep_pc)
        for r in list(st):
            if r >= a:
                del st[r]
        self.loop_entry(st, out, prep_pc + 1)
        body_st = {r: v for r, v in st.items()}
        for r in vars_:
            body_st[r] = ("Local", r)
        body, end = self.emit(prep_pc + 1, loop_pc, body_st, depth + 1)
        if end is not None:
            self.loop_entry(end, body, loop_pc)
        out.append(("ForIn", "Ipairs" if kind == "ipairs" else "Pairs", coll, vars_, body))
        return loop_pc + 2


def const_expr(v):
    if v is None:
        return ("Nil",)
    if isinstance(v, bool):
        return ("Bool", v)
    if isinstance(v, (int, float)):
        return ("Num", float(v))
    return ("Str", v)


class Unit:
    """Every function of one mission chunk, numbered as they are reached."""

    def __init__(self, root):
        self.root = root
        self.base = gm._rk_base(root)
        self.fns = []
        self.ids = {}
        self.queue = []
        self.parent = []
        self.current = None

    def add_proto(self, p, name):
        if id(p) in self.ids:
            return self.ids[id(p)]
        fid = len(self.fns)
        self.ids[id(p)] = fid
        self.fns.append(None)
        self.queue.append((p, fid, name))
        self.parent.append(self.current)
        return fid

    def run(self):
        self.add_proto(self.root, "main")
        while self.queue:
            p, fid, name = self.queue.pop(0)
            self.current = fid
            self.fns[fid] = FnTranslator(self, p, fid, name).translate()
        return self.fns


def name_functions(fns, parent):
    """Name each function after what it is assigned to (`ScriptPostLoad`,
    `Objective1.OnStart`); handlers keep their parent's name plus a number."""
    named = {0: "main"}

    def visit(n):
        if n[0] == "SetGlobal" and n[2][0] == "Closure":
            named.setdefault(n[2][1], n[1])
        if (n[0] == "SetIndex" and n[3][0] == "Closure" and n[1][0] == "Global"
                and n[2][0] == "Str"):
            named.setdefault(n[3][1], "%s.%s" % (n[1][1], n[2][1]))

    for f in fns:
        walk_exprs(f["body"], visit)
    counts = {}
    for fid, f in enumerate(fns):
        if fid in named:
            f["name"] = named[fid]
        else:
            par = fns[parent[fid]]["name"] if parent[fid] is not None else "main"
            counts[par] = counts.get(par, 0) + 1
            f["name"] = "%s.%d" % (par, counts[par])
    return fns


def translate(root):
    unit = Unit(root)
    fns = name_functions(unit.run(), unit.parent)
    return {"fns": fns, "main": 0}


# ------------------------------------------------------------------ census

def walk_exprs(node, fn):
    if isinstance(node, tuple):
        fn(node)
        for x in node[1:]:
            walk_exprs(x, fn)
    elif isinstance(node, list):
        for x in node:
            walk_exprs(x, fn)


def called_globals(script):
    """Global names the script calls but never assigns (engine functions)."""
    assigned = set()
    called = {}

    def visit(n):
        if n[0] == "SetGlobal":
            assigned.add(n[1])
        if n[0] == "Call" and n[1][0] == "Global":
            called[n[1][1]] = called.get(n[1][1], 0) + 1
        if n[0] == "Call" and n[1][0] == "Index" and n[1][1][0] == "Global" and n[1][2][0] == "Str":
            nm = "%s.%s" % (n[1][1][1], n[1][2][1])
            called[nm] = called.get(nm, 0) + 1
        if n[0] == "Method" and n[1][0] == "Global":
            nm = "%s:%s" % (n[1][1], n[2])
            called[nm] = called.get(nm, 0) + 1

    for f in script["fns"]:
        walk_exprs(f["body"], visit)
    return {k: v for k, v in called.items() if k.split(":")[0] not in assigned}


def do_files(script):
    """Names passed to ScriptCB_DoFile as constants."""
    names = []

    def visit(n):
        if (n[0] == "Call" and n[1] == ("Global", "ScriptCB_DoFile")
                and n[2] and n[2][0][0] == "Str"):
            names.append(n[2][0][1])

    for f in script["fns"]:
        walk_exprs(f["body"], visit)
    return names


# ------------------------------------------------------------------ RON

def ron_num(x):
    s = gm.f32_text(x)
    if s in ("inf", "-inf", "nan"):
        raise Unsupported("non-finite constant")
    if "e" in s:
        mant, exp = s.split("e")
        if "." not in mant:
            mant += ".0"
        return "%se%d" % (mant, int(exp))
    if "." not in s:
        s += ".0"
    return s


def ron_expr(e):
    t = e[0]
    if t == "Nil":
        return "Nil"
    if t == "Bool":
        return "Bool(%s)" % ("true" if e[1] else "false")
    if t == "Num":
        return "Num(%s)" % ron_num(e[1])
    if t == "Str":
        return "Str(%s)" % gm._ron_str(e[1])
    if t == "Local":
        return "Local(%d)" % e[1]
    if t == "Upval":
        return "Upval(%d)" % e[1]
    if t == "Global":
        return "Global(%s)" % gm._ron_str(e[1])
    if t == "Index":
        return "Index(%s, %s)" % (ron_expr(e[1]), ron_expr(e[2]))
    if t == "Call":
        return "Call(%s, [%s])" % (ron_expr(e[1]), ", ".join(ron_expr(x) for x in e[2]))
    if t == "Method":
        return "Method(%s, %s, [%s])" % (ron_expr(e[1]), gm._ron_str(e[2]),
                                         ", ".join(ron_expr(x) for x in e[3]))
    if t == "Bin":
        return "Bin(%s, %s, %s)" % (e[1], ron_expr(e[2]), ron_expr(e[3]))
    if t == "Not":
        return "Not(%s)" % ron_expr(e[1])
    if t == "Neg":
        return "Neg(%s)" % ron_expr(e[1])
    if t == "Closure":
        return "Closure(%d, [%s])" % (e[1], ", ".join(ron_expr(x) for x in e[2]))
    if t == "Table":
        return "Table([%s])" % ", ".join("(%s, %s)" % (ron_expr(k), ron_expr(v)) for k, v in e[1])
    if t in ("SelfFn", "Iter"):
        raise Unsupported("dangling %s value" % t)
    raise Unsupported("expr %r" % (t,))


def ron_stmts(stmts, ind):
    pad = " " * ind
    lines = []
    for s in stmts:
        t = s[0]
        if t == "Do":
            lines.append("%sDo(%s)," % (pad, ron_expr(s[1])))
        elif t == "SetLocal":
            lines.append("%sSetLocal(%d, %s)," % (pad, s[1], ron_expr(s[2])))
        elif t == "SetLocals":
            lines.append("%sSetLocals([%s], %s)," % (pad, ", ".join(str(r) for r in s[1]), ron_expr(s[2])))
        elif t == "SetGlobal":
            lines.append("%sSetGlobal(%s, %s)," % (pad, gm._ron_str(s[1]), ron_expr(s[2])))
        elif t == "SetUpval":
            lines.append("%sSetUpval(%d, %s)," % (pad, s[1], ron_expr(s[2])))
        elif t == "SetIndex":
            lines.append("%sSetIndex(%s, %s, %s)," % (pad, ron_expr(s[1]), ron_expr(s[2]), ron_expr(s[3])))
        elif t == "Return":
            lines.append("%sReturn([%s])," % (pad, ", ".join(ron_expr(x) for x in s[1])))
        elif t == "Close":
            lines.append("%sClose(%d)," % (pad, s[1]))
        elif t == "If":
            lines.append("%sIf(%s, [" % (pad, ron_expr(s[1])))
            lines += ron_stmts(s[2], ind + 4)
            lines.append("%s], [" % pad)
            lines += ron_stmts(s[3], ind + 4)
            lines.append("%s])," % pad)
        elif t == "ForNum":
            lines.append("%sForNum(%d, %s, %s, %s, [" % (pad, s[1], ron_expr(s[2]), ron_expr(s[3]), ron_expr(s[4])))
            lines += ron_stmts(s[5], ind + 4)
            lines.append("%s])," % pad)
        elif t == "ForIn":
            lines.append("%sForIn(%s, %s, [%s], [" % (pad, s[1], ron_expr(s[2]), ", ".join(str(r) for r in s[3])))
            lines += ron_stmts(s[4], ind + 4)
            lines.append("%s])," % pad)
        else:
            raise Unsupported("stmt %r" % (t,))
    return lines


def ron_script(script, ind=8):
    pad = " " * ind
    lines = ["%sscript: Some((" % pad, "%s    main: %d," % (pad, script["main"]), "%s    fns: [" % pad]
    for i, f in enumerate(script["fns"]):
        lines.append("%s        // fn %d" % (pad, i))
        lines.append("%s        (name: %s, params: %d, vararg: %s, locals: %d, body: [" % (
            pad, gm._ron_str(f["name"]), f["params"], "true" if f.get("vararg") else "false",
            f["locals"]))
        lines += ron_stmts(f["body"], ind + 12)
        lines.append("%s        ])," % pad)
    lines.append("%s    ]," % pad)
    lines.append("%s))," % pad)
    return "\n".join(lines)


def ron_lib(script):
    """A shared script on its own (engine/missions/campaign/lib/*.ron)."""
    body = ron_script(script, ind=0).split("\n")
    body[0] = "("
    body[-1] = ")"
    return "\n".join(body) + "\n"


def main():
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("script", nargs="+")
    ap.add_argument("--census", action="store_true")
    args = ap.parse_args()
    total = {}
    for name in args.script:
        f = gm.load_ps2("bf2", name)
        if f is None:
            print("missing", name)
            continue
        script = translate(f)
        if args.census:
            for k, v in called_globals(script).items():
                total.setdefault(k, set()).add(name)
        else:
            print(ron_script(script))
    if args.census:
        for k in sorted(total):
            print(k, len(total[k]), " ".join(sorted(total[k])))


if __name__ == "__main__":
    main()
