//! Float primitives and literals (docs/NUMBERS.md), on a rebuilt system.
#[path = "../tools/assembler.rs"]
mod assembler;
use march7::{Driver, Error, Image};

fn system() -> Image {
    let g0 = assembler::assemble(include_str!("../seed/system.asm")).unwrap();
    let mut d = Driver::boot(&g0).unwrap();
    d.fuel = 120_000_000;
    d.evaluate(include_str!("../seed/system.march")).unwrap();
    let boot = d.machine.stack.pop().unwrap();
    d.system_image(boot).unwrap()
}

fn run(image: &Image, source: &str) -> Result<Vec<u64>, Error> {
    let mut d = Driver::boot(image).unwrap();
    d.fuel = 100_000_000;
    d.evaluate(source)?;
    Ok(d.machine.stack.clone())
}

fn bits(xs: &[f64]) -> Vec<u64> {
    xs.iter().map(|x| x.to_bits()).collect()
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn literals_match_rusts_correctly_rounded_parser() {
    let g = system();
    let mut rng = Rng(2026);
    let mut literals = vec![
        "1.5",
        "0.1",
        "-0.0",
        "2.5e3",
        "1e3",
        "6.02e23",
        "1.797693134862315e2",
        "0.3",
        "-123.456",
        "9007199254740992.0",
        "1e22",
        "1e-22",
        "+4.25",
        "5E-1",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    // Random literals on the exact fast path: mantissa at most 2^53, decimal
    // exponent within +-22, in varied spellings.
    for _ in 0..400 {
        let m = rng.next() % (1 << 53);
        let digits = m.to_string();
        let split = 1 + (rng.next() as usize) % digits.len();
        let (int, frac) = digits.split_at(split);
        let effective = (rng.next() % 45) as i64 - 22;
        let exponent = effective + frac.len() as i64;
        let sign = if rng.next().is_multiple_of(3) {
            "-"
        } else {
            ""
        };
        let body = if frac.is_empty() {
            format!("{int}.0")
        } else {
            format!("{int}.{frac}")
        };
        literals.push(match rng.next() % 3 {
            0 if exponent == 0 => format!("{sign}{body}"),
            1 => format!("{sign}{body}e{exponent}"),
            _ => format!("{sign}{body}E{exponent}"),
        });
    }
    let expected: Vec<u64> = literals
        .iter()
        .map(|l| l.parse::<f64>().unwrap().to_bits())
        .collect();
    assert_eq!(run(&g, &literals.join(" ")).unwrap(), expected);
}

#[test]
fn arithmetic_is_ieee_with_one_canonical_nan() {
    let g = system();
    assert_eq!(
        run(&g, "1.5 2.25 f+ 10.0 4.0 f/ 3.0 0.5 f* 1.0 0.25 f-").unwrap(),
        bits(&[3.75, 2.5, 1.5, 0.75])
    );
    assert_eq!(
        run(&g, "1.0 0.0 f/ -1.0 0.0 f/ 0.0 0.0 f/").unwrap(),
        bits(&[f64::INFINITY, f64::NEG_INFINITY, f64::NAN])
    );
    // NaN is unequal to itself; comparisons give 0 or 1.
    assert_eq!(
        run(&g, "0.0 0.0 f/ dup feq? 1.0 2.0 flt? 2.0 2.0 feq?").unwrap(),
        [0, 1, 1]
    );
}

#[test]
fn conversions_truncate_and_reject_what_does_not_fit() {
    let g = system();
    assert_eq!(run(&g, "7 i>f -7 i>f").unwrap(), bits(&[7.0, -7.0]));
    assert_eq!(run(&g, "-7.9 f>i 7.9 f>i").unwrap(), [(-7i64) as u64, 7]);
    for bad in ["1.0 0.0 f/ f>i", "0.0 0.0 f/ f>i", "1e19 f>i"] {
        assert_eq!(run(&g, bad), Err(Error::Arithmetic), "{bad}");
    }
}

#[test]
fn literal_syntax_and_its_limits() {
    let g = system();
    // Integers are unchanged; a float needs a dot or an exponent.
    assert_eq!(run(&g, "42 -7").unwrap(), [42, (-7i64) as u64]);
    assert_eq!(run(&g, "1."), Err(Error::User(1)));
    assert_eq!(run(&g, "1.2.3"), Err(Error::User(1)));
    // Trailing zeros cost nothing, even past 2^53 digits of mantissa.
    assert_eq!(
        run(
            &g,
            "9007199254740992.0 100000000000000000000.0 1.500000000000000000000"
        )
        .unwrap(),
        bits(&[9007199254740992.0, 1e20, 1.5])
    );
    // Excess powers of ten move into the mantissa while it stays exact.
    assert_eq!(run(&g, "1e30 25e25").unwrap(), bits(&[1e30, 25e25]));
    // Beyond the exact fast path: trap instead of rounding wrongly.
    for big in [
        "99999999999999999999.0",
        "1e40",
        "1.5e-30",
        "1.7976931348623157e2",
    ] {
        assert_eq!(run(&g, big), Err(Error::User(21)), "{big}");
    }
    // A zero mantissa is zero at any exponent.
    assert_eq!(run(&g, "0.0e100").unwrap(), bits(&[0.0]));
}

#[test]
fn the_checker_and_quotations_handle_floats() {
    let g = system();
    assert_eq!(
        run(&g, ": avg f+ 2.0 f/ ; 3.0 4.0 avg ' avg stack-effect").unwrap(),
        [3.5f64.to_bits(), 2, 1, 1]
    );
    assert_eq!(
        run(&g, ": half [ 0.5 f* ] call ; 9.0 half").unwrap(),
        bits(&[4.5])
    );
    // A float literal is its own opcode in canonical code.
    let mut d = Driver::boot(&g).unwrap();
    d.evaluate(": c 2.0 ; ' c").unwrap();
    let xt = d.machine.stack.pop().unwrap();
    let cid = d.machine.cid(xt).unwrap();
    let image = d.snapshot().unwrap();
    let march7::Blob::Code(bytes) = &image.blobs[&cid] else {
        panic!("code")
    };
    assert_eq!(
        march7::code::decode(bytes).unwrap(),
        [march7::Op::Float(2.0f64.to_bits()), march7::Op::Return]
    );
}
