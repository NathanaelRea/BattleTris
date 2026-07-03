//! Player records, ranking, statistics, and persistence.
//!
//! This crate will own the modern schema for player profiles, ranked results,
//! head-to-head records, migrations, and optional legacy import/export tools. It
//! must not bind identity to Unix login names or Motif-era database files.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use rusqlite::{named_params, params, Connection, OptionalExtension, Row, Transaction};

mod embedded_migrations {
    use refinery::embed_migrations;

    embed_migrations!("migrations");
}

/// The legacy starting rank value from `BT_ELO_START`.
pub const STARTING_RANK: u64 = 1200;

const AVERAGE_GAME_VALUE: u64 = 5;
const DEFAULT_COMMUNITY_LABEL: &str = "local";

/// Result type for persistence operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Persistence errors surfaced by the database crate.
#[derive(Debug)]
pub enum Error {
    /// SQLite returned an error.
    Sqlite(rusqlite::Error),
    /// A schema migration failed.
    Migration(refinery::Error),
    /// A player id, display name, or community label failed validation.
    InvalidIdentity(String),
    /// The operating system has no supported project directory for BattleTris.
    ProjectDirsUnavailable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(err) => write!(f, "sqlite error: {err}"),
            Self::Migration(err) => write!(f, "migration error: {err}"),
            Self::InvalidIdentity(err) => write!(f, "invalid identity: {err}"),
            Self::ProjectDirsUnavailable => write!(f, "project directories are unavailable"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

impl From<refinery::Error> for Error {
    fn from(value: refinery::Error) -> Self {
        Self::Migration(value)
    }
}

/// Cross-platform persistence locations selected by ADR 0005.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistencePaths {
    /// TOML settings path.
    pub settings_file: PathBuf,
    /// SQLite database path for local player records.
    pub player_db_file: PathBuf,
    /// User-installed theme pack directory.
    pub themes_dir: PathBuf,
    /// User-installed sound pack directory.
    pub sounds_dir: PathBuf,
    /// Runtime log directory.
    pub logs_dir: PathBuf,
}

impl PersistencePaths {
    /// Builds the default platform paths for BattleTris.
    pub fn new() -> Result<Self> {
        let dirs = ProjectDirs::from("org", "BattleTris", "BattleTris")
            .ok_or(Error::ProjectDirsUnavailable)?;
        let logs_base = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());

        Ok(Self {
            settings_file: dirs.config_dir().join("settings.toml"),
            player_db_file: dirs.data_dir().join("battletris.sqlite3"),
            themes_dir: dirs.data_dir().join("themes"),
            sounds_dir: dirs.data_dir().join("sounds"),
            logs_dir: logs_base.join("logs"),
        })
    }
}

/// Stable player identity for fresh-schema records.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerId(String);

impl PlayerId {
    /// Creates a validated player id that is independent of Unix login names.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        Ok(Self(validate_label("player id", value.into(), 128)?))
    }

    /// Returns the string value stored in the database.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Ranking scope for a local database, server, or future community deployment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommunityLabel(String);

impl CommunityLabel {
    /// Creates a validated community label.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        Ok(Self(validate_label("community label", value.into(), 96)?))
    }

    /// The default V1 local/community scope.
    pub fn local() -> Self {
        Self(DEFAULT_COMMUNITY_LABEL.to_string())
    }

    /// Returns the string value stored in the database.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A player's current win/loss streak type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreakKind {
    /// No ranked result has been recorded yet.
    None,
    /// Current streak is wins.
    Wins,
    /// Current streak is losses.
    Losses,
}

impl StreakKind {
    fn from_db(value: &str) -> rusqlite::Result<Self> {
        match value {
            "none" => Ok(Self::None),
            "wins" => Ok(Self::Wins),
            "losses" => Ok(Self::Losses),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown streak kind {other}").into(),
            )),
        }
    }
}

/// A persisted player profile and ranked statistics snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerProfile {
    /// Stable player id.
    pub player_id: PlayerId,
    /// User-facing name.
    pub display_name: String,
    /// Ranking scope label.
    pub community_label: CommunityLabel,
    /// Current rank value.
    pub rank: u64,
    /// Ranked wins.
    pub wins: u64,
    /// Ranked losses.
    pub losses: u64,
    /// Best score in a ranked game.
    pub high_score: u64,
    /// Best line count in a ranked game.
    pub high_lines: u64,
    /// Best funds value in a ranked game.
    pub high_funds: u64,
    /// Current streak count.
    pub streak_count: u64,
    /// Current streak type.
    pub streak_kind: StreakKind,
    /// Shortest winning game duration, if any.
    pub fastest_kill_secs: Option<u64>,
    /// Shortest losing game duration, if any.
    pub quickest_death_secs: Option<u64>,
    /// Longest ranked game duration, if any.
    pub longest_game_secs: Option<u64>,
}

/// Head-to-head aggregate for one player against one opponent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadToHeadRecord {
    /// Player whose perspective this row represents.
    pub player_id: PlayerId,
    /// Opponent id.
    pub opponent_id: PlayerId,
    /// Ranking scope label.
    pub community_label: CommunityLabel,
    /// Wins by `player_id` against `opponent_id`.
    pub wins: u64,
    /// Losses by `player_id` against `opponent_id`.
    pub losses: u64,
}

/// Participant class for a completed game result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpponentKind {
    /// A human-vs-human result eligible for ranked writes.
    Human,
    /// A computer game, which remains unranked.
    Computer,
}

/// Completed game result payload used for ranked updates and history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameResult {
    /// Ranking scope label.
    pub community_label: CommunityLabel,
    /// Winning player id.
    pub winner_id: PlayerId,
    /// Losing player id.
    pub loser_id: PlayerId,
    /// Winner score.
    pub winner_score: u64,
    /// Winner line count.
    pub winner_lines: u64,
    /// Winner funds.
    pub winner_funds: i64,
    /// Loser score.
    pub loser_score: u64,
    /// Loser line count.
    pub loser_lines: u64,
    /// Loser funds.
    pub loser_funds: i64,
    /// Game duration in seconds.
    pub duration_secs: u64,
    /// Whether the game was intended to be ranked.
    pub ranked: bool,
    /// Opponent class for unranked computer filtering.
    pub opponent_kind: OpponentKind,
}

/// SQLite-backed player record store.
pub struct PlayerStore {
    conn: Connection,
}

impl PlayerStore {
    /// Opens a database file and runs pending migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        embedded_migrations::migrations::runner().run(&mut conn)?;
        Ok(Self { conn })
    }

    /// Opens an in-memory migrated database, primarily for tests.
    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        embedded_migrations::migrations::runner().run(&mut conn)?;
        Ok(Self { conn })
    }

    /// Creates or refreshes a player profile in the selected community.
    pub fn create_or_update_player(
        &self,
        player_id: &PlayerId,
        display_name: &str,
        community_label: &CommunityLabel,
    ) -> Result<()> {
        let display_name = validate_label("display name", display_name.to_string(), 96)?;
        let now = now_unix_secs();
        self.conn.execute(
            "INSERT INTO players (
                player_id, display_name, community_label, created_at_unix_secs, updated_at_unix_secs
              ) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(community_label, player_id) DO UPDATE SET
                display_name = excluded.display_name,
                updated_at_unix_secs = excluded.updated_at_unix_secs",
            params![
                player_id.as_str(),
                display_name,
                community_label.as_str(),
                to_db(now)
            ],
        )?;
        Ok(())
    }

    /// Returns one player profile by id.
    pub fn player(
        &self,
        player_id: &PlayerId,
        community_label: &CommunityLabel,
    ) -> Result<Option<PlayerProfile>> {
        self.conn
            .query_row(
                "SELECT player_id, display_name, community_label, rank, wins, losses,
                    high_score, high_lines, high_funds, streak_count, streak_kind,
                    fastest_kill_secs, quickest_death_secs, longest_game_secs
                 FROM players WHERE player_id = ?1 AND community_label = ?2",
                params![player_id.as_str(), community_label.as_str()],
                row_to_player,
            )
            .optional()
            .map_err(Error::from)
    }

    /// Returns roster rows sorted by rank descending, then id ascending.
    pub fn roster_by_rank(&self, community_label: &CommunityLabel) -> Result<Vec<PlayerProfile>> {
        self.roster(
            "SELECT player_id, display_name, community_label, rank, wins, losses,
                high_score, high_lines, high_funds, streak_count, streak_kind,
                fastest_kill_secs, quickest_death_secs, longest_game_secs
             FROM players WHERE community_label = ?1 ORDER BY rank DESC, player_id ASC",
            community_label,
        )
    }

    /// Returns roster rows sorted by display name, then id ascending.
    pub fn roster_by_name(&self, community_label: &CommunityLabel) -> Result<Vec<PlayerProfile>> {
        self.roster(
            "SELECT player_id, display_name, community_label, rank, wins, losses,
                high_score, high_lines, high_funds, streak_count, streak_kind,
                fastest_kill_secs, quickest_death_secs, longest_game_secs
             FROM players WHERE community_label = ?1 ORDER BY display_name COLLATE NOCASE ASC, player_id ASC",
            community_label,
        )
    }

    /// Returns a head-to-head row, if one exists.
    pub fn head_to_head(
        &self,
        player_id: &PlayerId,
        opponent_id: &PlayerId,
        community_label: &CommunityLabel,
    ) -> Result<Option<HeadToHeadRecord>> {
        self.conn
            .query_row(
                "SELECT player_id, opponent_id, community_label, wins, losses
                 FROM head_to_head_records
                 WHERE player_id = ?1 AND opponent_id = ?2 AND community_label = ?3",
                params![
                    player_id.as_str(),
                    opponent_id.as_str(),
                    community_label.as_str()
                ],
                row_to_head_to_head,
            )
            .optional()
            .map_err(Error::from)
    }

    /// Records a completed game. Computer or explicitly unranked games are kept out of stats.
    pub fn record_game_result(&mut self, result: &GameResult) -> Result<bool> {
        if !result.ranked || result.opponent_kind == OpponentKind::Computer {
            return Ok(false);
        }

        let tx = self.conn.transaction()?;
        let winner = load_player_for_update(&tx, &result.winner_id, &result.community_label)?;
        let loser = load_player_for_update(&tx, &result.loser_id, &result.community_label)?;
        let now = now_unix_secs();

        let winner_rank = recompute_rank(winner.rank, loser.rank, true);
        let loser_rank = recompute_rank(loser.rank, winner.rank, false);

        update_winner(&tx, result, &winner, winner_rank, now)?;
        update_loser(&tx, result, &loser, loser_rank, now)?;
        upsert_head_to_head(
            &tx,
            &result.winner_id,
            &result.loser_id,
            &result.community_label,
            1,
            0,
        )?;
        upsert_head_to_head(
            &tx,
            &result.loser_id,
            &result.winner_id,
            &result.community_label,
            0,
            1,
        )?;
        tx.execute(
            "INSERT INTO game_results (
                community_label, winner_id, loser_id, winner_score, winner_lines, winner_funds,
                loser_score, loser_lines, loser_funds, duration_secs, ranked, recorded_at_unix_secs
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
            params![
                result.community_label.as_str(),
                result.winner_id.as_str(),
                result.loser_id.as_str(),
                to_db(result.winner_score),
                to_db(result.winner_lines),
                result.winner_funds,
                to_db(result.loser_score),
                to_db(result.loser_lines),
                result.loser_funds,
                to_db(result.duration_secs),
                to_db(now),
            ],
        )?;
        tx.commit()?;
        Ok(true)
    }

    fn roster(&self, sql: &str, community_label: &CommunityLabel) -> Result<Vec<PlayerProfile>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![community_label.as_str()], row_to_player)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Error::from)
    }
}

/// Computes the fresh-schema rank update using the legacy rank concept.
pub fn recompute_rank(old_rank: u64, opponent_rank: u64, won: bool) -> u64 {
    let old_rank = old_rank.max(1);
    let opponent_rank = opponent_rank.max(1);
    if won {
        old_rank + (AVERAGE_GAME_VALUE * opponent_rank / old_rank).max(1)
    } else {
        let delta = AVERAGE_GAME_VALUE * old_rank / opponent_rank;
        if delta >= old_rank {
            1
        } else {
            old_rank - delta
        }
    }
}

fn update_winner(
    tx: &Transaction<'_>,
    result: &GameResult,
    winner: &PlayerProfile,
    rank: u64,
    now: u64,
) -> rusqlite::Result<()> {
    let streak_count = if winner.streak_kind == StreakKind::Wins {
        winner.streak_count + 1
    } else {
        1
    };
    tx.execute(
        "UPDATE players SET
            rank = :rank,
            wins = wins + 1,
            high_score = max(high_score, :score),
            high_lines = max(high_lines, :lines),
            high_funds = max(high_funds, :funds),
            streak_count = :streak_count,
            streak_kind = 'wins',
            fastest_kill_secs = CASE
                WHEN fastest_kill_secs IS NULL OR :duration < fastest_kill_secs THEN :duration
                ELSE fastest_kill_secs
            END,
            longest_game_secs = CASE
                WHEN longest_game_secs IS NULL OR :duration > longest_game_secs THEN :duration
                ELSE longest_game_secs
            END,
            updated_at_unix_secs = :now
         WHERE player_id = :player_id AND community_label = :community_label",
        named_params! {
            ":rank": to_db(rank),
            ":score": to_db(result.winner_score),
            ":lines": to_db(result.winner_lines),
            ":funds": result.winner_funds,
            ":duration": to_db(result.duration_secs),
            ":streak_count": to_db(streak_count),
            ":now": to_db(now),
            ":player_id": result.winner_id.as_str(),
            ":community_label": result.community_label.as_str(),
        },
    )?;
    Ok(())
}

fn update_loser(
    tx: &Transaction<'_>,
    result: &GameResult,
    loser: &PlayerProfile,
    rank: u64,
    now: u64,
) -> rusqlite::Result<()> {
    let streak_count = if loser.streak_kind == StreakKind::Losses {
        loser.streak_count + 1
    } else {
        1
    };
    tx.execute(
        "UPDATE players SET
            rank = :rank,
            losses = losses + 1,
            high_score = max(high_score, :score),
            high_lines = max(high_lines, :lines),
            high_funds = max(high_funds, :funds),
            streak_count = :streak_count,
            streak_kind = 'losses',
            quickest_death_secs = CASE
                WHEN quickest_death_secs IS NULL OR :duration < quickest_death_secs THEN :duration
                ELSE quickest_death_secs
            END,
            longest_game_secs = CASE
                WHEN longest_game_secs IS NULL OR :duration > longest_game_secs THEN :duration
                ELSE longest_game_secs
            END,
            updated_at_unix_secs = :now
         WHERE player_id = :player_id AND community_label = :community_label",
        named_params! {
            ":rank": to_db(rank),
            ":score": to_db(result.loser_score),
            ":lines": to_db(result.loser_lines),
            ":funds": result.loser_funds,
            ":duration": to_db(result.duration_secs),
            ":streak_count": to_db(streak_count),
            ":now": to_db(now),
            ":player_id": result.loser_id.as_str(),
            ":community_label": result.community_label.as_str(),
        },
    )?;
    Ok(())
}

fn upsert_head_to_head(
    tx: &Transaction<'_>,
    player_id: &PlayerId,
    opponent_id: &PlayerId,
    community_label: &CommunityLabel,
    wins: u64,
    losses: u64,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO head_to_head_records (player_id, opponent_id, community_label, wins, losses)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(community_label, player_id, opponent_id) DO UPDATE SET
            wins = wins + excluded.wins,
            losses = losses + excluded.losses",
        params![
            player_id.as_str(),
            opponent_id.as_str(),
            community_label.as_str(),
            to_db(wins),
            to_db(losses),
        ],
    )?;
    Ok(())
}

fn load_player_for_update(
    tx: &Transaction<'_>,
    player_id: &PlayerId,
    community_label: &CommunityLabel,
) -> rusqlite::Result<PlayerProfile> {
    tx.query_row(
        "SELECT player_id, display_name, community_label, rank, wins, losses,
            high_score, high_lines, high_funds, streak_count, streak_kind,
            fastest_kill_secs, quickest_death_secs, longest_game_secs
         FROM players WHERE player_id = ?1 AND community_label = ?2",
        params![player_id.as_str(), community_label.as_str()],
        row_to_player,
    )
}

fn row_to_player(row: &Row<'_>) -> rusqlite::Result<PlayerProfile> {
    let streak: String = row.get(10)?;
    Ok(PlayerProfile {
        player_id: PlayerId(row.get(0)?),
        display_name: row.get(1)?,
        community_label: CommunityLabel(row.get(2)?),
        rank: from_db(row, 3)?,
        wins: from_db(row, 4)?,
        losses: from_db(row, 5)?,
        high_score: from_db(row, 6)?,
        high_lines: from_db(row, 7)?,
        high_funds: from_db(row, 8)?,
        streak_count: from_db(row, 9)?,
        streak_kind: StreakKind::from_db(&streak)?,
        fastest_kill_secs: optional_from_db(row, 11)?,
        quickest_death_secs: optional_from_db(row, 12)?,
        longest_game_secs: optional_from_db(row, 13)?,
    })
}

fn row_to_head_to_head(row: &Row<'_>) -> rusqlite::Result<HeadToHeadRecord> {
    Ok(HeadToHeadRecord {
        player_id: PlayerId(row.get(0)?),
        opponent_id: PlayerId(row.get(1)?),
        community_label: CommunityLabel(row.get(2)?),
        wins: from_db(row, 3)?,
        losses: from_db(row, 4)?,
    })
}

fn validate_label(kind: &str, value: String, max_len: usize) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidIdentity(format!("{kind} cannot be empty")));
    }
    if trimmed.len() > max_len {
        return Err(Error::InvalidIdentity(format!(
            "{kind} exceeds {max_len} bytes"
        )));
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(Error::InvalidIdentity(format!(
            "{kind} cannot contain control characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn to_db(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

fn from_db(row: &Row<'_>, idx: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(idx)?;
    u64::try_from(value).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Integer,
            Box::new(err),
        )
    })
}

fn optional_from_db(row: &Row<'_>, idx: usize) -> rusqlite::Result<Option<u64>> {
    let value: Option<i64> = row.get(idx)?;
    value
        .map(|inner| {
            u64::try_from(inner).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    idx,
                    rusqlite::types::Type::Integer,
                    Box::new(err),
                )
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_create_empty_roster() {
        let store = PlayerStore::open_in_memory().unwrap();
        assert!(store
            .roster_by_rank(&CommunityLabel::local())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn ranked_human_result_updates_records_and_head_to_head() {
        let mut store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::local();
        let alice = PlayerId::new("alice-local-id").unwrap();
        let bob = PlayerId::new("bob-local-id").unwrap();
        store
            .create_or_update_player(&alice, "Alice", &community)
            .unwrap();
        store
            .create_or_update_player(&bob, "Bob", &community)
            .unwrap();

        let result = sample_result(&community, &alice, &bob, OpponentKind::Human, true);
        assert!(store.record_game_result(&result).unwrap());

        let alice_profile = store.player(&alice, &community).unwrap().unwrap();
        let bob_profile = store.player(&bob, &community).unwrap().unwrap();
        assert_eq!(alice_profile.rank, 1205);
        assert_eq!(bob_profile.rank, 1195);
        assert_eq!(alice_profile.wins, 1);
        assert_eq!(alice_profile.losses, 0);
        assert_eq!(alice_profile.high_score, 12_000);
        assert_eq!(alice_profile.high_lines, 40);
        assert_eq!(alice_profile.high_funds, 900);
        assert_eq!(alice_profile.streak_kind, StreakKind::Wins);
        assert_eq!(alice_profile.streak_count, 1);
        assert_eq!(alice_profile.fastest_kill_secs, Some(180));
        assert_eq!(alice_profile.longest_game_secs, Some(180));
        assert_eq!(bob_profile.wins, 0);
        assert_eq!(bob_profile.losses, 1);
        assert_eq!(bob_profile.quickest_death_secs, Some(180));

        let alice_vs_bob = store
            .head_to_head(&alice, &bob, &community)
            .unwrap()
            .unwrap();
        let bob_vs_alice = store
            .head_to_head(&bob, &alice, &community)
            .unwrap()
            .unwrap();
        assert_eq!((alice_vs_bob.wins, alice_vs_bob.losses), (1, 0));
        assert_eq!((bob_vs_alice.wins, bob_vs_alice.losses), (0, 1));
    }

    #[test]
    fn computer_and_unranked_games_do_not_update_records() {
        let mut store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::local();
        let human = PlayerId::new("human").unwrap();
        let computer = PlayerId::new("computer").unwrap();
        store
            .create_or_update_player(&human, "Human", &community)
            .unwrap();
        store
            .create_or_update_player(&computer, "Computer", &community)
            .unwrap();

        let computer_result =
            sample_result(&community, &human, &computer, OpponentKind::Computer, true);
        assert!(!store.record_game_result(&computer_result).unwrap());
        let unranked_result =
            sample_result(&community, &human, &computer, OpponentKind::Human, false);
        assert!(!store.record_game_result(&unranked_result).unwrap());

        let human_profile = store.player(&human, &community).unwrap().unwrap();
        assert_eq!(human_profile.rank, STARTING_RANK);
        assert_eq!(human_profile.wins, 0);
        assert_eq!(human_profile.losses, 0);
        assert!(store
            .head_to_head(&human, &computer, &community)
            .unwrap()
            .is_none());
    }

    #[test]
    fn ranked_results_store_negative_final_funds() {
        let mut store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::local();
        let alice = PlayerId::new("alice").unwrap();
        let bob = PlayerId::new("bob").unwrap();
        store
            .create_or_update_player(&alice, "Alice", &community)
            .unwrap();
        store
            .create_or_update_player(&bob, "Bob", &community)
            .unwrap();

        let mut result = sample_result(&community, &alice, &bob, OpponentKind::Human, true);
        result.winner_funds = -125;
        result.loser_funds = -500;

        assert!(store.record_game_result(&result).unwrap());
    }

    #[test]
    fn roster_views_sort_by_rank_or_display_name() {
        let mut store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::local();
        let alice = PlayerId::new("alice").unwrap();
        let bob = PlayerId::new("bob").unwrap();
        let carol = PlayerId::new("carol").unwrap();
        store
            .create_or_update_player(&alice, "Zephyr", &community)
            .unwrap();
        store
            .create_or_update_player(&bob, "alpha", &community)
            .unwrap();
        store
            .create_or_update_player(&carol, "Carol", &community)
            .unwrap();
        store
            .record_game_result(&sample_result(
                &community,
                &carol,
                &alice,
                OpponentKind::Human,
                true,
            ))
            .unwrap();

        let by_rank = store.roster_by_rank(&community).unwrap();
        assert_eq!(by_rank[0].player_id, carol);
        let by_name = store.roster_by_name(&community).unwrap();
        assert_eq!(by_name[0].player_id, bob);
        assert_eq!(by_name[1].player_id, carol);
        assert_eq!(by_name[2].player_id, alice);
    }

    #[test]
    fn same_player_id_has_independent_records_per_community() {
        let mut store = PlayerStore::open_in_memory().unwrap();
        let local = CommunityLabel::local();
        let garage = CommunityLabel::new("garage").unwrap();
        let ada = PlayerId::new("ada").unwrap();
        let ben = PlayerId::new("ben").unwrap();
        store
            .create_or_update_player(&ada, "Ada Local", &local)
            .unwrap();
        store
            .create_or_update_player(&ben, "Ben Local", &local)
            .unwrap();
        store
            .create_or_update_player(&ada, "Ada Garage", &garage)
            .unwrap();
        store
            .create_or_update_player(&ben, "Ben Garage", &garage)
            .unwrap();

        store
            .record_game_result(&sample_result(
                &garage,
                &ada,
                &ben,
                OpponentKind::Human,
                true,
            ))
            .unwrap();

        let local_ada = store.player(&ada, &local).unwrap().unwrap();
        let garage_ada = store.player(&ada, &garage).unwrap().unwrap();
        assert_eq!(local_ada.display_name, "Ada Local");
        assert_eq!(local_ada.rank, STARTING_RANK);
        assert_eq!(local_ada.wins, 0);
        assert_eq!(garage_ada.display_name, "Ada Garage");
        assert_eq!(garage_ada.rank, STARTING_RANK + 5);
        assert_eq!(garage_ada.wins, 1);
    }

    #[test]
    fn identity_rejects_empty_or_control_values() {
        assert!(PlayerId::new(" ").is_err());
        assert!(PlayerId::new("bad\nvalue").is_err());
        assert!(CommunityLabel::new("main-server").is_ok());
    }

    #[test]
    fn rank_formula_preserves_legacy_integer_shape_without_named_stat_caps() {
        assert_eq!(recompute_rank(STARTING_RANK, STARTING_RANK, true), 1205);
        assert_eq!(recompute_rank(STARTING_RANK, STARTING_RANK, false), 1195);
        assert_eq!(recompute_rank(10, 1, false), 1);
        assert_eq!(recompute_rank(10_000, 1, true), 10_001);
    }

    fn sample_result(
        community: &CommunityLabel,
        winner: &PlayerId,
        loser: &PlayerId,
        opponent_kind: OpponentKind,
        ranked: bool,
    ) -> GameResult {
        GameResult {
            community_label: community.clone(),
            winner_id: winner.clone(),
            loser_id: loser.clone(),
            winner_score: 12_000,
            winner_lines: 40,
            winner_funds: 900,
            loser_score: 8_000,
            loser_lines: 24,
            loser_funds: 500,
            duration_secs: 180,
            ranked,
            opponent_kind,
        }
    }
}
