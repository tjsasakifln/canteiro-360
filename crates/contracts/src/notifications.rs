use crate::reliability::ReliabilityScope;
pub use canteiro_domain::notifications::{NoticeKind, NoticePreferences, NoticeState};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct NoticeEntry {
    pub event_id: String,
    pub kind: NoticeKind,
    pub scope_id: String,
    pub due_on: Option<String>,
    pub state: NoticeState,
    pub attempts: u32,
    pub created_at: String,
    pub destination: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct NoticePreferencesView {
    pub version: Option<i64>,
    pub settings: NoticePreferences,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct NoticesContext {
    pub context: Option<ReliabilityScope>,
    pub organization_name: String,
    pub csrf: String,
    pub preferences: NoticePreferencesView,
    pub notices: Vec<NoticeEntry>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NoticePreferencesCommand {
    pub context: ReliabilityScope,
    pub expected_version: Option<i64>,
    pub settings: NoticePreferences,
}
