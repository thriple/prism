#[test]
#[ignore]
fn probe() {
    let r: prism_tools::Recording =
        serde_json::from_str(include_str!("data/owner-session-17-shell.json")).expect("recording");
    for n in [591usize, 670] {
        let mut r = r.clone();
        r.events.truncate(n);
        eprintln!("ZZREPLAY to {n}");
        let s = prism_tools::Session::replay(&r);
        let m = s.work_model();
        let h = prism_ops::health::check(m);
        eprintln!(
            "ZZDONE {n}: version {} faces {} crossing {} leaky {} watertight {}",
            s.version(),
            m.faces().count(),
            h.crossing.faces.len(),
            h.leaky,
            h.watertight
        );
    }
}
