use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn names(reply: &Value) -> Vec<&str> {
    reply["see_through"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

/// Two cubes side by side, the second painted half see-through by its appearance.
fn two_cubes() -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Left"}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [20, 0, 0], "operation": "new", "body_name": "Right"}));
    run(&mut s, "appearance.assign", json!({"bodies": ["Right"], "color": "#3366cc", "opacity": 0.5}));
    s
}

/// The key toggles: see-through, then solid again. The design, its appearances, its undo
/// history and its saved state are untouched throughout.
#[test]
fn see_through_toggles_without_changing_the_design() {
    let mut s = two_cubes();
    let doc = s.doc.clone();
    let (undo, dirty) = (s.undo.len(), s.is_dirty());
    let reply = run(&mut s, "view.see_through", json!({"bodies": ["Left"]}));
    assert_eq!(reply["on"], true);
    assert_eq!(names(&reply), ["Left"]);
    // A second body joins the first; both already see-through, the toggle makes them solid.
    let reply = run(&mut s, "view.see_through", json!({"bodies": ["Left", "Right"]}));
    assert_eq!((reply["on"].as_bool(), names(&reply)), (Some(true), vec!["Left", "Right"]));
    let reply = run(&mut s, "view.see_through", json!({"bodies": ["Left", "Right"]}));
    assert_eq!((reply["on"].as_bool(), names(&reply)), (Some(false), vec![]));
    // `on` says which way, whatever the bodies are now.
    run(&mut s, "view.see_through", json!({"bodies": "Right", "on": true}));
    let reply = run(&mut s, "view.see_through", json!({"bodies": "Right", "on": true}));
    assert_eq!(names(&reply), ["Right"], "asking twice keeps it see-through");
    assert!(std::sync::Arc::ptr_eq(&s.doc, &doc), "the design is the same design");
    assert_eq!(s.doc.model_body_opacity(s.world_state().unwrap().bodies.iter().find(|b| b.name == "Right").unwrap()), 0.5);
    assert_eq!((s.undo.len(), s.is_dirty()), (undo, dirty), "no undo step, nothing to save");
}

/// Clear See-Through makes every see-through body solid again, at once.
#[test]
fn clear_makes_every_body_solid_again() {
    let mut s = two_cubes();
    run(&mut s, "view.see_through", json!({"bodies": ["Left", "Right"]}));
    assert_eq!(run(&mut s, "view.clear_see_through", json!({}))["cleared"], 2);
    assert!(s.see_through.is_empty());
    assert_eq!(run(&mut s, "view.clear_see_through", json!({}))["cleared"], 0, "nothing left to clear");
}

/// A component placement stands for its bodies, and only its bodies.
#[test]
fn a_placement_makes_its_bodies_see_through() {
    let mut s = Session::default();
    let part = run(&mut s, "component.create", json!({"name": "Part"}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Stock"}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [20, 0, 0], "operation": "new", "body_name": "Block"}));
    let path = vec![part["occurrence"].as_u64().unwrap()];
    let inside = s.occurrence_bodies(&path, true).unwrap();
    assert!(!inside.is_empty() && !inside.iter().any(|b| b == "Block"), "{inside:?}");
    let reply = run(&mut s, "view.see_through", json!({"paths": [path]}));
    assert_eq!(names(&reply), inside.iter().map(String::as_str).collect::<Vec<_>>());
}

/// What is refused changes nothing.
#[test]
fn unknown_or_missing_bodies_are_refused() {
    let mut s = two_cubes();
    run(&mut s, "view.see_through", json!({"bodies": ["Left"]}));
    for p in [json!({}), json!({"bodies": []}), json!({"bodies": ["Left", "Nowhere"]}), json!({"paths": [[999]]})] {
        assert!(s.execute("view.see_through", &p).is_err(), "{p}");
        assert_eq!(s.see_through.iter().collect::<Vec<_>>(), ["Left"], "{p}");
    }
}

/// A body renamed or removed since leaves the list the next time it changes.
#[test]
fn bodies_no_longer_there_leave_the_list() {
    let mut s = two_cubes();
    run(&mut s, "view.see_through", json!({"bodies": ["Left", "Right"]}));
    run(&mut s, "body.rename", json!({"body": "Left", "name": "Renamed"}));
    let reply = run(&mut s, "view.see_through", json!({"bodies": ["Right"], "on": true}));
    assert_eq!(names(&reply), ["Right"]);
}
