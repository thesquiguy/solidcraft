//! Measure Into Value (`measure_into`): a measurement picked in the view written into the open
//! dialog's value field.

use serde_json::json;
use solidcore_record::Vec3;
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind;
use solidcraft_engine::geom::prelude::*;

use crate::dialogs::Kind as DialogKind;
use crate::scenario::Harness;

/// A 10 × 20 × 30 box at the origin, in a design shown.
fn block() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.run("solid.box", json!({"length": 10, "width": 20, "height": 30})).unwrap();
    h.frames(2);
    h
}

fn edge(h: &Harness, p: Vec3) -> Sel {
    let st = crate::open_meshing::session_world(&h.app.session);
    let m = st.body("Body1").unwrap().mesh();
    let index = m.edges.iter().position(|e| e.windows(2).any(|w| p.dist_to_segment(w[0], w[1]) < 1e-9)).expect("an edge there");
    Sel::Edge { body: "Body1".into(), index, point: p }
}

/// The Fillet dialog's radius as typed.
fn radius(h: &Harness) -> String {
    match h.app.dialog.as_ref().map(|d| &d.kind) {
        Some(DialogKind::Fillet { radius, .. }) => radius.clone(),
        _ => panic!("the Fillet dialog is open"),
    }
}

/// Frames until measuring writes its value (or `want` is never reached).
fn settle(h: &mut Harness, want: &str) -> String {
    for _ in 0..300 {
        h.frames(1);
        if radius(h) == want {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    radius(h)
}

/// Fillet open with its radius field drawn, and Measure Into Value started on it.
fn measuring() -> (Harness, String) {
    let mut h = block();
    h.app.start("solid.fillet");
    h.frames(3);
    let before = radius(&h);
    h.app.start(crate::measure_into::COMMAND);
    h.frames(2);
    assert!(crate::measure_into::active(&h.app), "measuring: {:?}", h.app.status);
    (h, before)
}

#[test]
fn an_edge_picked_writes_its_length_into_the_value_and_enter_keeps_it() {
    let (mut h, _) = measuring();
    let sel = edge(&h, Vec3::new(5.0, 0.0, 0.0));
    crate::measure_into::pick(&mut h.app, sel);
    let want = crate::length_input::shown(&h.app.session.doc, 10.0);
    assert_eq!(settle(&mut h, &want), want);
    assert!(crate::measure_into::active(&h.app), "one pick waits for a possible second");
    h.step(&json!({"key": "Enter"})).unwrap();
    h.frames(2);
    assert!(!crate::measure_into::active(&h.app));
    assert!(h.app.dialog.is_some(), "Enter ends measuring, not the dialog");
    assert_eq!(radius(&h), want);
}

#[test]
fn two_picks_write_the_distance_between_them_and_end() {
    let (mut h, _) = measuring();
    let sel = edge(&h, Vec3::new(5.0, 0.0, 0.0));
    crate::measure_into::pick(&mut h.app, sel);
    h.frames(1);
    let sel = edge(&h, Vec3::new(5.0, 20.0, 0.0));
    crate::measure_into::pick(&mut h.app, sel);
    let want = crate::length_input::shown(&h.app.session.doc, 20.0);
    assert_eq!(settle(&mut h, &want), want, "{:?}", h.app.status);
    h.frames(2);
    assert!(!crate::measure_into::active(&h.app), "the second pick ends measuring");
}

#[test]
fn escape_puts_back_the_old_value_and_keeps_the_dialog() {
    let (mut h, before) = measuring();
    let sel = edge(&h, Vec3::new(0.0, 0.0, 15.0));
    crate::measure_into::pick(&mut h.app, sel);
    let want = crate::length_input::shown(&h.app.session.doc, 30.0);
    assert_eq!(settle(&mut h, &want), want);
    h.step(&json!({"key": "Escape"})).unwrap();
    assert_eq!(settle(&mut h, &before), before);
    assert!(!crate::measure_into::active(&h.app));
    assert!(h.app.dialog.is_some(), "Esc ends measuring, not the dialog");
}

#[test]
fn the_key_starts_measuring_and_without_a_dialog_says_why_not() {
    let mut h = block();
    h.step(&json!({"key": "D", "ctrl": true, "shift": true})).unwrap();
    assert!(!crate::measure_into::active(&h.app));
    assert_eq!(h.app.status.as_ref().map(|(text, _, _)| text.as_str()), Some(h.app.tr("measure_into.no_field")));
    h.app.start("solid.fillet");
    h.frames(3);
    h.step(&json!({"key": "D", "ctrl": true, "shift": true})).unwrap();
    assert!(crate::measure_into::active(&h.app), "{:?}", h.app.status);
}

#[test]
fn a_field_takes_the_size_its_label_and_unit_ask_for() {
    let mut h = Harness::new();
    h.app.run("solid.cylinder", json!({"radius": 5, "height": 10})).unwrap();
    h.frames(2);
    let doc = h.app.session.doc.clone();
    let rim = {
        let st = crate::open_meshing::session_world(&h.app.session);
        let m = st.body("Body1").unwrap().mesh();
        let rim = m.edges.iter().position(|e| e.len() > 2 && e.iter().all(|p| (p.z - 10.0).abs() < 1e-9)).expect("the top circle");
        let p = (m.edges[rim][1] + m.edges[rim][2]) * 0.5;
        Sel::Edge { body: "Body1".into(), index: rim, point: p }
    };
    let r = h.app.session.measure_items(std::slice::from_ref(&rim));
    let mm = |v: f64| crate::length_input::shown(&doc, v);
    assert_eq!(crate::measure_into::measured(&doc, &r, Kind::Length, "Radius").as_deref(), Some(mm(5.0).as_str()));
    assert_eq!(crate::measure_into::measured(&doc, &r, Kind::Length, "Diameter").as_deref(), Some(mm(10.0).as_str()));
    assert_eq!(crate::measure_into::measured(&doc, &r, Kind::Length, "Distance").as_deref(), Some(mm(10.0).as_str()));
    // A field's text says whether it holds an angle.
    assert_eq!(crate::measure_into::field_kind(&doc, "45 deg"), Kind::Angle);
    assert_eq!(crate::measure_into::field_kind(&doc, "20 mm"), Kind::Length);
    assert_eq!(crate::measure_into::field_kind(&doc, "5"), Kind::Length);
    assert_eq!(crate::measure_into::field_kind(&doc, ""), Kind::Length);
}

#[test]
fn an_angle_field_takes_the_angle_between_two_edges() {
    let h = block();
    let doc = h.app.session.doc.clone();
    let a = edge(&h, Vec3::new(5.0, 0.0, 0.0));
    let b = edge(&h, Vec3::new(0.0, 0.0, 15.0));
    let r = h.app.session.measure_items(&[a, b]);
    assert_eq!(crate::measure_into::measured(&doc, &r, Kind::Angle, "Angle").as_deref(), Some("90 deg"), "{r}");
    // A point alone has no size to write.
    let corner = Sel::Vertex { body: "Body1".into(), point: Vec3::new(0.0, 0.0, 0.0) };
    let r = h.app.session.measure_items(&[corner]);
    assert_eq!(crate::measure_into::measured(&doc, &r, Kind::Length, "Radius"), None, "{r}");
}
