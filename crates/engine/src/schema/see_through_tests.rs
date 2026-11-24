use serde_json::{Value, json};

use super::{ClearSeeThroughParams, ClearSeeThroughResult, SeeThroughParams, SeeThroughResult};
use crate::Session;
use crate::schema::tests::{assert_keys, assert_strict_keeps_todays_errors, conforms};
use crate::schema::{ParamCheck, input_schema_for, output_schema_for};

#[test]
fn schemas_and_the_key_check_agree() {
    assert_keys(input_schema_for::<SeeThroughParams>(), &["bodies", "on", "paths"]);
    assert_keys(input_schema_for::<ClearSeeThroughParams>(), &[]);
}

#[test]
fn strict_keeps_todays_errors() {
    assert_strict_keeps_todays_errors(&[
        ("view.see_through", json!({"bodies": 5})),
        ("view.see_through", json!({"bodies": ["Body1"], "on": "yes"})),
        ("view.see_through", json!({"paths": []})),
    ]);
}

/// Every form of both commands, in Warn: each reply matches its output schema and carries no
/// warning; a misspelt key's warning still matches.
#[test]
fn replies_match_the_output_schema() {
    let mut s = Session { param_check: ParamCheck::Warn, ..Session::default() };
    let mut run = |id: &str, p: Value| s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"));
    let matches = |schema: schemars::Schema, reply: &Value| {
        let schema = schema.as_value();
        assert!(conforms(schema, schema, reply), "{reply} doesn't match {schema}");
    };
    run("solid.box", json!({"length": 10, "width": 10, "height": 10}));
    let calls = [
        json!({"bodies": "Body1"}),
        json!({"bodies": ["Body1"]}),
        json!({"bodies": ["Body1"], "on": true}),
        json!({"bodies": ["Body1"], "on": false}),
    ];
    for p in calls {
        let reply = run("view.see_through", p.clone());
        assert!(reply.get("param_warnings").is_none(), "{p}: {reply}");
        matches(output_schema_for::<SeeThroughResult>(), &reply);
    }
    let cleared = run("view.clear_see_through", json!({}));
    assert!(cleared.get("param_warnings").is_none(), "{cleared}");
    matches(output_schema_for::<ClearSeeThroughResult>(), &cleared);
    let warned = run("view.see_through", json!({"bodies": ["Body1"], "bodys": 1}));
    assert!(warned["param_warnings"].as_array().is_some_and(|w| !w.is_empty()), "{warned}");
    matches(output_schema_for::<SeeThroughResult>(), &warned);
}
