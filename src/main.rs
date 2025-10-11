// March2 FORTH - Bootstrap Architecture
//
// Building a proper FORTH from the ground up:
// - Tiny core in Rust
// - Everything else is words (including : and ;)
// - Bootstrap from minimal primitives

mod value;
mod xt;
mod word;
mod forth;
mod input;
mod repl;

fn main() {
    repl::run();
}
