//! Local game state, computer opponent driving, and fixed-tick simulation.

use super::*;

#[derive(Resource)]
pub(super) struct LocalGame {
    pub(super) game: TwoPlayerGame,
    pub(super) computer: Option<ComputerController>,
    pub(super) local_player: PlayerId,
    pub(super) mode: LocalGameMode,
    pub(super) network_session: Option<NetworkSession>,
    pub(super) network_lockstep: Option<NetworkLockstep>,
    pub(super) network_failed_closed: bool,
    pub(super) network_game_over_sent: bool,
    pub(super) network_result_claim_submitted: bool,
    pub(super) status_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalGameMode {
    LocalHumanVsHuman,
    ComputerOpponent,
    DirectConnect,
    HostedPlay,
}

#[derive(Resource, Debug, Clone)]
pub(super) struct RosterRecords {
    pub(super) rows: Vec<RosterRow>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RosterRow {
    pub(super) player_key: String,
    pub(super) rank: u64,
    pub(super) display_name: String,
    pub(super) wins: u64,
    pub(super) losses: u64,
    pub(super) high_score: u64,
    pub(super) high_lines: u64,
    pub(super) high_funds: u64,
    pub(super) streak: String,
    pub(super) fastest_kill_secs: Option<u64>,
    pub(super) quickest_death_secs: Option<u64>,
    pub(super) longest_game_secs: Option<u64>,
}

impl RosterRecords {
    pub(super) fn load() -> Self {
        let paths = match PersistencePaths::new() {
            Ok(paths) => paths,
            Err(error) => {
                return Self {
                    rows: Vec::new(),
                    error: Some(error.to_string()),
                };
            }
        };
        if let Some(parent) = paths.player_db_file.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return Self {
                    rows: Vec::new(),
                    error: Some(format!(
                        "record directory {} could not be created: {error}",
                        parent.display()
                    )),
                };
            }
        }
        match PlayerStore::open(&paths.player_db_file)
            .and_then(|store| store.roster_by_rank(&CommunityLabel::local()))
        {
            Ok(rows) => Self {
                rows: rows
                    .into_iter()
                    .map(|profile| RosterRow {
                        player_key: profile.player_id.as_str().to_string(),
                        rank: profile.rank,
                        display_name: profile.display_name,
                        wins: profile.wins,
                        losses: profile.losses,
                        high_score: profile.high_score,
                        high_lines: profile.high_lines,
                        high_funds: profile.high_funds,
                        streak: streak_label(profile.streak_kind, profile.streak_count),
                        fastest_kill_secs: profile.fastest_kill_secs,
                        quickest_death_secs: profile.quickest_death_secs,
                        longest_game_secs: profile.longest_game_secs,
                    })
                    .collect(),
                error: None,
            },
            Err(error) => Self {
                rows: Vec::new(),
                error: Some(format!(
                    "record store {} could not be read: {error}",
                    paths.player_db_file.display()
                )),
            },
        }
    }
}

impl LocalGame {
    pub(super) fn new_human_vs_human() -> Self {
        Self {
            game: TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2)),
            computer: None,
            local_player: PlayerId::One,
            mode: LocalGameMode::LocalHumanVsHuman,
            network_session: None,
            network_lockstep: None,
            network_failed_closed: false,
            network_game_over_sent: false,
            network_result_claim_submitted: false,
            status_message: None,
        }
    }

    pub(super) fn new_human_vs_computer(level: usize) -> Self {
        let difficulty =
            computer_difficulty(sanitize_ernie_level(level)).expect("legacy AI difficulty exists");
        Self {
            game: TwoPlayerGame::human_vs_computer(
                GameSeed::from_u64(1),
                Board::empty(),
                GameSeed::from_u64(2),
                Board::empty(),
                PlayerId::Two,
                difficulty,
            ),
            computer: Some(ComputerController::new(
                PlayerId::Two,
                GameSeed::from_u64(42),
                difficulty.level,
            )),
            local_player: PlayerId::One,
            mode: LocalGameMode::ComputerOpponent,
            network_session: None,
            network_lockstep: None,
            network_failed_closed: false,
            network_game_over_sent: false,
            network_result_claim_submitted: false,
            status_message: Some(format!("Playing {} Ernie", difficulty.name)),
        }
    }

    pub(super) fn new_networked(session: NetworkSession) -> Self {
        let (player_one_seed, player_two_seed) = derive_player_seeds(session.base_seed);
        let local_player = core_player_for_slot(session.local_slot);
        let mode = if session.hosted.is_some() {
            LocalGameMode::HostedPlay
        } else {
            LocalGameMode::DirectConnect
        };
        let status_message = Some(network_session_status_label(&session, None));

        Self {
            game: TwoPlayerGame::new(
                GameSeed::from_u64(player_one_seed),
                GameSeed::from_u64(player_two_seed),
            ),
            computer: None,
            local_player,
            mode,
            network_lockstep: Some(NetworkLockstep::new(session.local_slot)),
            network_session: Some(session),
            network_failed_closed: false,
            network_game_over_sent: false,
            network_result_claim_submitted: false,
            status_message,
        }
    }

    pub(super) fn restart(&mut self) {
        *self = match self.game.mode() {
            GameMode::HumanVsHuman => Self::new_human_vs_human(),
            GameMode::HumanVsComputer { difficulty, .. } => {
                Self::new_human_vs_computer(difficulty.level)
            }
        };
    }

    pub(super) fn is_networked(&self) -> bool {
        self.network_session.is_some() && self.network_lockstep.is_some()
    }
}

pub(super) const fn core_player_for_slot(slot: PlayerSlot) -> PlayerId {
    match slot {
        PlayerSlot::One => PlayerId::One,
        PlayerSlot::Two => PlayerId::Two,
    }
}

#[derive(Resource, Debug, Default)]
pub(super) struct ClientTickClock {
    pub(super) gameplay_elapsed_ms: u64,
    pub(super) computer_elapsed_ms: u64,
    pub(super) network_heartbeat_elapsed_ms: u64,
    pub(super) network_checksum_elapsed_ms: u64,
    pub(super) network_last_phase: Option<GamePhase>,
}

#[derive(Resource, Debug, Default)]
pub(super) struct InputRepeatState {
    pub(super) left: [HeldKeyRepeat; 2],
    pub(super) right: [HeldKeyRepeat; 2],
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct HeldKeyRepeat {
    pub(super) held_ms: u64,
    pub(super) next_repeat_ms: u64,
}

impl HeldKeyRepeat {
    pub(super) fn observe(
        self,
        pressed: bool,
        just_pressed: bool,
        elapsed_ms: u64,
    ) -> (Self, bool) {
        if !pressed {
            return (Self::default(), false);
        }
        if just_pressed {
            return (
                Self {
                    held_ms: 0,
                    next_repeat_ms: INPUT_REPEAT_INITIAL_MS,
                },
                true,
            );
        }

        let held_ms = self.held_ms.saturating_add(elapsed_ms);
        if held_ms >= self.next_repeat_ms {
            return (
                Self {
                    held_ms,
                    next_repeat_ms: self.next_repeat_ms.saturating_add(INPUT_REPEAT_MS),
                },
                true,
            );
        }

        (Self { held_ms, ..self }, false)
    }
}

#[derive(Resource, Debug, Default)]
pub(super) struct ReconPanel {
    pub(super) next_log_index: usize,
    pub(super) manual_condor: bool,
    pub(super) snapshot: Option<ReconSnapshot>,
}

#[derive(Resource, Debug)]
pub(super) struct BazaarUiState {
    pub(super) selected: WeaponToken,
    pub(super) last_message: String,
    pub(super) visual_arsenal: Option<[Option<WeaponToken>; 10]>,
}

impl Default for BazaarUiState {
    fn default() -> Self {
        Self {
            selected: WeaponToken::Gimp,
            last_message: "Select a weapon, then Add. Click staged arsenal slots to remove."
                .to_string(),
            visual_arsenal: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ComputerController {
    pub(super) player: PlayerId,
    pub(super) opponent: ComputerOpponent,
    pub(super) elapsed_ms: u64,
    pub(super) bazaar_elapsed_ms: u64,
    pub(super) planned: Vec<Command>,
    pub(super) shopped_this_bazaar: bool,
}

impl ComputerController {
    pub(super) fn new(player: PlayerId, seed: GameSeed, level: usize) -> Self {
        Self {
            player,
            opponent: ComputerOpponent::new(seed, level),
            elapsed_ms: 0,
            bazaar_elapsed_ms: 0,
            planned: Vec::new(),
            shopped_this_bazaar: false,
        }
    }

    pub(super) fn reset_for_play(&mut self) {
        self.bazaar_elapsed_ms = 0;
        self.shopped_this_bazaar = false;
    }
}

pub(super) fn drive_computer_opponent(
    time: Res<Time>,
    settings: Res<ClientSettings>,
    mut local: ResMut<LocalGame>,
    mut clock: ResMut<ClientTickClock>,
    capture: Option<Res<VisualCapture>>,
) {
    if capture.is_some() {
        return;
    }
    if settings.screen != ClientScreen::Game {
        return;
    }
    clock.computer_elapsed_ms = clock
        .computer_elapsed_ms
        .saturating_add(time.delta().as_millis().min(u128::from(u64::MAX)) as u64);
    let Some(mut computer) = local.computer.take() else {
        return;
    };

    while clock.computer_elapsed_ms >= CLIENT_FIXED_TICK_MS {
        clock.computer_elapsed_ms -= CLIENT_FIXED_TICK_MS;
        match local.game.phase() {
            GamePhase::Playing => {
                computer.reset_for_play();
                drive_computer_play(CLIENT_FIXED_TICK_MS, &mut local.game, &mut computer);
            }
            GamePhase::Bazaar => {
                drive_computer_bazaar(CLIENT_FIXED_TICK_MS, &mut local.game, &mut computer);
            }
            GamePhase::Paused | GamePhase::GameOver => {}
        }
    }

    local.computer = Some(computer);
}

pub(super) fn drive_computer_play(
    elapsed_ms: u64,
    game: &mut TwoPlayerGame,
    computer: &mut ComputerController,
) {
    if game.phase() != GamePhase::Playing {
        return;
    }

    for label in computer.opponent.launch_slots(
        game.player(computer.player).arsenal(),
        game.player(computer.player).lines(),
        game.player(opponent_player(computer.player)).lines(),
        computer_bazaar_line_value(game, computer.player),
    ) {
        let _ = game.launch_weapon_slot(computer.player, label);
    }

    computer.elapsed_ms = computer.elapsed_ms.saturating_add(elapsed_ms);
    let delay = computer.opponent.difficulty().delay_ms;
    if delay > 0 && computer.elapsed_ms < delay {
        return;
    }
    computer.elapsed_ms = 0;

    if computer.planned.is_empty() {
        computer.planned = game
            .player(computer.player)
            .active_piece()
            .and_then(|_| {
                computer
                    .opponent
                    .choose_placement(game.player(computer.player))
            })
            .map(|placement| {
                computer
                    .opponent
                    .commands_for_placement(game.player(computer.player), &placement)
            })
            .unwrap_or_default();
    }

    let commands_this_frame = if delay == 0 {
        computer.planned.len().max(1)
    } else {
        1
    };
    for _ in 0..commands_this_frame {
        let Some(command) = computer.planned.first().copied() else {
            break;
        };
        computer.planned.remove(0);
        let _ = game.command(computer.player, command);
    }
}

pub(super) fn drive_computer_bazaar(
    elapsed_ms: u64,
    game: &mut TwoPlayerGame,
    computer: &mut ComputerController,
) {
    if !computer.shopped_this_bazaar {
        let bought = game
            .bazaar_session(computer.player)
            .map(|bazaar| {
                let mut simulated = bazaar.clone();
                computer.opponent.shop(
                    &mut simulated,
                    game.player(computer.player).lines(),
                    game.player(opponent_player(computer.player)).lines(),
                    game.player(computer.player).board(),
                )
            })
            .unwrap_or_default();
        for token in bought {
            let _ = game.bazaar_buy(computer.player, token);
        }
        computer.shopped_this_bazaar = true;
    }

    computer.bazaar_elapsed_ms = computer.bazaar_elapsed_ms.saturating_add(elapsed_ms);
    if computer.bazaar_elapsed_ms >= BAZAAR_LEAVE_DELAY_MS {
        let _ = game.bazaar_done(computer.player);
    }
}

pub(super) fn tick_game(
    time: Res<Time>,
    settings: Res<ClientSettings>,
    mut local: ResMut<LocalGame>,
    mut clock: ResMut<ClientTickClock>,
    mut network_runtime: ResMut<ClientNetworkRuntime>,
    mut network_state: ResMut<ClientNetworkState>,
    capture: Option<Res<VisualCapture>>,
) {
    if capture.is_some() {
        return;
    }
    if settings.screen != ClientScreen::Game {
        return;
    }
    clock.gameplay_elapsed_ms = clock
        .gameplay_elapsed_ms
        .saturating_add(time.delta().as_millis().min(u128::from(u64::MAX)) as u64);
    if clock.gameplay_elapsed_ms < CLIENT_FIXED_TICK_MS || local.game.phase() != GamePhase::Playing
    {
        return;
    }

    if local.is_networked() {
        tick_network_game(
            &mut local,
            &mut clock,
            &mut network_runtime,
            &mut network_state,
            &settings,
        );
        return;
    }

    while clock.gameplay_elapsed_ms >= CLIENT_FIXED_TICK_MS {
        clock.gameplay_elapsed_ms -= CLIENT_FIXED_TICK_MS;
        let _ = local.game.tick_player(PlayerId::One, CLIENT_FIXED_TICK_MS);
        let _ = local.game.tick_player(PlayerId::Two, CLIENT_FIXED_TICK_MS);
        if local.game.phase() != GamePhase::Playing {
            break;
        }
    }
}

pub(super) fn tick_network_game(
    local: &mut LocalGame,
    clock: &mut ClientTickClock,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    settings: &ClientSettings,
) {
    while clock.gameplay_elapsed_ms >= CLIENT_FIXED_TICK_MS {
        if local.network_failed_closed {
            clock.gameplay_elapsed_ms = 0;
            return;
        }
        clock.gameplay_elapsed_ms -= CLIENT_FIXED_TICK_MS;
        clock.network_heartbeat_elapsed_ms = clock
            .network_heartbeat_elapsed_ms
            .saturating_add(CLIENT_FIXED_TICK_MS);
        clock.network_checksum_elapsed_ms = clock
            .network_checksum_elapsed_ms
            .saturating_add(CLIENT_FIXED_TICK_MS);

        let Some(mut lockstep) = local.network_lockstep.take() else {
            local.status_message = Some("Network lockstep state is missing.".to_string());
            return;
        };

        let watermark = lockstep.mark_input_delay_watermark();
        try_send_network_command(
            network_runtime,
            network_state,
            NetworkCommand::SendTickWatermark(watermark.clone()),
        );

        if clock.network_heartbeat_elapsed_ms >= NETWORK_HEARTBEAT_INTERVAL_MS {
            clock.network_heartbeat_elapsed_ms = 0;
            try_send_network_command(
                network_runtime,
                network_state,
                NetworkCommand::SendHeartbeat(Heartbeat {
                    player: watermark.player,
                    current_tick: lockstep.current_tick(),
                    watermark_tick: watermark.through_tick,
                }),
            );
        }

        let previous_phase = clock.network_last_phase.unwrap_or(local.game.phase());
        match lockstep.advance_ready_limited(&mut local.game, 1) {
            Ok(_) => {
                for report in lockstep.drain_pending_desync_reports(&local.game) {
                    fail_closed_on_desync(local, network_runtime, network_state, report);
                }
                if local.network_failed_closed {
                    local.network_lockstep = Some(lockstep);
                    break;
                }

                if let Some(session) = local.network_session.as_mut() {
                    session.current_tick = lockstep.current_tick();
                    session.peer_watermark = lockstep.peer_watermark();
                    local.status_message =
                        Some(network_session_status_label(session, Some(&lockstep)));
                }

                let current_phase = local.game.phase();
                if previous_phase != current_phase {
                    send_network_checksum(local, &lockstep, network_runtime, network_state);
                    clock.network_last_phase = Some(current_phase);
                }
                if clock.network_checksum_elapsed_ms >= NETWORK_CHECKSUM_INTERVAL_MS {
                    clock.network_checksum_elapsed_ms = 0;
                    send_network_checksum(local, &lockstep, network_runtime, network_state);
                }
                if current_phase == GamePhase::GameOver && !local.network_game_over_sent {
                    if let Some(game_over) = game_over_message_with_tick(local, &lockstep) {
                        local.network_game_over_sent = try_send_network_command(
                            network_runtime,
                            network_state,
                            NetworkCommand::SendGameOver(game_over),
                        );
                        send_network_checksum(local, &lockstep, network_runtime, network_state);
                    }
                }
                if current_phase == GamePhase::GameOver && !local.network_result_claim_submitted {
                    submit_hosted_ranked_result_claim(
                        local,
                        lockstep.current_tick(),
                        network_runtime,
                        network_state,
                        settings,
                    );
                }
                local.network_lockstep = Some(lockstep);
            }
            Err(error) => {
                let message = format!("Network lockstep error: {error:?}");
                local.status_message = Some(message.clone());
                network_state.last_error = Some(message.clone());
                network_state.push_message(message.clone());
                warn!("{message}");
                try_send_network_command(
                    network_runtime,
                    network_state,
                    NetworkCommand::Disconnect { reason: message },
                );
                local.network_lockstep = Some(lockstep);
                break;
            }
        }

        if local.game.phase() != GamePhase::Playing {
            break;
        }
    }
}

pub(super) fn send_network_checksum(
    local: &LocalGame,
    lockstep: &NetworkLockstep,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if local.network_failed_closed || lockstep.current_tick() == 0 {
        return;
    }
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::SendChecksum(lockstep.checksum_message(&local.game)),
    );
}

pub(super) fn submit_hosted_ranked_result_claim(
    local: &mut LocalGame,
    duration_ticks: u64,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    settings: &ClientSettings,
) {
    local.network_result_claim_submitted = true;
    let Some(session) = local.network_session.clone() else {
        return;
    };
    if !session.ranked {
        set_local_result_status(local, FinalResultStatus::Unranked);
        network_state.result_status = FinalResultStatus::Unranked;
        return;
    }
    if local.network_failed_closed {
        let status = FinalResultStatus::Rejected("desynced".to_string());
        set_local_result_status(local, status.clone());
        network_state.result_status = status;
        return;
    }
    if !settings.lobby_enabled {
        let status = FinalResultStatus::Rejected("lobby server disabled".to_string());
        set_local_result_status(local, status.clone());
        network_state.result_status = status;
        return;
    }
    let Ok(server_addr) = parse_network_addr(&settings.lobby_addr, "lobby address", network_state)
    else {
        let status = FinalResultStatus::Rejected("connection error".to_string());
        set_local_result_status(local, status.clone());
        network_state.result_status = status;
        return;
    };
    let claim = match build_ranked_result_claim(&session, &local.game, duration_ticks) {
        Ok(claim) => claim,
        Err(reason) => {
            let status = FinalResultStatus::Rejected(reason);
            set_local_result_status(local, status.clone());
            network_state.result_status = status;
            return;
        }
    };
    network_state.result_status = FinalResultStatus::None;
    local.status_message = Some("Submitting hosted ranked result claim".to_string());
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::SubmitResult { server_addr, claim },
    );
}

pub(super) fn game_over_message(local: &LocalGame) -> Option<GameOver> {
    let lockstep = local.network_lockstep.as_ref()?;
    game_over_message_with_tick(local, lockstep)
}

pub(super) fn game_over_message_with_tick(
    local: &LocalGame,
    lockstep: &NetworkLockstep,
) -> Option<GameOver> {
    local.game.event_log().iter().rev().find_map(|logged| {
        if let BattleEvent::GameOver { winner, loser } = logged.event {
            Some(GameOver {
                winner: protocol_slot_for_player(winner),
                loser: protocol_slot_for_player(loser),
                sequence: lockstep.current_tick().saturating_sub(1),
            })
        } else {
            None
        }
    })
}

pub(super) fn update_recon_panel(mut recon: ResMut<ReconPanel>, local: Res<LocalGame>) {
    for logged in &local.game.event_log()[recon.next_log_index..] {
        match &logged.event {
            BattleEvent::ReconUpdated {
                viewer,
                target,
                snapshot,
            } if *viewer == local.local_player
                && *target == opponent_player(local.local_player) =>
            {
                recon.snapshot = Some(snapshot.clone());
            }
            BattleEvent::ReconDisabled { viewer, target, .. }
                if *viewer == local.local_player
                    && *target == opponent_player(local.local_player) =>
            {
                recon.snapshot = None;
            }
            _ => {}
        }
    }
    recon.next_log_index = local.game.event_log().len();
}

pub(super) fn computer_bazaar_line_value(game: &TwoPlayerGame, computer: PlayerId) -> u32 {
    game.player(computer)
        .lines()
        .saturating_add(game.player(opponent_player(computer)).lines())
}
