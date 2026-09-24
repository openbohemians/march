//! Adversarial B0b reader-primitive tests from @march-claude.
//!
//! These complement `reader_primitives.rs` and `reader_graph.rs`.  Tokenizer
//! and integer tests compare every result against an independent reference
//! written here, over exhaustive cursor sweeps and generated inputs.  Other
//! tests probe dynamic record keys, guard-level use in a name/number
//! dispatcher, budgets, reflection descriptors, and capability safety.

use march_research::{Atom, Bindings, Cid, Clause, Node, ReduceError, Reducer, Store};

const BUDGET: usize = 1_000_000;
const WHITESPACE: [u8; 6] = [b' ', b'\t', b'\n', b'\r', 0x0b, 0x0c];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn atom(store: &mut Store, value: Atom) -> Cid {
    store.intern(Node::Const(value))
}

fn text(store: &mut Store, value: &str) -> Cid {
    atom(store, Atom::Text(value.into()))
}

fn int(store: &mut Store, value: i64) -> Cid {
    atom(store, Atom::Int(value))
}

fn run_with(store: &mut Store, root: Cid, budget: usize) -> Result<Cid, ReduceError> {
    Reducer::with_budget(store, &Bindings::new(), budget)
        .run(root)
        .map(|reduction| reduction.root)
}

fn run(store: &mut Store, root: Cid) -> Result<Cid, ReduceError> {
    run_with(store, root, BUDGET)
}

fn field(store: &Store, record: Cid, name: &str) -> Cid {
    let Some(Node::Record(fields)) = store.get(record) else {
        panic!("{} is not a record", store.format(record));
    };
    fields
        .iter()
        .find(|(key, _)| key == name)
        .unwrap_or_else(|| panic!("missing field {name}"))
        .1
}

fn atom_of(store: &Store, cid: Cid) -> Atom {
    match store.get(cid) {
        Some(Node::Const(value)) => value.clone(),
        other => panic!("expected a constant, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// NextToken.

/// Reference tokenizer: returns (found, token, successor position), or None
/// when the cursor is not a valid UTF-8 boundary of `input`.
fn reference_next(input: &str, offset: i64) -> Option<(bool, String, i64)> {
    let start = usize::try_from(offset).ok()?;
    if start > input.len() || !input.is_char_boundary(start) {
        return None;
    }
    let bytes = input.as_bytes();
    let mut index = start;
    while index < bytes.len() && WHITESPACE.contains(&bytes[index]) {
        index += 1;
    }
    let token_start = index;
    while index < bytes.len() && !WHITESPACE.contains(&bytes[index]) {
        index += 1;
    }
    Some((
        token_start != index,
        input[token_start..index].to_string(),
        index as i64,
    ))
}

fn next_token(
    store: &mut Store,
    input: &str,
    offset: i64,
) -> Result<(bool, String, i64), ReduceError> {
    let text = text(store, input);
    let position = int(store, offset);
    let root = store.intern(Node::NextToken { text, position });
    let result = run(store, root)?;
    let found = atom_of(store, field(store, result, "found"));
    let token = atom_of(store, field(store, result, "token"));
    let position = atom_of(store, field(store, result, "position"));
    match (found, token, position) {
        (Atom::Bool(found), Atom::Text(token), Atom::Int(position)) => Ok((found, token, position)),
        other => panic!("malformed next-token result {other:?}"),
    }
}

fn tokenize(store: &mut Store, input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut position = 0;
    loop {
        let (found, token, next) = next_token(store, input, position).unwrap();
        assert!(input.is_char_boundary(next as usize), "cursor split UTF-8");
        if !found {
            assert_eq!(next as usize, input.len());
            assert!(token.is_empty());
            return tokens;
        }
        assert!(next > position, "no progress on {input:?}");
        tokens.push(token);
        position = next;
    }
}

fn reference_tokens(input: &str) -> Vec<String> {
    input
        .split(|c: char| c.is_ascii() && WHITESPACE.contains(&(c as u8)))
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

/// Mixed ASCII whitespace, non-ASCII whitespace, multi-byte letters, control
/// bytes, and the punctuation the seed language will use.
const MIXED: &str = " \t:( é\u{a0}x\u{2003}\r\n\u{0b}\u{0c}dup;\0\u{7f}\u{3000}日本 ) ";

#[test]
fn every_cursor_matches_the_reference_tokenizer() {
    let mut store = Store::new();
    for offset in -2..=(MIXED.len() as i64 + 2) {
        match (
            reference_next(MIXED, offset),
            next_token(&mut store, MIXED, offset),
        ) {
            (Some(expected), Ok(actual)) => assert_eq!(actual, expected, "offset {offset}"),
            (None, Err(ReduceError::Type(_))) => {}
            (expected, actual) => panic!("offset {offset}: {expected:?} vs {actual:?}"),
        }
    }
    for extreme in [i64::MIN, i64::MAX] {
        assert!(matches!(
            next_token(&mut store, MIXED, extreme),
            Err(ReduceError::Type(_))
        ));
    }
}

#[test]
fn only_six_ascii_bytes_separate_tokens() {
    let mut store = Store::new();
    assert_eq!(
        tokenize(&mut store, MIXED),
        vec![":(", "é\u{a0}x\u{2003}", "dup;\0\u{7f}\u{3000}日本", ")",]
    );
    // Delimiters need whitespace: `square:` and `(dup` are single tokens.
    assert_eq!(
        tokenize(&mut store, "square: (dup *) ;"),
        vec!["square:", "(dup", "*)", ";"]
    );
    assert!(tokenize(&mut store, "").is_empty());
    assert!(tokenize(&mut store, " \t\n\r\u{0b}\u{0c}").is_empty());
}

#[test]
fn generated_inputs_tokenize_exactly_like_the_reference() {
    let alphabet = [
        " ", "\t", "\n", "\r", "\u{0b}", "\u{0c}", "a", "é", "\u{a0}", "\u{2003}", "\0", "(", ")",
        ";", ":", "\"", "7", "-", "日",
    ];
    let mut rng = Rng(0x5eed_1234_abcd_ef01);
    let mut store = Store::new();
    for sample in 0..300 {
        let input: String = (0..rng.below(40))
            .map(|_| alphabet[rng.below(alphabet.len())])
            .collect();
        assert_eq!(
            tokenize(&mut store, &input),
            reference_tokens(&input),
            "sample {sample}: {input:?}"
        );
    }
}

#[test]
fn next_token_rejects_ground_operands_of_the_wrong_type() {
    let mut store = Store::new();
    let seven = int(&mut store, 7);
    let word = text(&mut store, "word");
    let unit = atom(&mut store, Atom::Unit);
    for (text, position) in [(seven, seven), (word, word), (unit, seven), (word, unit)] {
        let root = store.intern(Node::NextToken { text, position });
        assert!(
            matches!(run(&mut store, root), Err(ReduceError::Type(_))),
            "{}",
            store.format(root)
        );
    }
}

// ---------------------------------------------------------------------------
// ParseInt.

fn parse_int(store: &mut Store, input: &str) -> Option<i64> {
    let text = text(store, input);
    let root = store.intern(Node::ParseInt(text));
    let result = run(store, root).unwrap();
    let found = atom_of(store, field(store, result, "found"));
    let value = atom_of(store, field(store, result, "value"));
    match (found, value) {
        (Atom::Bool(true), Atom::Int(value)) => Some(value),
        (Atom::Bool(false), Atom::Unit) => None,
        other => panic!("malformed parse-int result for {input:?}: {other:?}"),
    }
}

/// Reference parser: optional ASCII sign, then one or more ASCII digits,
/// checked in i64.
fn reference_parse(input: &str) -> Option<i64> {
    let digits = input.strip_prefix(['+', '-']).unwrap_or(input);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    input.parse::<i64>().ok()
}

#[test]
fn parse_int_limits_and_near_misses_match_the_reference() {
    let mut store = Store::new();
    let cases = [
        "0",
        "-0",
        "+0",
        "007",
        "-007",
        "9223372036854775807",
        "+9223372036854775807",
        "-9223372036854775808",
        "9223372036854775808",
        "+9223372036854775808",
        "-9223372036854775809",
        "99999999999999999999999999",
        "-00000000000000000000009223372036854775808",
        "",
        "+",
        "-",
        "+-1",
        "-+1",
        "--1",
        " 1",
        "1 ",
        "1_000",
        "1e3",
        "0x10",
        "1+",
        "1-",
        "٣",
        "１",
        "²",
        "7\0",
    ];
    for case in cases {
        assert_eq!(
            parse_int(&mut store, case),
            reference_parse(case),
            "{case:?}"
        );
    }
    assert_eq!(
        parse_int(&mut store, "-9223372036854775808"),
        Some(i64::MIN)
    );
    assert_eq!(parse_int(&mut store, "9223372036854775807"), Some(i64::MAX));
    // Classic FORTH word spellings stay names, not numbers.
    for word in ["1+", "1-", "+", "-", "2*", "0="] {
        assert_eq!(parse_int(&mut store, word), None, "{word}");
    }
}

#[test]
fn generated_integers_and_decorations_round_trip() {
    let mut rng = Rng(0x1357_9bdf_2468_ace0);
    let mut store = Store::new();
    for _ in 0..400 {
        let value = match rng.below(4) {
            0 => rng.next() as i64,
            1 => (rng.next() % 1000) as i64 - 500,
            2 => i64::MAX - (rng.next() % 10) as i64,
            _ => i64::MIN + (rng.next() % 10) as i64,
        };
        let plain = value.to_string();
        assert_eq!(parse_int(&mut store, &plain), Some(value));
        if value >= 0 {
            assert_eq!(parse_int(&mut store, &format!("+{plain}")), Some(value));
            assert_eq!(parse_int(&mut store, &format!("0{plain}")), Some(value));
        }
        // Appending a digit either overflows or scales exactly.
        let longer = format!("{plain}7");
        assert_eq!(parse_int(&mut store, &longer), reference_parse(&longer));
    }
}

#[test]
fn parse_int_of_a_non_text_ground_value_is_a_type_error() {
    let mut store = Store::new();
    let seven = int(&mut store, 7);
    let pair = store.intern(Node::Pair(seven, seven));
    for operand in [seven, pair] {
        let root = store.intern(Node::ParseInt(operand));
        assert!(matches!(run(&mut store, root), Err(ReduceError::Type(_))));
    }
}

// ---------------------------------------------------------------------------
// Lookup and PutKey.

fn lookup(store: &mut Store, record: Cid, key: &str) -> Result<(bool, Cid), ReduceError> {
    let key = text(store, key);
    let root = store.intern(Node::Lookup { record, key });
    let result = run(store, root)?;
    let found = matches!(
        atom_of(store, field(store, result, "found")),
        Atom::Bool(true)
    );
    Ok((found, field(store, result, "value")))
}

fn put_key(store: &mut Store, record: Cid, key: &str, value: Cid) -> Cid {
    let key = text(store, key);
    let root = store.intern(Node::PutKey { record, key, value });
    run(store, root).unwrap()
}

#[test]
fn dynamic_updates_are_order_independent_canonical_and_immutable() {
    let mut store = Store::new();
    let empty = store.intern(Node::Record(Vec::new()));
    let one = int(&mut store, 1);
    let two = int(&mut store, 2);
    let unit = atom(&mut store, Atom::Unit);

    let ab = {
        let a = put_key(&mut store, empty, "a", one);
        put_key(&mut store, a, "b", two)
    };
    let ba = {
        let b = put_key(&mut store, empty, "b", two);
        put_key(&mut store, b, "a", one)
    };
    let hand_built = store.intern(Node::Record(vec![("b".into(), two), ("a".into(), one)]));
    assert_eq!(ab, ba);
    assert_eq!(ab, hand_built);

    // Rebinding the same value is the identity; rebinding a new value leaves
    // the original record untouched.
    assert_eq!(put_key(&mut store, ab, "a", one), ab);
    let replaced = put_key(&mut store, ab, "a", two);
    assert_ne!(replaced, ab);
    assert_eq!(lookup(&mut store, ab, "a").unwrap(), (true, one));
    assert_eq!(lookup(&mut store, replaced, "a").unwrap(), (true, two));

    // Absent differs from present-Unit, including for the result record's own
    // field names and unusual keys.
    for key in ["found", "value", "op", "", "é", "日本", " ", "a\0"] {
        assert_eq!(
            lookup(&mut store, ab, key).unwrap(),
            (false, unit),
            "{key:?}"
        );
        let with_unit = put_key(&mut store, ab, key, unit);
        assert_eq!(
            lookup(&mut store, with_unit, key).unwrap(),
            (true, unit),
            "{key:?}"
        );
        assert_eq!(lookup(&mut store, with_unit, "b").unwrap(), (true, two));
    }
}

#[test]
fn lookup_returns_the_held_edge_itself() {
    let mut store = Store::new();
    let body = atom(&mut store, Atom::Unit);
    let quote = store.intern(Node::Quote { params: 0, body });
    let dictionary = store.intern(Node::Record(vec![("q".into(), quote)]));
    assert_eq!(lookup(&mut store, dictionary, "q").unwrap(), (true, quote));
    // A key spelling some CID is only a key.
    let spelled = quote.to_string();
    let unit = atom(&mut store, Atom::Unit);
    assert_eq!(
        lookup(&mut store, dictionary, &spelled).unwrap(),
        (false, unit)
    );
}

#[test]
fn lookup_and_put_key_reject_ground_operands_of_the_wrong_type() {
    let mut store = Store::new();
    let seven = int(&mut store, 7);
    let key = text(&mut store, "k");
    let record = store.intern(Node::Record(vec![("k".into(), seven)]));
    let body = atom(&mut store, Atom::Unit);
    let quote = store.intern(Node::Quote { params: 0, body });
    let pair = store.intern(Node::Pair(seven, seven));
    for not_record in [seven, key, pair, quote] {
        let root = store.intern(Node::Lookup {
            record: not_record,
            key,
        });
        assert!(matches!(run(&mut store, root), Err(ReduceError::Type(_))));
        let root = store.intern(Node::PutKey {
            record: not_record,
            key,
            value: seven,
        });
        assert!(matches!(run(&mut store, root), Err(ReduceError::Type(_))));
    }
    for not_key in [seven, pair, record] {
        let root = store.intern(Node::Lookup {
            record,
            key: not_key,
        });
        assert!(matches!(run(&mut store, root), Err(ReduceError::Type(_))));
    }
}

// ---------------------------------------------------------------------------
// Closed code: a miniature name/number dispatcher.

/// `resolve(token, dictionary) =
///    [ parse-int(token).found -> parse-int(token).value
///    ; true                  -> lookup(dictionary, token).value ]`
fn resolver(store: &mut Store) -> Cid {
    let token = store.intern(Node::Param(0));
    let dictionary = store.intern(Node::Param(1));
    let parsed = store.intern(Node::ParseInt(token));
    let is_number = store.intern(Node::Get {
        record: parsed,
        field: "found".into(),
    });
    let number = store.intern(Node::Get {
        record: parsed,
        field: "value".into(),
    });
    let entry = store.intern(Node::Lookup {
        record: dictionary,
        key: token,
    });
    let named = store.intern(Node::Get {
        record: entry,
        field: "value".into(),
    });
    let otherwise = atom(store, Atom::Bool(true));
    store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: is_number,
                body: number,
            },
            Clause {
                guard: otherwise,
                body: named,
            },
        ],
    })
}

#[test]
fn guards_can_dispatch_on_numbers_versus_names_with_residual_tokens() {
    let mut store = Store::new();
    let family = resolver(&mut store);
    let forty_two = int(&mut store, 42);
    let dictionary = store.intern(Node::Record(vec![("answer".into(), forty_two)]));
    let resolve = |store: &mut Store, token: Cid| {
        let call = store.intern(Node::Dispatch {
            family,
            arguments: vec![token, dictionary],
        });
        run(store, call)
    };

    let seven = text(&mut store, "-7");
    let result = resolve(&mut store, seven).unwrap();
    assert_eq!(store.get(result), Some(&Node::Const(Atom::Int(-7))));
    let name = text(&mut store, "answer");
    assert_eq!(resolve(&mut store, name).unwrap(), forty_two);
    let absent = text(&mut store, "1+");
    let unit = atom(&mut store, Atom::Unit);
    assert_eq!(resolve(&mut store, absent).unwrap(), unit);

    // An unknown token leaves one compact residual that later resolves.
    let hole = store.intern(Node::Hole("token".into()));
    let residual = resolve(&mut store, hole).unwrap();
    assert!(matches!(store.get(residual), Some(Node::Dispatch { .. })));
    let mut bindings = Bindings::new();
    bindings.insert("token", name);
    let resolved = Reducer::with_budget(&mut store, &bindings, BUDGET)
        .run(residual)
        .unwrap()
        .root;
    assert_eq!(resolved, forty_two);
}

// ---------------------------------------------------------------------------
// Budgets.

#[test]
fn scanning_parsing_and_lookup_charge_proportionally_to_input() {
    let mut store = Store::new();
    // Whitespace runs, long tokens, long numerals, and wide records must each
    // exhaust a budget smaller than their size rather than run for free.
    let spaces = format!("{}x", " ".repeat(5_000));
    let long_token = "y".repeat(5_000);
    let numeral = "0".repeat(5_000);
    let fields = (0..2_000)
        .map(|index| (format!("k{index:05}"), Node::Const(Atom::Int(index))))
        .collect::<Vec<_>>();
    let fields = fields
        .into_iter()
        .map(|(name, node)| (name, store.intern(node)))
        .collect();
    let wide = store.intern(Node::Record(fields));

    let zero = int(&mut store, 0);
    let spaces = text(&mut store, &spaces);
    let long_token = text(&mut store, &long_token);
    let numeral = text(&mut store, &numeral);
    let key = text(&mut store, "k01999");
    let roots = [
        store.intern(Node::NextToken {
            text: spaces,
            position: zero,
        }),
        store.intern(Node::NextToken {
            text: long_token,
            position: zero,
        }),
        store.intern(Node::ParseInt(numeral)),
        store.intern(Node::Lookup { record: wide, key }),
    ];
    for root in roots {
        assert!(
            matches!(
                run_with(&mut store, root, 1_000),
                Err(ReduceError::BudgetExhausted { .. })
            ),
            "{} ran within a budget smaller than its input",
            store.format(root).chars().take(40).collect::<String>()
        );
        assert!(run_with(&mut store, root, 20_000).is_ok());
    }
}

#[test]
fn budget_sweep_is_monotone_for_a_small_reader_step() {
    let mut store = Store::new();
    let input = text(&mut store, "   hello world");
    let zero = int(&mut store, 0);
    let root = store.intern(Node::NextToken {
        text: input,
        position: zero,
    });
    let mut succeeded = None;
    for budget in 0..200 {
        match run_with(&mut store, root, budget) {
            Ok(result) => {
                if let Some(previous) = succeeded {
                    assert_eq!(result, previous, "budget {budget}");
                }
                succeeded = Some(result);
            }
            Err(ReduceError::BudgetExhausted { .. }) => {
                assert!(succeeded.is_none(), "failure at {budget} after success");
            }
            Err(other) => panic!("budget {budget}: {other:?}"),
        }
    }
    assert!(succeeded.is_some());
}

// ---------------------------------------------------------------------------
// Reflection descriptors and capability safety.

fn description(store: &mut Store, operation: &str, fields: Vec<(&str, Cid)>) -> Cid {
    let operation = text(store, operation);
    let mut all = vec![("op".to_string(), operation)];
    all.extend(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_string(), value)),
    );
    store.intern(Node::Record(all))
}

fn param(store: &mut Store, index: i64) -> Cid {
    let index = int(store, index);
    description(store, "param", vec![("index", index)])
}

/// Build `Intern({op: quote, ...})` and report the store size before running,
/// so a rejected reflection can be checked for leaving no residue.
fn quote_intern_root(store: &mut Store, parameters: i64, body: Cid) -> (Cid, usize) {
    let parameters = int(store, parameters);
    let quote = description(
        store,
        "quote",
        vec![("parameters", parameters), ("body", body)],
    );
    let root = store.intern(Node::Intern(quote));
    (root, store.len())
}

#[test]
fn malformed_reader_descriptions_are_rejected() {
    let mut store = Store::new();
    let p0 = param(&mut store, 0);
    let p1 = param(&mut store, 1);
    let extra = text(&mut store, "extra");
    let cases = [
        description(&mut store, "next-token", vec![("text", p0)]),
        description(
            &mut store,
            "next-token",
            vec![("text", p0), ("position", p1), ("delimiter", extra)],
        ),
        description(&mut store, "parse-int", vec![]),
        description(&mut store, "parse-int", vec![("value", p0)]),
        description(&mut store, "lookup", vec![("record", p0)]),
        description(&mut store, "lookup", vec![("record", p0), ("field", p1)]),
        description(&mut store, "put-key", vec![("record", p0), ("key", p1)]),
        description(
            &mut store,
            "nexttoken",
            vec![("text", p0), ("position", p1)],
        ),
    ];
    for case in cases {
        let (root, before) = quote_intern_root(&mut store, 2, case);
        assert!(
            matches!(run(&mut store, root), Err(ReduceError::Reflection(_))),
            "{}",
            store.format(case)
        );
        assert_eq!(store.len(), before, "rejected reflection left nodes");
    }
}

#[test]
fn a_world_in_a_dictionary_cannot_be_forked_through_dynamic_keys() {
    let mut store = Store::new();
    let world = atom(&mut store, Atom::Trace(Vec::new()));
    let dictionary = store.intern(Node::Record(vec![("w".into(), world)]));
    let key = text(&mut store, "w");
    let prefix = text(&mut store, "w ");
    let zero = int(&mut store, 0);
    // A second key derived at runtime ("w" read from text), not a shared CID.
    let scanned = store.intern(Node::NextToken {
        text: prefix,
        position: zero,
    });
    let derived = store.intern(Node::Get {
        record: scanned,
        field: "token".into(),
    });
    let direct = store.intern(Node::Lookup {
        record: dictionary,
        key,
    });
    let indirect = store.intern(Node::Lookup {
        record: dictionary,
        key: derived,
    });
    let a = text(&mut store, "a");
    let b = text(&mut store, "b");
    let first = store.intern(Node::Get {
        record: direct,
        field: "value".into(),
    });
    let second = store.intern(Node::Get {
        record: indirect,
        field: "value".into(),
    });
    let emit_a = store.intern(Node::Emit {
        token: first,
        message: a,
    });
    let emit_b = store.intern(Node::Emit {
        token: second,
        message: b,
    });
    let root = store.intern(Node::Pair(emit_a, emit_b));
    assert!(matches!(
        run(&mut store, root),
        Err(ReduceError::LinearValueDuplicated(_))
    ));

    // The same fork attempted inside a quotation over a dictionary parameter.
    let dictionary_parameter = store.intern(Node::Param(0));
    let direct = store.intern(Node::Lookup {
        record: dictionary_parameter,
        key,
    });
    let indirect = store.intern(Node::Lookup {
        record: dictionary_parameter,
        key: derived,
    });
    let first = store.intern(Node::Get {
        record: direct,
        field: "value".into(),
    });
    let second = store.intern(Node::Get {
        record: indirect,
        field: "value".into(),
    });
    let emit_a = store.intern(Node::Emit {
        token: first,
        message: a,
    });
    let emit_b = store.intern(Node::Emit {
        token: second,
        message: b,
    });
    let body = store.intern(Node::Pair(emit_a, emit_b));
    let quote = store.intern(Node::Quote { params: 1, body });
    let apply = store.intern(Node::Apply {
        function: quote,
        arguments: vec![dictionary],
    });
    assert!(matches!(
        run(&mut store, apply),
        Err(ReduceError::LinearValueDuplicated(_))
    ));
}
