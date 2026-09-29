//! Host-boundary proof, not proposed March source syntax for store operations.
use march_research::fast::{Context, Executor, store::Store, stream};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut program = stream::seed()?;
    let word = stream::compile(&mut program, "6 7 * ' dup tuple 2")?;
    let mut executor = Executor::new(&program);
    let outputs = executor.start(word, &[], &Context::new(), 10_000)?;
    let original = Store::new();
    let stored = executor.store_put(&original, &["demo", "result"], outputs[0])?;
    drop(executor);
    let value = stored.get(&["demo", "result"])?.unwrap();
    println!("empty snapshot: {}", original.cid());
    println!("stored snapshot: {}", stored.cid());
    println!("frozen value: {} {:?}", value.cid(), value.nodes());
    let input = value.to_input_graph(&program)?;
    let mut reader = Executor::new(&program);
    let outputs = reader.start_graph(
        program.lookup("dup").unwrap(),
        &input,
        &[value.root()],
        &Context::new(),
        10_000,
    )?;
    assert_eq!(reader.content_id(outputs[0])?, value.cid());
    assert!(original.is_empty());
    assert_eq!(stored.without(&["demo", "result"])?, original);
    Ok(())
}
