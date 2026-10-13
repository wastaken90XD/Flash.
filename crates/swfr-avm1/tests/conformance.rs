//! M4 AS1/AS2 Conformance Suite (`crates/swfr-avm1/tests/conformance.rs`).
//! Generates and runs 160 distinct SWF movies with `SwfBuilder` and `ActionBuilder`,
//! verifying every movie's `trace()` output against ECMA-262 3rd edition semantics (TSD 3.1.6, 3.6 M4).
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use swfr_avm1::Avm1;
use swfr_scene::Stage;
use swfr_swf::{Movie, Tag};
use swfr_testkit::{ActionBuilder, SwfBuilder};

struct ConformanceCase {
    name: String,
    swf_version: u8,
    actions: Vec<u8>,
    expected_trace: Vec<String>,
}

fn run_movie_case(case: &ConformanceCase) {
    let mut builder = SwfBuilder::new(case.swf_version, 320, 240, 60.0, 4);
    builder.push_tag(&Tag::DoAction {
        actions: case.actions.clone(),
    });
    builder.push_tag(&Tag::ShowFrame);
    builder.push_tag(&Tag::FrameLabel {
        name: "scene_two".to_owned(),
        is_anchor: false,
    });
    builder.push_tag(&Tag::ShowFrame);
    let swf_bytes = builder.build();

    let movie = Movie::parse(&swf_bytes).expect("conformance movie must parse");
    let mut stage = Stage::from_movie(&movie);
    let mut vm = Avm1::new(movie.header.version);

    let events = stage.step_frame();
    for (_, code) in events.actions {
        vm.execute_bytecode(&code, &mut stage);
    }

    assert_eq!(
        vm.trace_log, case.expected_trace,
        "AVM1 conformance failure in movie '{}'",
        case.name
    );
}

#[test]
#[allow(clippy::too_many_lines, clippy::arithmetic_side_effects)]
fn avm1_es3_conformance_160_generated_movies() {
    let mut cases: Vec<ConformanceCase> = Vec::with_capacity(160);

    // Group 1 (Movies 1..=30): ES3 Arithmetic (+, -, *, /, %, ++, --)
    for i in 1..=30_i32 {
        let a = i * 7 - 50;
        let b = (i % 9) + 1;
        let mut ab = ActionBuilder::new();
        ab.push_i32(a).push_i32(b).add2().trace();
        ab.push_i32(a).push_i32(b).sub().trace();
        ab.push_i32(a).push_i32(b).mul().trace();
        ab.push_i32(a).push_i32(b).modulo().trace();
        ab.push_i32(a).increment().trace();
        ab.push_i32(b).decrement().trace();
        cases.push(ConformanceCase {
            name: format!("arith_int_{i:02}"),
            swf_version: 8,
            actions: ab.build(),
            expected_trace: vec![
                (a + b).to_string(),
                (a - b).to_string(),
                (a * b).to_string(),
                (a % b).to_string(),
                (a + 1).to_string(),
                (b - 1).to_string(),
            ],
        });
    }

    // Group 2 (Movies 31..=55): Bitwise operations (&, |, ^, <<, >>, >>>)
    for i in 1..=25_i32 {
        let lhs = i.wrapping_mul(0x1357_9BDF_u32 as i32);
        let rhs = (i * 29) ^ 0x5A;
        let sh = (i as u32) & 0x1F;
        let mut ab = ActionBuilder::new();
        ab.push_i32(lhs).push_i32(rhs).bit_and().trace();
        ab.push_i32(lhs).push_i32(rhs).bit_or().trace();
        ab.push_i32(lhs).push_i32(rhs).bit_xor().trace();
        ab.push_i32(lhs).push_i32(sh as i32).shl().trace();
        ab.push_i32(lhs).push_i32(sh as i32).shr().trace();
        ab.push_i32(lhs).push_i32(sh as i32).ushr().trace();
        cases.push(ConformanceCase {
            name: format!("bitwise_{i:02}"),
            swf_version: 8,
            actions: ab.build(),
            expected_trace: vec![
                (lhs & rhs).to_string(),
                (lhs | rhs).to_string(),
                (lhs ^ rhs).to_string(),
                lhs.wrapping_shl(sh).to_string(),
                lhs.wrapping_shr(sh).to_string(),
                (lhs as u32).wrapping_shr(sh).to_string(),
            ],
        });
    }

    // Group 3 (Movies 56..=80): Coercions, SWF6 vs SWF8 undefined rules, typeof, equality, relational
    for i in 1..=25_i32 {
        let swf_ver = if i % 2 == 0 { 8 } else { 6 };
        let mut ab = ActionBuilder::new();
        // undefined + i
        ab.push_undefined().push_i32(i).add2().trace();
        // "val=" + i
        ab.push_str("val=").push_i32(i).add2().trace();
        // i == "i" (abstract equality) and i === "i" (strict equality)
        let s_num = i.to_string();
        ab.push_i32(i).push_str(&s_num).equals2().trace();
        ab.push_i32(i).push_str(&s_num).strict_equals().trace();
        // typeof checks
        ab.push_i32(i).typeof_op().trace();
        ab.push_str("abc").typeof_op().trace();
        ab.push_bool(i > 10).not().trace();

        let undef_plus = if swf_ver <= 6 {
            i.to_string()
        } else {
            "NaN".to_owned()
        };
        cases.push(ConformanceCase {
            name: format!("coercion_v{swf_ver}_{i:02}"),
            swf_version: swf_ver,
            actions: ab.build(),
            expected_trace: vec![
                undef_plus,
                format!("val={i}"),
                "true".to_owned(),
                "false".to_owned(),
                "number".to_owned(),
                "string".to_owned(),
                (i <= 10).to_string(),
            ],
        });
    }

    // Group 4 (Movies 81..=105): String operations & prototype methods
    for i in 1..=25_usize {
        let word = format!("FlashMovie_{i:02}_Test");
        let mut ab = ActionBuilder::new();
        ab.push_str(&word).string_len().trace();
        // word.toLowerCase()
        ab.push_i32(0)
            .push_str(&word)
            .push_str("toLowerCase")
            .call_method()
            .trace();
        // word.toUpperCase()
        ab.push_i32(0)
            .push_str(&word)
            .push_str("toUpperCase")
            .call_method()
            .trace();
        // word.substr(5, 8) (args pushed in reverse order for ActionCallMethod)
        ab.push_i32(8)
            .push_i32(5)
            .push_i32(2)
            .push_str(&word)
            .push_str("substr")
            .call_method()
            .trace();
        // word.indexOf("Movie")
        ab.push_str("Movie")
            .push_i32(1)
            .push_str(&word)
            .push_str("indexOf")
            .call_method()
            .trace();

        cases.push(ConformanceCase {
            name: format!("string_methods_{i:02}"),
            swf_version: 8,
            actions: ab.build(),
            expected_trace: vec![
                word.len().to_string(),
                word.to_lowercase(),
                word.to_uppercase(),
                word[5..13].to_owned(),
                "5".to_owned(),
            ],
        });
    }

    // Group 5 (Movies 106..=130): Arrays, Objects, Property access, and Closures
    for i in 1..=25_i32 {
        let mut ab = ActionBuilder::new();
        // Build array [i, i*2, i*3] (InitArray pops elements in order)
        ab.push_i32(i * 3)
            .push_i32(i * 2)
            .push_i32(i)
            .push_i32(3)
            .init_array();
        ab.push_str("arr").op(0x4D).set_var(); // StackSwap + SetVariable
                                               // arr.length
        ab.push_str("arr")
            .get_var()
            .push_str("length")
            .get_member()
            .trace();
        // arr.join("-")
        ab.push_str("-")
            .push_i32(1)
            .push_str("arr")
            .get_var()
            .push_str("join")
            .call_method()
            .trace();
        // User function `double_add(x)` returning `x * 2 + i`
        let mut fn_body = ActionBuilder::new();
        fn_body
            .push_str("x")
            .get_var()
            .push_i32(2)
            .mul()
            .push_i32(i)
            .add2()
            .ret();
        let body_bytes = fn_body.build();
        ab.define_function("double_add", &["x"], &body_bytes);
        ab.push_i32(10)
            .push_i32(1)
            .push_str("double_add")
            .call_function()
            .trace();

        cases.push(ConformanceCase {
            name: format!("arrays_and_functions_{i:02}"),
            swf_version: 8,
            actions: ab.build(),
            expected_trace: vec![
                "3".to_owned(),
                format!("{i}-{}-{}", i * 2, i * 3),
                (20 + i).to_string(),
            ],
        });
    }

    // Group 6 (Movies 131..=160): Math, Global functions, Prototypes, MovieClip timeline control
    for i in 1..=30_i32 {
        let mut ab = ActionBuilder::new();
        // Math.abs(-i)
        ab.push_i32(-i)
            .push_i32(1)
            .push_str("Math")
            .get_var()
            .push_str("abs")
            .call_method()
            .trace();
        // Math.max(i, 15)
        ab.push_i32(15)
            .push_i32(i)
            .push_i32(2)
            .push_str("Math")
            .get_var()
            .push_str("max")
            .call_method()
            .trace();
        // parseInt("0x20") + i
        ab.push_str("0x20")
            .push_i32(1)
            .push_str("parseInt")
            .call_function()
            .push_i32(i)
            .add2()
            .trace();
        // _root._totalframes
        ab.push_str("_root")
            .get_var()
            .push_str("_totalframes")
            .get_member()
            .trace();

        cases.push(ConformanceCase {
            name: format!("math_and_movieclip_{i:02}"),
            swf_version: 8,
            actions: ab.build(),
            expected_trace: vec![
                i.abs().to_string(),
                i.max(15).to_string(),
                (32 + i).to_string(),
                "4".to_owned(),
            ],
        });
    }

    assert_eq!(cases.len(), 160);
    for case in &cases {
        run_movie_case(case);
    }
}

#[test]
fn avm1_watchdog_aborts_infinite_loop_cleanly() {
    let mut ab = ActionBuilder::new();
    // Jump back -5 bytes to repeat the Jump instruction forever
    ab.jump(-5);
    let code = ab.build();

    let mut b = SwfBuilder::new(8, 100, 100, 60.0, 1);
    b.push_tag(&Tag::DoAction { actions: code });
    b.push_tag(&Tag::ShowFrame);
    let movie = Movie::parse(&b.build()).expect("valid SWF");
    let mut stage = Stage::from_movie(&movie);
    let mut vm = Avm1::new(8);
    vm.max_ops_per_run = 1_000;

    let ev = stage.step_frame();
    for (_, actions) in ev.actions {
        vm.execute_bytecode(&actions, &mut stage);
    }
    assert!(
        vm.watchdog_tripped,
        "infinite loop must trip the AVM1 instruction watchdog cleanly"
    );
}

#[test]
fn avm1_timers_key_and_gc_work_deterministically() {
    let mut fn_body = ActionBuilder::new();
    fn_body.push_str("timer_fired").trace().ret();
    let body_bytes = fn_body.build();

    let mut ab = ActionBuilder::new();
    ab.define_function("on_tick", &[], &body_bytes);
    // setTimeout("on_tick", 32) -> 2 frames at 60fps
    ab.push_i32(32)
        .push_str("on_tick")
        .push_i32(2)
        .push_str("setTimeout")
        .call_function()
        .op(0x17); // Pop
                   // Key.isDown(37)
    ab.push_i32(37)
        .push_i32(1)
        .push_str("Key")
        .get_var()
        .push_str("isDown")
        .call_method()
        .trace();

    let mut b = SwfBuilder::new(8, 100, 100, 60.0, 1);
    b.push_tag(&Tag::DoAction {
        actions: ab.build(),
    });
    b.push_tag(&Tag::ShowFrame);
    let movie = Movie::parse(&b.build()).expect("valid SWF");
    let mut stage = Stage::from_movie(&movie);
    let mut vm = Avm1::new(8);
    vm.set_key_state(37, true);

    let ev = stage.step_frame();
    for (_, actions) in ev.actions {
        vm.execute_bytecode(&actions, &mut stage);
    }
    assert_eq!(vm.trace_log, vec!["true"]);

    vm.step_timers(&mut stage, 16);
    assert_eq!(vm.trace_log, vec!["true"]);
    vm.step_timers(&mut stage, 16);
    assert_eq!(vm.trace_log, vec!["true", "timer_fired"]);
}
