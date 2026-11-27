use prism_tools::session::{Input, Recording, Session};

#[test]
#[ignore]
fn probe30() {
    let r: Recording = serde_json::from_str(include_str!("../data/owner-session-30.json")).unwrap();
    let mut q = r.clone();
    q.events.clear();
    let mut s = Session::replay(&q);
    let mut last = String::new();
    for (i, e) in r.events.iter().enumerate() {
        s.handle(e);
        let key = matches!(e.input, Input::Press { .. } | Input::Release { .. } | Input::Tool { .. } | Input::Action { .. } | Input::Round { .. });
        let sel = s.selection();
        let line = format!("tool {} notice {} hint {:?} sel e{} f{} faces {}", s.tool(), s.hint_is_notice(), s.hint(), sel.edges.len(), sel.faces.len(), s.model().faces().count());
        if key || (line != last && s.hint_is_notice()) {
            println!("{i} {:?}: {line}", std::mem::discriminant(&e.input));
            last = line;
        }
    }
}
