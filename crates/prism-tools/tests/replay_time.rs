//! Times a recording's replay (set PRISM_REPLAY to its path). Ignored: a
//! tool for finding slow inputs, not a test.

use prism_tools::{Recording, Session};

#[test]
#[ignore = "a timing tool: PRISM_REPLAY=file cargo test --release -- --ignored --nocapture"]
fn time_a_replay() {
    let Ok(path) = std::env::var("PRISM_REPLAY") else {
        return;
    };
    let r: Recording = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let t = std::time::Instant::now();
    let s = Session::replay(&r);
    println!(
        "replay: {:.2}s for {} events; matches: {}",
        t.elapsed().as_secs_f64(),
        r.events.len(),
        r.ends_with_document(s.document())
    );
}
