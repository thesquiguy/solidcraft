//! Measure Into Value (`ui.measure_into_field`): while a dialog is open, measure what is picked
//! next in the view and write the measurement into the dialog's value field, as if it were typed.
//!
//! The field is the value field that has the keyboard, or the one that had it last, or else the
//! dialog's first value field. One pick of an edge or a round face writes its size: a straight
//! edge's length, a round one's diameter (its radius in a field labelled Radius), a slot's width,
//! an arc's angle in an angle field. A second pick writes the distance or angle between the two
//! and ends measuring; centre to centre when one of them is round. Enter or a right-click in the
//! view keeps the value, Esc puts back the field's text from before.
//!
//! The written value is a number, not a link to the geometry: it does not follow the model when
//! the model changes. Measuring runs off the window thread, like the quick measurement
//! (`quick_measure`).

use serde_json::Value;
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind;

use crate::SolveApp;

/// The command behind the key and the command search.
pub(crate) const COMMAND: &str = "ui.measure_into_field";

/// Measuring into a field: the picks so far and the measurement on its way.
pub struct MeasureInto {
    /// The value field written into.
    field: egui::Id,
    /// The field is watched (`ui_kit::watch_value_field`), from the first frame on.
    watching: bool,
    picks: Vec<Sel>,
    job: Option<std::sync::mpsc::Receiver<Option<Value>>>,
}

/// Whether picks in the view are measured into a field now.
pub(crate) fn active(app: &SolveApp) -> bool {
    app.measure_into.is_some()
}

/// Start measuring into the dialog's value field. Says so in the status line, or why not.
pub(crate) fn start(app: &mut SolveApp) {
    let Some(field) = app.dialog.as_ref().and_then(crate::dialogs::Dialog::measure_field) else {
        let text = app.tr("measure_into.no_field").to_owned();
        app.set_status(text, true);
        return;
    };
    app.measure_into = Some(MeasureInto { field, watching: false, picks: Vec::new(), job: None });
    app.measure_into_restoring = false;
    let text = app.tr("measure_into.start").to_owned();
    app.set_status(text, false);
}

/// Stop measuring. `keep` false puts back the field's text from before the first write.
pub(crate) fn end(app: &mut SolveApp, ctx: &egui::Context, keep: bool) {
    if app.measure_into.take().is_none() {
        return;
    }
    if !keep {
        crate::ui_kit::restore_value_field(ctx);
        // The field takes its old text the next time it is drawn; stop watching after that.
        app.measure_into_restoring = true;
    } else {
        crate::ui_kit::unwatch_value_field(ctx);
    }
    app.status = None;
}

/// A pick in the view while measuring: the first or second item measured. A third starts over.
pub(crate) fn pick(app: &mut SolveApp, sel: Sel) {
    let Some(m) = app.measure_into.as_mut() else { return };
    if m.picks.len() >= 2 {
        m.picks.clear();
    }
    m.picks.push(sel);
    let picks = m.picks.clone();
    let job = crate::quick_measure::start(app, &picks);
    if let Some(m) = app.measure_into.as_mut() {
        m.job = job;
    }
}

/// Keys while measuring, before anything else sees them: Esc puts back the old text, Enter keeps
/// the value; neither reaches the dialog. Not measuring, the command's key with a dialog open
/// starts measuring, even while a value field has the keyboard (a typing field takes no other
/// shortcut). True when a key was taken.
pub(crate) fn keys(app: &mut SolveApp, ctx: &egui::Context) -> bool {
    if active(app) {
        for (key, keep) in [(egui::Key::Escape, false), (egui::Key::Enter, true)] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                end(app, ctx, keep);
                app.esc_handled |= key == egui::Key::Escape;
                return true;
            }
        }
        return false;
    }
    if app.dialog.is_none() {
        return false;
    }
    let bound = crate::keymap::keys(app, COMMAND);
    let pressed = ctx.input(|i| {
        i.events.iter().position(|e| {
            matches!(e, egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. }
                if bound.contains(&crate::keymap::key_name(*key, *modifiers)))
        })
    });
    let Some(at) = pressed else { return false };
    // The key and any character it typed are the command's, not the field's.
    ctx.input_mut(|i| {
        let mut index = 0;
        i.events.retain(|e| {
            let keep = index != at && !(index > at && matches!(e, egui::Event::Text(_)));
            index += 1;
            keep
        });
    });
    start(app);
    true
}

/// Each frame: write a measurement that is in, and stop when the dialog or its field is gone.
pub(crate) fn poll(app: &mut SolveApp, ctx: &egui::Context) {
    if app.measure_into_restoring {
        // The old text went back into the field when it was last drawn.
        if crate::ui_kit::watched_value_field(ctx).is_none_or(|(_, _, drawn)| drawn + 1 < ctx.cumulative_pass_nr()) {
            crate::ui_kit::unwatch_value_field(ctx);
            app.measure_into_restoring = false;
        } else {
            ctx.request_repaint();
        }
    }
    let Some(m) = app.measure_into.as_mut() else { return };
    if !m.watching {
        m.watching = true;
        crate::ui_kit::watch_value_field(ctx, m.field);
        ctx.memory_mut(|memory| memory.request_focus(m.field));
    }
    // A field not drawn for a few passes has gone with its dialog or its row.
    let field = crate::ui_kit::watched_value_field(ctx);
    if app.dialog.is_none() || field.as_ref().is_none_or(|(_, _, drawn)| drawn + 3 < ctx.cumulative_pass_nr()) {
        app.measure_into = None;
        crate::ui_kit::unwatch_value_field(ctx);
        return;
    }
    let Some(m) = app.measure_into.as_mut() else { return };
    let Some(job) = &m.job else { return };
    let result = match job.try_recv() {
        Ok(result) => result,
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
            return;
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
    };
    m.job = None;
    let picks = m.picks.len();
    let (text, label) = field.map(|(text, label, _)| (text, label)).unwrap_or_default();
    let kind = field_kind(&app.session.doc, &text);
    let value = result.as_ref().and_then(|r| measured(&app.session.doc, r, kind, &label));
    match value {
        Some(value) => {
            crate::ui_kit::fill_value_field(ctx, &value);
            ctx.request_repaint();
            if picks >= 2 {
                end(app, ctx, true);
                let text = solidcraft_ui_text::format("measure_into.done", &[("value", &value)]);
                app.set_status(text, false);
            } else {
                let text = solidcraft_ui_text::format("measure_into.filled", &[("value", &value)]);
                app.set_status(text, false);
            }
        }
        None if picks >= 2 => {
            if let Some(m) = app.measure_into.as_mut() {
                m.picks.clear();
            }
            let text = app.tr("measure_into.none").to_owned();
            app.set_status(text, true);
        }
        None => {
            let text = app.tr("measure_into.second").to_owned();
            app.set_status(text, false);
        }
    }
}

/// What a field holds, from its text: an angle when it reads as one, otherwise a length (a bare
/// number in a length field is millimetres, and most value fields are lengths).
pub(crate) fn field_kind(doc: &solidcraft_engine::doc::Document, text: &str) -> Kind {
    let text = text.trim();
    if !text.is_empty() && doc.eval(text, Kind::Unitless).is_err() && doc.eval(text, Kind::Angle).is_ok() {
        Kind::Angle
    } else {
        Kind::Length
    }
}

/// The value a measurement writes into a field of `kind` labelled `label`, as the field's text:
/// in the design's unit for a length, in degrees for an angle. `None`: nothing of that kind.
pub(crate) fn measured(doc: &solidcraft_engine::doc::Document, r: &Value, kind: Kind, label: &str) -> Option<String> {
    let items = r.get("items").and_then(Value::as_array)?;
    match kind {
        Kind::Angle => {
            let deg = match items.as_slice() {
                [one] => one.get("angle_deg").and_then(Value::as_f64),
                [_, _] => r.get("angle_deg").and_then(Value::as_f64),
                _ => None,
            }?;
            Some(degrees(deg))
        }
        Kind::Length | Kind::Unitless => {
            let mm = match items.as_slice() {
                [one] => size_of(one, label),
                [_, _] => r.get("center_distance").and_then(Value::as_f64).or_else(|| r.get("distance_mm").and_then(Value::as_f64)),
                _ => None,
            }?;
            mm.is_finite().then(|| crate::length_input::shown(doc, mm))
        }
    }
}

/// One item's size in mm: a round one's diameter (its radius for a field labelled Radius), a
/// slot's width, a straight edge's or curve's length. A point, a plane or a flat face has none.
fn size_of(item: &Value, label: &str) -> Option<f64> {
    let num = |key: &str| item.get(key).and_then(Value::as_f64);
    if item.get("diameter").is_some() {
        return if label.to_ascii_lowercase().contains("radius") { num("radius") } else { num("diameter") };
    }
    if let Some(width) = item.get("slot").and_then(|slot| slot.get("width")).and_then(Value::as_f64) {
        return Some(width);
    }
    match item.get("type").and_then(Value::as_str) {
        Some("edge" | "sketch_curve") => num("length_mm"),
        _ => None,
    }
}

/// Degrees as an angle field's text, to a ten-thousandth of a degree.
fn degrees(deg: f64) -> String {
    let s = format!("{deg:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{} deg", if s == "-0" || s.is_empty() { "0" } else { s })
}
