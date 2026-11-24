//! Contracts and result types for the commands in `cmd/see_through.rs`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::browser::PlacementPaths;
use super::types::{StringList, present};

/// Draw bodies see-through for a while, or solid again. Their colours and appearances stay as
/// they are, and clicks in the view go through them to what lies behind. View state: not saved
/// with the design and not an undo step; it lasts until the design is closed.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SeeThroughParams {
    /// The bodies, by name. Give `bodies`, `paths` or both.
    #[serde(default)]
    pub bodies: Option<StringList>,
    /// Component placements, each by the occurrence ids from the design down: every body in
    /// each placement and the placements under it.
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "PlacementPaths")]
    pub paths: Option<PlacementPaths>,
    /// True makes them see-through, false makes them solid again. Default: see-through, unless
    /// every one of them is see-through already; then solid again.
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub on: Option<bool>,
}

/// What `view.see_through` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SeeThroughResult {
    /// True when the call made the bodies see-through, false when it made them solid again.
    pub on: bool,
    /// Every body drawn see-through now, by name, in name order.
    pub see_through: Vec<String>,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// Draw every see-through body solid again. Nothing in the design changes.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ClearSeeThroughParams {}

/// What `view.clear_see_through` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ClearSeeThroughResult {
    /// How many bodies were see-through and are solid again.
    pub cleared: usize,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

#[cfg(test)]
#[path = "see_through_tests.rs"]
mod tests;
