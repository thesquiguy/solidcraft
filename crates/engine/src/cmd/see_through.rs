//! See-Through: bodies drawn see-through for a while, to look and click past them, without
//! changing their colours or appearances (#16384). View state: not saved, not an undo step.
use serde_json::Value;

use super::CommandSpec;
use crate::params::bad;
use crate::schema::see_through::{ClearSeeThroughParams, ClearSeeThroughResult, SeeThroughParams, SeeThroughResult};
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session};

const SEE_THROUGH_CONTRACT: TypedContract<SeeThroughParams> = TypedContract::new();
const CLEAR_SEE_THROUGH_CONTRACT: TypedContract<ClearSeeThroughParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("view.see_through", "See-Through", crate::checked!("view.see_through", see_through))
        .icon("eye")
        .key("Shift+T")
        .noundo()
        .contract(&SEE_THROUGH_CONTRACT)
        .output(output_schema_for::<SeeThroughResult>)
        .params(
            "bodies?: [body name]; paths?: [[occurrence ids]] (placements, with the placements under them); on?: bool (omitted: \
             see-through, or solid again when all of them are see-through already) → {on, see_through: [every see-through body]}. \
             The bodies keep their colours and appearances; clicks go through them. View state: not saved, not an undo step, gone \
             when the design is closed. In the app, its key acts on the selection",
        ),
    CommandSpec::new("view.clear_see_through", "Clear See-Through", crate::checked!("view.clear_see_through", clear_see_through))
        .icon("eye")
        .key("Alt+Shift+T")
        .noundo()
        .doc_only()
        .contract(&CLEAR_SEE_THROUGH_CONTRACT)
        .output(output_schema_for::<ClearSeeThroughResult>)
        .params("(no parameters) → {cleared}; every see-through body is drawn solid again. Nothing in the design changes"),
];

/// `view.see_through`: the bodies named, and those of the placements given, see-through or
/// solid again.
fn see_through(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "view.see_through";
    let request: SeeThroughParams = crate::schema::read(cmd, p)?;
    let mut bodies = request.bodies.map(|b| b.0).unwrap_or_default();
    for path in request.paths.map(|p| p.0).unwrap_or_default() {
        s.occurrence_placement(&path).map_err(|e| bad(cmd, e.to_string()))?;
        bodies.extend(s.occurrence_bodies(&path, true).map_err(|e| bad(cmd, e.to_string()))?);
    }
    if bodies.is_empty() {
        return Err(bad(cmd, "give `bodies` or `paths`: the bodies to draw see-through"));
    }
    let world = s.world_state()?;
    let known: std::collections::BTreeSet<&str> = world.bodies.iter().map(|b| b.name.as_str()).collect();
    if let Some(missing) = bodies.iter().find(|b| !known.contains(b.as_str())) {
        return Err(bad(cmd, format!("no body named `{missing}`")));
    }
    // Bodies a later edit removed or renamed leave the list, so it names only bodies there are.
    s.see_through.retain(|b| known.contains(b.as_str()));
    let on = request.on.unwrap_or_else(|| !bodies.iter().all(|b| s.see_through.contains(b)));
    if on {
        s.see_through.extend(bodies);
    } else {
        for b in &bodies {
            s.see_through.remove(b);
        }
    }
    s.refresh_display();
    reply(cmd, SeeThroughResult { on, see_through: s.see_through.iter().cloned().collect(), param_warnings: Vec::new() })
}

/// `view.clear_see_through`: every see-through body solid again.
fn clear_see_through(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "view.clear_see_through";
    let _: ClearSeeThroughParams = crate::schema::read(cmd, p)?;
    let cleared = std::mem::take(&mut s.see_through).len();
    if cleared > 0 {
        s.refresh_display();
    }
    reply(cmd, ClearSeeThroughResult { cleared, param_warnings: Vec::new() })
}

/// A reply built from its schema's type, so the two can't drift apart.
fn reply(cmd: &str, r: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(r).map_err(|e| bad(cmd, e.to_string()))
}

#[cfg(test)]
#[path = "see_through_tests.rs"]
mod tests;
