//! Response models for the monkeytype API. Every endpoint wraps its payload in
//! `{message, data}`. Models tolerate unknown fields (serde default) so server
//! additions never break the client.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Envelope<T> {
    #[serde(default)]
    pub message: String,
    pub data: Option<T>,
}

/// `POST /results` response payload (packages/contracts results.addResult).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PostResultData {
    pub is_pb: bool,
    pub xp: i64,
    pub daily_xp_bonus: bool,
    pub streak: i64,
    pub inserted_id: String,
    /// Present only when the result made a leaderboard.
    pub daily_leaderboard_rank: Option<i64>,
    pub weekly_xp_leaderboard_rank: Option<i64>,
    pub xp_breakdown: serde_json::Value,
    pub tag_pbs: Vec<String>,
}
