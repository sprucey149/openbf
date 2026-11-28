use super::*;

fn push_str(buf: &mut Vec<u8>, s: Option<&str>) {
    match s {
        None => buf.extend(0u32.to_le_bytes()),
        Some(text) => {
            buf.extend(((text.len() + 1) as u32).to_le_bytes());
            buf.extend_from_slice(text.as_bytes());
            buf.push(0);
        }
    }
}

fn push_proto(
    buf: &mut Vec<u8>,
    nups: u8,
    consts: &[LuaConst],
    protos: &[(&[u8], &[LuaConst], &[u32])],
    code: &[u32],
) {
    push_str(buf, Some("@test"));
    buf.extend(0u32.to_le_bytes()); // lineDefined
    buf.push(nups);
    buf.push(0); // numparams
    buf.push(0); // is_vararg
    buf.push(16); // maxstack
    buf.extend(0u32.to_le_bytes()); // lineinfo
    buf.extend(0u32.to_le_bytes()); // locals
    buf.extend(0u32.to_le_bytes()); // upvalues
    buf.extend((consts.len() as u32).to_le_bytes());
    for c in consts {
        match c {
            LuaConst::Nil => buf.push(0),
            LuaConst::Bool(b) => {
                buf.push(1);
                buf.push(u8::from(*b));
            }
            LuaConst::Num(n) => {
                buf.push(3);
                buf.extend((*n as f32).to_le_bytes());
            }
            LuaConst::Str(s) => {
                buf.push(4);
                push_str(buf, Some(s));
            }
        }
    }
    buf.extend((protos.len() as u32).to_le_bytes());
    for &(cnups, pconsts, pcode) in protos {
        push_proto(buf, cnups[0], pconsts, &[], pcode);
    }
    buf.extend((code.len() as u32).to_le_bytes());
    for &ins in code {
        buf.extend(ins.to_le_bytes());
    }
}

fn ins_abc(op: Op50, a: u32, b: u32, c: u32) -> u32 {
    (op as u32) | ((c & 0x1FF) << 6) | ((b & 0x1FF) << 15) | ((a & 0xFF) << 24)
}

fn ins_abx(op: Op50, a: u32, bx: u32) -> u32 {
    (op as u32) | ((bx & 0x3FFFF) << 6) | ((a & 0xFF) << 24)
}

fn make_chunk(consts: &[LuaConst], code: &[u32]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(LUA_SIG);
    buf.push(LUA_VERSION_50);
    buf.push(1); // little endian
    buf.push(4); // int
    buf.push(4); // size_t
    buf.push(4); // instruction
    buf.push(6); // OP
    buf.push(8); // A
    buf.push(9); // B
    buf.push(9); // C
    buf.push(4); // number = f32
    #[allow(clippy::approx_constant)]
    buf.extend(3.1415926f32.to_le_bytes());
    push_proto(&mut buf, 0, consts, &[], code);
    buf
}

#[test]
fn rejects_corrupt_and_oversized_bytecode() {
    assert!(matches!(undump(b"notlua"), Err(LuaError::Truncated(_))));
    assert!(matches!(
        undump(b"nope\x50\x01\x04\x04\x04\x06\x08\x09\x09\x04\x00\x00\x00\x00"),
        Err(LuaError::BadHeader(_))
    ));
    let huge = vec![0u8; MAX_BYTECODE_BYTES + 1];
    assert!(matches!(undump(&huge), Err(LuaError::TooLarge(_))));
    let bad_ver = b"\x1bLua\x51\x01\x04\x04\x04\x06\x08\x09\x09\x04\x00\x00\x00\x00";
    assert!(matches!(undump(bad_ver), Err(LuaError::BadHeader(_))));
}

#[test]
fn evaluates_addme_table_insertion_and_dynamic_warnings() {
    let rk = 128u32;
    let consts = [
        LuaConst::Str("sp_missionselect_listbox_contents".into()), // 0
        LuaConst::Str("mapluafile".into()),                        // 1
        LuaConst::Str("MOD%s_%s".into()),                          // 2
        LuaConst::Str("era_g".into()),                             // 3
        LuaConst::Num(1.0),                                        // 4
        LuaConst::Str("AddDownloadableContent".into()),            // 5
        LuaConst::Str("MOD".into()),                               // 6
        LuaConst::Str("OnObjectKillName".into()),                  // 7
        LuaConst::Str("door01".into()),                            // 8
    ];
    let code = [
        // sp_missionselect_listbox_contents[1] = { mapluafile = "MOD%s_%s", era_g = 1 }
        ins_abx(Op50::GetGlobal, 0, 0),
        ins_abx(Op50::NewTable, 1, 0),
        ins_abc(Op50::SetTable, 1, rk + 1, rk + 2),
        ins_abc(Op50::SetTable, 1, rk + 3, rk + 4),
        ins_abc(Op50::SetTable, 0, rk + 4, 1),
        // AddDownloadableContent("MOD", "MODg_con", 1)
        ins_abx(Op50::GetGlobal, 0, 5),
        ins_abx(Op50::LoadK, 1, 6),
        ins_abc(Op50::Call, 0, 2, 1),
        // OnObjectKillName("door01")
        ins_abx(Op50::GetGlobal, 0, 7),
        ins_abx(Op50::LoadK, 1, 8),
        ins_abc(Op50::Call, 0, 2, 1),
        ins_abc(Op50::Return, 0, 1, 0),
    ];
    let chunk = make_chunk(&consts, &code);
    let proto = undump(&chunk).unwrap();
    let env = evaluate_toplevel(proto);

    let sp = env.table(env.sp_listbox_id).unwrap();
    assert_eq!(sp.array_len(), 1);
    let entry_tid = match sp.get_int(1).unwrap() {
        LuaVal::Table(id) => *id,
        other => panic!("unexpected listbox entry {other:?}"),
    };
    let entry = env.table(entry_tid).unwrap();
    assert_eq!(
        entry.get_str("mapluafile").unwrap().as_str(),
        Some("MOD%s_%s")
    );
    assert_eq!(entry.get_str("era_g").unwrap().as_i64(), Some(1));
    assert!(env.calls.iter().any(|c| c.func == "AddDownloadableContent"));
    assert!(env
        .unsupported
        .iter()
        .any(|u| u.api == "OnObjectKillName" && u.detail.contains("door01")));
}

#[test]
fn standalone_registration_respects_dependency_guard() {
    let rk = 128;
    let constants = [
        LuaConst::Str("ScriptCB_IsFileExist".into()),
        LuaConst::Str("../../addon/Optional/SIDE/optional.lvl".into()),
        LuaConst::Num(1.0),
        LuaConst::Str("AddDownloadableContent".into()),
        LuaConst::Str("DEMO".into()),
        LuaConst::Str("demo_optional".into()),
        LuaConst::Str("democ_con".into()),
    ];
    let code = [
        ins_abx(Op50::GetGlobal, 0, 0),
        ins_abx(Op50::LoadK, 1, 1),
        ins_abc(Op50::Call, 0, 2, 2),
        ins_abc(Op50::Eq, 0, 0, rk + 2),
        ins_abx(Op50::Jmp, 0, 131_071 + 4),
        ins_abx(Op50::GetGlobal, 0, 3),
        ins_abx(Op50::LoadK, 1, 4),
        ins_abx(Op50::LoadK, 2, 5),
        ins_abc(Op50::Call, 0, 3, 1),
        ins_abx(Op50::GetGlobal, 0, 3),
        ins_abx(Op50::LoadK, 1, 4),
        ins_abx(Op50::LoadK, 2, 6),
        ins_abc(Op50::Call, 0, 3, 1),
        ins_abc(Op50::Return, 0, 1, 0),
    ];
    let env = evaluate_standalone_registration(undump(&make_chunk(&constants, &code)).unwrap());
    let registered: Vec<_> = env
        .calls
        .iter()
        .filter(|call| call.func == "AddDownloadableContent")
        .filter_map(|call| call.args[1].as_str())
        .collect();
    assert_eq!(registered, ["democ_con"]);
}

#[test]
fn a_generic_for_visits_each_entry_then_leaves_the_loop() {
    let rk = 128;
    let constants = [
        LuaConst::Num(1.0),
        LuaConst::Str("first".into()),
        LuaConst::Num(2.0),
        LuaConst::Str("second".into()),
        LuaConst::Str("pairs".into()),
        LuaConst::Str("AddDownloadableContent".into()),
        LuaConst::Str("after".into()),
    ];
    // local t = { "first", "second" }
    // for k, v in pairs(t) do AddDownloadableContent(v) end
    // AddDownloadableContent("after")
    let code = [
        ins_abx(Op50::NewTable, 0, 0),
        ins_abc(Op50::SetTable, 0, rk, rk + 1),
        ins_abc(Op50::SetTable, 0, rk + 2, rk + 3),
        ins_abx(Op50::GetGlobal, 1, 4),
        ins_abc(Op50::Move, 2, 0, 0),
        ins_abc(Op50::Call, 1, 2, 5),
        ins_abx(Op50::TForPrep, 1, 131_071 + 3),
        ins_abx(Op50::GetGlobal, 5, 5),
        ins_abc(Op50::Move, 6, 4, 0),
        ins_abc(Op50::Call, 5, 2, 1),
        ins_abc(Op50::TForLoop, 1, 0, 1),
        ins_abx(Op50::Jmp, 0, 131_071 - 5),
        ins_abx(Op50::GetGlobal, 5, 5),
        ins_abx(Op50::LoadK, 6, 6),
        ins_abc(Op50::Call, 5, 2, 1),
        ins_abc(Op50::Return, 0, 1, 0),
    ];
    let env = evaluate_standalone_registration(undump(&make_chunk(&constants, &code)).unwrap());
    let passed: Vec<_> = env
        .calls
        .iter()
        .filter(|call| call.func == "AddDownloadableContent")
        .filter_map(|call| call.args[0].as_str())
        .collect();
    assert_eq!(passed, ["first", "second", "after"]);
    assert!(env.warnings.is_empty(), "{:?}", env.warnings);
}
