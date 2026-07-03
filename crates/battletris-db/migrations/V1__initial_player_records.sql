CREATE TABLE players (
    player_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    community_label TEXT NOT NULL,
    rank INTEGER NOT NULL DEFAULT 1200 CHECK (rank >= 1),
    wins INTEGER NOT NULL DEFAULT 0 CHECK (wins >= 0),
    losses INTEGER NOT NULL DEFAULT 0 CHECK (losses >= 0),
    high_score INTEGER NOT NULL DEFAULT 0 CHECK (high_score >= 0),
    high_lines INTEGER NOT NULL DEFAULT 0 CHECK (high_lines >= 0),
    high_funds INTEGER NOT NULL DEFAULT 0 CHECK (high_funds >= 0),
    streak_count INTEGER NOT NULL DEFAULT 0 CHECK (streak_count >= 0),
    streak_kind TEXT NOT NULL DEFAULT 'none' CHECK (streak_kind IN ('none', 'wins', 'losses')),
    fastest_kill_secs INTEGER CHECK (fastest_kill_secs IS NULL OR fastest_kill_secs >= 0),
    quickest_death_secs INTEGER CHECK (quickest_death_secs IS NULL OR quickest_death_secs >= 0),
    longest_game_secs INTEGER CHECK (longest_game_secs IS NULL OR longest_game_secs >= 0),
    created_at_unix_secs INTEGER NOT NULL,
    updated_at_unix_secs INTEGER NOT NULL,
    PRIMARY KEY (community_label, player_id)
);

CREATE INDEX players_community_rank_idx ON players (community_label, rank DESC, player_id ASC);
CREATE INDEX players_community_name_idx ON players (community_label, display_name COLLATE NOCASE ASC, player_id ASC);

CREATE TABLE head_to_head_records (
    player_id TEXT NOT NULL,
    opponent_id TEXT NOT NULL,
    community_label TEXT NOT NULL,
    wins INTEGER NOT NULL DEFAULT 0 CHECK (wins >= 0),
    losses INTEGER NOT NULL DEFAULT 0 CHECK (losses >= 0),
    PRIMARY KEY (community_label, player_id, opponent_id),
    FOREIGN KEY (community_label, player_id) REFERENCES players(community_label, player_id) ON DELETE CASCADE,
    FOREIGN KEY (community_label, opponent_id) REFERENCES players(community_label, player_id) ON DELETE CASCADE,
    CHECK (player_id <> opponent_id)
);

CREATE TABLE game_results (
    result_id INTEGER PRIMARY KEY AUTOINCREMENT,
    community_label TEXT NOT NULL,
    winner_id TEXT NOT NULL,
    loser_id TEXT NOT NULL,
    winner_score INTEGER NOT NULL CHECK (winner_score >= 0),
    winner_lines INTEGER NOT NULL CHECK (winner_lines >= 0),
    winner_funds INTEGER NOT NULL CHECK (winner_funds >= 0),
    loser_score INTEGER NOT NULL CHECK (loser_score >= 0),
    loser_lines INTEGER NOT NULL CHECK (loser_lines >= 0),
    loser_funds INTEGER NOT NULL CHECK (loser_funds >= 0),
    duration_secs INTEGER NOT NULL CHECK (duration_secs >= 0),
    ranked INTEGER NOT NULL CHECK (ranked IN (0, 1)),
    recorded_at_unix_secs INTEGER NOT NULL,
    FOREIGN KEY (community_label, winner_id) REFERENCES players(community_label, player_id) ON DELETE RESTRICT,
    FOREIGN KEY (community_label, loser_id) REFERENCES players(community_label, player_id) ON DELETE RESTRICT,
    CHECK (winner_id <> loser_id)
);

CREATE INDEX game_results_community_recorded_idx ON game_results (community_label, recorded_at_unix_secs DESC);
