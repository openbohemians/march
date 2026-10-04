#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Blob, Driver, Error, Image, Machine, Op, Primitive};

fn seed() -> Image {
    assembler::assemble(include_str!("../seed/system.asm")).unwrap()
}
fn boot() -> Driver {
    Driver::boot(&seed()).unwrap()
}
fn word_cid(d: &mut Driver, name: &str) -> march7::Cid {
    d.evaluate(&format!("quote {name}")).unwrap();
    let xt = d.machine.stack.pop().unwrap();
    d.machine.cid(xt).unwrap()
}

#[test]
fn primitive_wire_ids_are_pinned() {
    use Primitive::*;
    let primitives = [
        Dup,
        Drop,
        Swap,
        Over,
        Rot,
        Add,
        Sub,
        Mul,
        Div,
        Mod,
        Eq,
        Lt,
        And,
        Or,
        Xor,
        Not,
        Shl,
        Shr,
        Load8,
        Store8,
        Load64,
        Store64,
        RegionNew,
        RegionFree,
        RegionSize,
        Execute,
        Seal,
        CodeCid,
        Resolve,
        Publish,
        BlobCid,
        BlobRead,
        Trap,
        ScratchPush,
        ScratchPop,
        ScratchPeek,
        FAdd,
        FSub,
        FMul,
        FDiv,
        FEq,
        FLt,
        IToF,
        FToI,
        WorkLoad,
        WorkStore,
        Mark,
        Gather,
        VecLen,
        VecAt,
        VecPush,
        VecSet,
    ];
    for (id, p) in primitives.into_iter().enumerate() {
        assert_eq!(
            march7::code::encode(&[Op::Prim(p), Op::Return]),
            [2, id as u8, 0]
        );
        assert_eq!(
            march7::code::decode(&[2, id as u8, 0]).unwrap(),
            [Op::Prim(p), Op::Return]
        );
    }
    for id in 52..=255 {
        assert_eq!(Primitive::decode(id), Err(Error::InvalidCode));
    }
}

#[test]
fn rejected_code_does_not_poison_saving() {
    let mut d = boot();
    d.evaluate(": square dup u* ;").unwrap();
    // An immediate word emits a call to a nonexistent CID into its caller.
    d.evaluate(&format!(": bad 3 c, {} ; immediate", "0 c, ".repeat(32)))
        .unwrap();
    let before = d.snapshot().unwrap().encode().unwrap();
    assert_eq!(d.evaluate(": victim bad ;"), Err(Error::InvalidCode));
    d.evaluate("7 square").unwrap();
    assert_eq!(d.machine.stack, [49]);
    let after = d.snapshot().unwrap().encode().unwrap();
    assert_eq!(after, before);
    let mut restored = Driver::boot(&Image::decode(&after).unwrap()).unwrap();
    restored.evaluate("8 square").unwrap();
    assert_eq!(restored.machine.stack, [64]);
    assert_eq!(restored.evaluate("victim"), Err(Error::User(1)));
}

#[test]
fn unsigned_system_words_are_explicit() {
    let mut d = boot();
    d.evaluate("-1 0 ult? -1 2 u/ -1 2 umod -1 1 u+ 0 1 u- 3 7 u*")
        .unwrap();
    assert_eq!(d.machine.stack, [0, u64::MAX / 2, 1, 0, u64::MAX, 21]);
    for name in ["+", "-", "*", "/", "mod", "lt?"] {
        assert_eq!(d.evaluate(name), Err(Error::User(1)));
    }
}

#[test]
fn freed_region_slots_are_reused_without_reviving_old_handles() {
    let mut m = Machine::empty();
    let old = m.input(b"old").unwrap();
    m.release(old).unwrap();
    for _ in 0..100_100 {
        let fresh = m.input(b"new").unwrap();
        assert_ne!(fresh, old);
        assert_eq!(fresh as u32, old as u32);
        assert_eq!(m.read(fresh, 0, 3).unwrap(), b"new");
        assert_eq!(m.read(old, 0, 3), Err(Error::Memory));
        assert_eq!(m.write(old, 0, b"x"), Err(Error::Memory));
        assert_eq!(m.release(old), Err(Error::Memory));
        m.release(fresh).unwrap();
    }
    assert_eq!(m.stats.peak_live_bytes, 1024 * 1024 + 3);
}

#[test]
fn repeated_inputs_do_not_exhaust_region_slots() {
    let mut d = boot();
    for _ in 0..100_100 {
        d.evaluate("1 drop").unwrap();
    }
    assert!(d.machine.stack.is_empty());
    d.evaluate(": square dup u* ; 7 square").unwrap();
    assert_eq!(d.machine.stack, [49]);
    let mut restored = Driver::boot(&d.snapshot().unwrap()).unwrap();
    restored.evaluate("8 square").unwrap();
    assert_eq!(restored.machine.stack, [64]);
}

#[test]
fn square_and_immediate_self_extension() {
    let mut d = boot();
    d.evaluate(": square dup u* ; 7 square").unwrap();
    assert_eq!(d.machine.stack, [49]);
    d.evaluate(": define word begin ; immediate define cube dup dup u* u* ; 3 cube")
        .unwrap();
    d.evaluate(": forty-two 40 2 u+ literal ; immediate : answer forty-two ; answer")
        .unwrap();
    assert_eq!(d.machine.stack, [49, 27, 42]);
    d.evaluate(": plain-answer 42 ;").unwrap();
    assert_eq!(word_cid(&mut d, "answer"), word_cid(&mut d, "plain-answer"));
}

#[test]
fn saved_image_reconstructs_bindings_and_immediate_flags() {
    let mut d = boot();
    d.evaluate(": define word begin ; immediate define square dup u* ;")
        .unwrap();
    let cid = word_cid(&mut d, "square");
    let image = d.snapshot().unwrap();
    let bytes = image.encode().unwrap();
    for _ in 0..3 {
        let mut next = Driver::boot(&Image::decode(&bytes).unwrap()).unwrap();
        assert_eq!(word_cid(&mut next, "square"), cid);
        assert_eq!(next.snapshot().unwrap().encode().unwrap(), bytes);
        next.evaluate("define cube dup dup u* u* ; 7 square 3 cube")
            .unwrap();
        assert_eq!(next.machine.stack, [49, 27]);
    }
}

#[test]
fn failed_definition_keeps_old_binding_and_prior_successful_work() {
    let mut d = boot();
    d.evaluate(": square dup u* ; 11").unwrap();
    let before = word_cid(&mut d, "square");
    assert_eq!(
        d.evaluate(": completed 5 ; : square nonexistent ;"),
        Err(Error::User(1))
    );
    assert_eq!(d.machine.stack, [11]);
    assert_eq!(word_cid(&mut d, "square"), before);
    d.evaluate("7 square completed : next 23 ; next").unwrap();
    assert_eq!(d.machine.stack, [11, 49, 5, 23]);
    assert_eq!(d.evaluate(": square dup"), Err(Error::User(3)));
    d.evaluate("2 square").unwrap();
    assert_eq!(d.machine.stack.last(), Some(&4));
}

#[test]
fn early_binding_survives_redefinition_and_reload() {
    let mut d = boot();
    d.evaluate(": foo 1 ; : old foo ; : foo 2 ; : new foo ;")
        .unwrap();
    let mut d = Driver::boot(&d.snapshot().unwrap()).unwrap();
    d.evaluate("old new").unwrap();
    assert_eq!(d.machine.stack, [1, 2]);
}

#[test]
fn numbers_first_checked_conversion_and_comments_are_words() {
    let mut d = boot();
    d.evaluate(": 42 999 ; 42 quote 42 call -- ignore : bad\n -3 +4 u+")
        .unwrap();
    assert_eq!(d.machine.stack, [42, 999, 1]);
    d.evaluate("-9223372036854775808 9223372036854775807")
        .unwrap();
    assert_eq!(&d.machine.stack[3..], [1u64 << 63, i64::MAX as u64]);
    for n in ["9223372036854775808", "-9223372036854775809"] {
        assert_eq!(d.evaluate(n), Err(Error::User(2)));
    }
    d.evaluate(": 999999999999999999999x 8 ; 999999999999999999999x")
        .unwrap();
    assert_eq!(d.machine.stack.last(), Some(&8));
}

#[test]
fn closed_quotation_is_a_cid_dependency() {
    let mut d = boot();
    d.evaluate(": foo 2 ; : deferred quote foo ; : foo 9 ; deferred call")
        .unwrap();
    assert_eq!(d.machine.stack, [2]);
    let mut d = Driver::boot(&d.snapshot().unwrap()).unwrap();
    d.evaluate("deferred call").unwrap();
    assert_eq!(d.machine.stack, [2]);
}

#[test]
fn independent_compilers_share_only_immutable_inputs() {
    let image = seed();
    let mut a = Driver::boot(&image).unwrap();
    let mut b = Driver::boot(&image).unwrap();
    a.evaluate(": square dup u* ;").unwrap();
    b.evaluate(": square dup u* ;").unwrap();
    assert_eq!(word_cid(&mut a, "square"), word_cid(&mut b, "square"));
    a.evaluate(": only-a 1 ;").unwrap();
    assert_eq!(b.evaluate("only-a"), Err(Error::User(1)));
}

#[test]
fn malformed_images_and_dependencies_are_rejected() {
    let image = seed();
    let bytes = image.encode().unwrap();
    for n in [0, 1, 7, 70, bytes.len() - 1] {
        assert!(Image::decode(&bytes[..n]).is_err());
    }
    let mut bad = bytes.clone();
    *bad.last_mut().unwrap() ^= 1;
    assert!(Image::decode(&bad).is_err());
    let mut bad = bytes;
    bad.push(0);
    assert!(Image::decode(&bad).is_err());
    let mut image = image;
    image.blobs.remove(&image.entry);
    assert!(Driver::boot(&image).is_err());
    let mut m = Machine::empty();
    assert_eq!(
        m.define(&[Op::Call([9; 32]), Op::Return]),
        Err(Error::InvalidCode)
    );
    assert_eq!(
        m.define(&[Op::Branch(20), Op::Return]),
        Err(Error::InvalidCode)
    );
}

#[test]
fn runtime_limits_errors_and_region_lifetimes() {
    let mut m = Machine::empty();
    let run = |m: &mut Machine, ops: &[Op]| {
        let xt = m.define(ops).unwrap();
        m.run(xt, 100)
    };
    assert_eq!(run(&mut m, &[Op::Branch(0)]), Err(Error::Fuel));
    assert_eq!(
        run(&mut m, &[Op::Prim(Primitive::Drop), Op::Return]),
        Err(Error::Stack)
    );
    assert_eq!(
        run(
            &mut m,
            &[Op::Lit(1), Op::Lit(0), Op::Prim(Primitive::Div), Op::Return]
        ),
        Err(Error::Arithmetic)
    );
    assert_eq!(
        run(
            &mut m,
            &[
                Op::Lit(1),
                Op::Lit(64),
                Op::Prim(Primitive::Shl),
                Op::Return
            ]
        ),
        Err(Error::Arithmetic)
    );
    assert_eq!(
        run(
            &mut m,
            &[
                Op::Lit(1),
                Op::Lit(3),
                Op::Prim(Primitive::Load64),
                Op::Return
            ]
        ),
        Err(Error::Memory)
    );
    let r = m.input(b"x").unwrap();
    assert_eq!(m.write(r, 0, b"y"), Err(Error::ReadOnly));
    m.release(r).unwrap();
    let other = m.input(b"z").unwrap();
    assert_ne!(r, other);
    assert_eq!(m.read(r, 0, 1), Err(Error::Memory));
    assert_eq!(m.run(u64::MAX, 100), Err(Error::InvalidToken));
}

#[test]
fn data_references_are_read_only_and_survive_images() {
    let mut image = seed();
    let b = Blob::Data(b"hello".to_vec());
    let cid = b.cid();
    image.blobs.insert(cid, b);
    let code = Blob::Code(march7::code::encode(&[Op::Data(cid), Op::Return]));
    let entry = code.cid();
    image.blobs.insert(entry, code);
    image.entry = entry;
    let bytes = image.encode().unwrap();
    let image = Image::decode(&bytes).unwrap();
    let mut m = Machine::new(&image).unwrap();
    let xt = m.link(entry).unwrap();
    m.run(xt, 100).unwrap();
    let r = m.stack[0];
    assert_eq!(m.stack[1..], [0, 5]);
    assert_eq!(m.read(r, 0, 5).unwrap(), b"hello");
    assert_eq!(m.write(r, 0, b"j"), Err(Error::ReadOnly));
}

#[test]
fn arithmetic_and_tail_calls_are_explicit() {
    let mut m = Machine::empty();
    let target = m
        .define(&[
            Op::Lit(u64::MAX),
            Op::Lit(1),
            Op::Prim(Primitive::Add),
            Op::Return,
        ])
        .unwrap();
    let c = m.cid(target).unwrap();
    let entry = m.define(&[Op::Tail(c)]).unwrap();
    m.run(entry, 100).unwrap();
    assert_eq!(m.stack, [0]);
    assert_eq!(m.stats.peak_control, 0);
}

#[test]
fn fuel_failure_abandons_unfinished_definition() {
    let mut d = boot();
    d.evaluate(": square dup u* ; : forever recur ; immediate")
        .unwrap();
    d.fuel = 5_000;
    assert_eq!(d.evaluate(": square forever ;"), Err(Error::Fuel));
    d.fuel = 10_000_000;
    d.evaluate("7 square").unwrap();
    assert_eq!(d.machine.stack, [49]);
}

#[test]
fn assembler_has_no_forth_source_or_macro_language() {
    assert!(assembler::assemble(": square dup u* ;").is_err());
    assert!(assembler::assemble("word x\nmacro y\nend\nentry x\ndata z\nroot z").is_err());
    assert!(assembler::assemble("word x\ncall x\nend\nentry x\ndata z\nroot z").is_err());
    let code = include_str!("../src/main.rs");
    assert!(!code.contains("assembler"));
}

#[test]
fn normal_binary_boots_compiles_and_reloads_without_the_assembler() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("march7-boot-{}-{unique}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let input = dir.join("input.image");
    let output = dir.join("output.image");
    std::fs::write(&input, seed().encode().unwrap()).unwrap();
    let first = std::process::Command::new(env!("CARGO_BIN_EXE_march7"))
        .current_dir(&dir)
        .env("PATH", "")
        .arg(&input)
        .args([
            "--eval",
            ": define word begin ; immediate define square dup u* ; 7 square",
            "--save",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&first.stdout).trim(), "[49]");
    let next = std::process::Command::new(env!("CARGO_BIN_EXE_march7"))
        .current_dir(&dir)
        .env("PATH", "")
        .arg(&output)
        .args(["--eval", "define cube dup dup u* u* ; 7 square 3 cube"])
        .output()
        .unwrap();
    assert!(
        next.status.success(),
        "{}",
        String::from_utf8_lossy(&next.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&next.stdout).trim(), "[49, 27]");
    std::fs::remove_file(input).unwrap();
    std::fs::remove_file(output).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn repeated_failed_attempts_release_definition_buffers() {
    let mut d = boot();
    d.evaluate(": square dup u* ;").unwrap();
    for _ in 0..100 {
        assert_eq!(d.evaluate(": square not-a-word ;"), Err(Error::User(1)));
    }
    d.evaluate("7 square").unwrap();
    assert_eq!(d.machine.stack, [49]);
    assert!(d.machine.stats.allocated_bytes > 6_000_000);
    assert!(d.machine.stats.peak_live_bytes < 1_200_000);
}

#[test]
fn two_compiler_words_rebuild_in_march_with_identical_cids() {
    let mut d = boot();
    let colon = word_cid(&mut d, ":");
    let literal = word_cid(&mut d, "literal");
    for _ in 0..3 {
        d.evaluate(": : word begin ; : literal 1 c, , ;").unwrap();
        assert_eq!(word_cid(&mut d, ":"), colon);
        assert_eq!(word_cid(&mut d, "literal"), literal);
        d = Driver::boot(&d.snapshot().unwrap()).unwrap();
        d.evaluate(": square dup u* ; 7 square").unwrap();
        assert_eq!(d.machine.stack, [49]);
        d.machine.stack.clear();
    }
}

#[test]
fn canonical_bytes_and_cid_are_pinned_independently_of_rust_layout() {
    let bytes = march7::code::encode(&[Op::Lit(42), Op::Return]);
    assert_eq!(bytes, [1, 42, 0, 0, 0, 0, 0, 0, 0, 0]);
    let cid = Blob::Code(bytes).cid();
    let hex = cid.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(
        hex,
        "cb91fa4b7e86b3dbf2dc11ebca29ca64c1c854cf028ed153904554bcb939440c"
    );
    assert_eq!(
        march7::code::decode(&vec![0; march7::code::MAX_WORD_OPS + 1]),
        Err(Error::Limit)
    );
}

#[test]
fn tail_recur_is_opcode_eleven_and_twelve_is_unknown() {
    assert_eq!(march7::code::encode(&[Op::TailRecur, Op::Return]), [11, 0]);
    assert_eq!(
        march7::code::decode(&[11, 0]).unwrap(),
        [Op::TailRecur, Op::Return]
    );
    for op in 12..=255u8 {
        assert_eq!(march7::code::decode(&[op, 0]), Err(Error::InvalidCode));
    }
}
