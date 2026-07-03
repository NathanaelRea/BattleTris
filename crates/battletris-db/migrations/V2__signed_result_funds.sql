PRAGMA foreign_keys = OFF;

CREATE TABLE game_results_signed_funds (
    result_id INTEGER PRIMARY KEY AUTOINCREMENT,
    community_label TEXT NOT NULL,
    winner_id TEXT NOT NULL,
    loser_id TEXT NOT NULL,
    winner_score INTEGER NOT NULL CHECK (winner_score >= 0),
    winner_lines INTEGER NOT NULL CHECK (winner_lines >= 0),
    winner_funds INTEGER NOT NULL,
    loser_score INTEGER NOT NULL CHECK (loser_score >= 0),
    loser_lines INTEGER NOT NULL CHECK (loser_lines >= 0),
    loser_funds INTEGER NOT NULL,
    duration_secs INTEGER NOT NULL CHECK (duration_secs >= 0),
    ranked INTEGER NOT NULL CHECK (ranked IN (0, 1)),
    recorded_at_unix_secs INTEGER NOT NULL,
    FOREIGN KEY (community_label, winner_id) REFERENCES players(community_label, player_id) ON DELETE RESTRICT,
    FOREIGN KEY (community_label, loser_id) REFERENCES players(community_label, player_id) ON DELETE RESTRICT,
    CHECK (winner_id <> loser_id)
);

INSERT INTO game_results_signed_funds (
    result_id, community_label, winner_id, loser_id, winner_score, winner_lines, winner_funds,
    loser_score, loser_lines, loser_funds, duration_secs, ranked, recorded_at_unix_secs
)
SELECT
    result_id, community_label, winner_id, loser_id, winner_score, winner_lines, winner_funds,
    loser_score, loser_lines, loser_funds, duration_secs, ranked, recorded_at_unix_secs
FROM game_results;

DROP TABLE game_results;
ALTER TABLE game_results_signed_funds RENAME TO game_results;
CREATE INDEX game_results_community_recorded_idx ON game_results (community_label, recorded_at_unix_secs DESC);

PRAGMA foreign_keys = ON;
