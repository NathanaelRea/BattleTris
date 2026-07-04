//! Bevy-facing networking adapter state and systems.

use super::*;

#[derive(Resource, Debug)]
pub(super) struct ClientNetworkRuntime {
    pub(super) runtime: NetworkRuntime,
}

impl Default for ClientNetworkRuntime {
    fn default() -> Self {
        Self {
            runtime: NetworkRuntime::start(),
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub(super) struct ClientNetworkState {
    pub(super) lifecycle: NetworkLifecycleState,
    pub(super) last_error: Option<String>,
    pub(super) listening_bind_addr: Option<SocketAddr>,
    pub(super) listening_share_addr: Option<SocketAddr>,
    pub(super) pending_challenge: Option<Challenge>,
    pub(super) lobby_list: Option<LobbyList>,
    pub(super) lobby_selected_index: usize,
    pub(super) lobby_registration: Option<LobbyEntry>,
    pub(super) lobby_server_addr: Option<SocketAddr>,
    pub(super) hosted_status: Option<HostedSessionStatus>,
    pub(super) hosted_start: Option<HostedGameStart>,
    pub(super) connected_session: Option<NetworkSession>,
    pub(super) result_status: FinalResultStatus,
    pub(super) ranked_records: Option<RankedRecords>,
    pub(super) lan_entries: Vec<LanDiscoveryEntry>,
    pub(super) lan_selected_index: usize,
    pub(super) lan_advertising: bool,
    pub(super) last_input: Option<PlayerInput>,
    pub(super) last_tick_watermark: Option<TickWatermark>,
    pub(super) last_heartbeat: Option<Heartbeat>,
    pub(super) last_checksum: Option<GameChecksum>,
    pub(super) last_game_over: Option<GameOver>,
    pub(super) last_bazaar_buy: Option<BazaarBuy>,
    pub(super) last_bazaar_remove: Option<BazaarRemove>,
    pub(super) last_bazaar_done_player: Option<battletris_protocol::PlayerSlot>,
    pub(super) transient_messages: Vec<String>,
    pub(super) hosted_poll_elapsed_ms: u64,
    pub(super) lobby_browse_elapsed_ms: u64,
    pub(super) sleep_availability_attempted: bool,
}

impl Default for ClientNetworkState {
    fn default() -> Self {
        Self {
            lifecycle: NetworkLifecycleState::Idle,
            last_error: None,
            listening_bind_addr: None,
            listening_share_addr: None,
            pending_challenge: None,
            lobby_list: None,
            lobby_selected_index: 0,
            lobby_registration: None,
            lobby_server_addr: None,
            hosted_status: None,
            hosted_start: None,
            connected_session: None,
            result_status: FinalResultStatus::None,
            ranked_records: None,
            lan_entries: Vec::new(),
            lan_selected_index: 0,
            lan_advertising: false,
            last_input: None,
            last_tick_watermark: None,
            last_heartbeat: None,
            last_checksum: None,
            last_game_over: None,
            last_bazaar_buy: None,
            last_bazaar_remove: None,
            last_bazaar_done_player: None,
            transient_messages: Vec::new(),
            hosted_poll_elapsed_ms: 0,
            lobby_browse_elapsed_ms: 0,
            sleep_availability_attempted: false,
        }
    }
}

impl ClientNetworkState {
    pub(super) fn push_message(&mut self, message: impl Into<String>) {
        const MAX_TRANSIENT_MESSAGES: usize = 8;
        self.transient_messages.push(message.into());
        if self.transient_messages.len() > MAX_TRANSIENT_MESSAGES {
            let overflow = self.transient_messages.len() - MAX_TRANSIENT_MESSAGES;
            self.transient_messages.drain(0..overflow);
        }
    }
}

#[derive(SystemParam)]
pub(super) struct NetworkPumpParams<'w> {
    pub(super) runtime: ResMut<'w, ClientNetworkRuntime>,
    pub(super) network_state: ResMut<'w, ClientNetworkState>,
    pub(super) local: ResMut<'w, LocalGame>,
    pub(super) settings: ResMut<'w, ClientSettings>,
    pub(super) clock: ResMut<'w, ClientTickClock>,
    pub(super) repeat: ResMut<'w, InputRepeatState>,
    pub(super) recon: ResMut<'w, ReconPanel>,
    pub(super) bazaar_ui: ResMut<'w, BazaarUiState>,
    pub(super) sound: ResMut<'w, SoundEventState>,
}

pub(super) fn pump_network_events(mut input: NetworkPumpParams) {
    loop {
        match input.runtime.runtime.channels_mut().try_recv_event() {
            Ok(event) => {
                let hosted_start = match &event {
                    NetworkEvent::HostedGameStarted(start) => Some(start.clone()),
                    _ => None,
                };
                let listening = matches!(&event, NetworkEvent::Listening { .. });
                apply_network_game_event(
                    &mut input.local,
                    &mut input.runtime,
                    &mut input.network_state,
                    &event,
                );
                let network_game_ended = matches!(
                    &event,
                    NetworkEvent::StateChanged(NetworkLifecycleState::Idle)
                        | NetworkEvent::Error { .. }
                ) && input.local.is_networked();
                if let Some(session) = reduce_client_network_event(&mut input.network_state, event)
                {
                    *input.local = LocalGame::new_networked(session);
                    input.settings.screen = ClientScreen::Game;
                    *input.clock = ClientTickClock::default();
                    *input.repeat = InputRepeatState::default();
                    *input.recon = ReconPanel::default();
                    *input.bazaar_ui = BazaarUiState::default();
                    input.sound.next_log_index = 0;
                    input.sound.last_event = None;
                    input.sound.pending_events.clear();
                } else if network_game_ended {
                    input.local.network_session = None;
                    input.local.network_lockstep = None;
                    input.local.network_failed_closed = true;
                    input.settings.screen = ClientScreen::Challenge;
                }
                if let Some(start) = hosted_start {
                    join_hosted_direct_after_start(
                        &input.settings,
                        &mut input.runtime,
                        &mut input.network_state,
                        start,
                    );
                }
                if listening
                    && ((input.settings.challenge_style == ChallengeStyle::Modern
                        && (input.settings.challenge_mode == ChallengeMode::HostViaLobby
                            || input.settings.challenge_mode == ChallengeMode::HostDirect))
                        || input.settings.screen == ClientScreen::Sleep)
                {
                    start_lan_advertising(
                        &input.settings,
                        &mut input.runtime,
                        &mut input.network_state,
                    );
                }
                if listening
                    && ((input.settings.challenge_style == ChallengeStyle::Modern
                        && input.settings.challenge_mode == ChallengeMode::HostViaLobby)
                        || input.settings.screen == ClientScreen::Sleep)
                {
                    register_hosted_lobby(
                        &input.settings,
                        &mut input.runtime,
                        &mut input.network_state,
                    );
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => break,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                let message = "network runtime event channel closed".to_string();
                input.network_state.lifecycle = NetworkLifecycleState::Error {
                    message: message.clone(),
                };
                input.network_state.last_error = Some(message.clone());
                input.network_state.push_message(message.clone());
                error!("{message}");
                break;
            }
        }
    }
}

pub(super) fn apply_network_game_event(
    local: &mut LocalGame,
    runtime: &mut ClientNetworkRuntime,
    state: &mut ClientNetworkState,
    event: &NetworkEvent,
) {
    match event {
        NetworkEvent::PendingResult(pending) => {
            set_local_result_status(local, FinalResultStatus::Pending(pending.clone()));
        }
        NetworkEvent::RecordedResult(_) => {
            set_local_result_status(local, FinalResultStatus::Recorded);
        }
        NetworkEvent::RejectedResult(rejected) => {
            set_local_result_status(local, FinalResultStatus::Rejected(rejected.reason.clone()));
        }
        _ => {}
    }

    let Some(lockstep) = local.network_lockstep.as_mut() else {
        return;
    };
    let result = match event {
        NetworkEvent::InputReceived(input) => {
            lockstep.receive_remote_input(&mut local.game, input.clone())
        }
        NetworkEvent::TickWatermark(watermark) => {
            lockstep.receive_peer_watermark(watermark.clone())
        }
        NetworkEvent::Heartbeat(heartbeat) => lockstep.receive_peer_watermark(TickWatermark {
            player: heartbeat.player,
            through_tick: heartbeat.watermark_tick,
        }),
        NetworkEvent::BazaarBuy(buy) => lockstep.apply_bazaar_buy(&mut local.game, buy.clone()),
        NetworkEvent::BazaarRemove(remove) => {
            lockstep.apply_bazaar_remove(&mut local.game, remove.clone())
        }
        NetworkEvent::BazaarDone { player } => {
            lockstep.apply_bazaar_done(&mut local.game, BazaarDone { player: *player });
            Ok(())
        }
        NetworkEvent::Checksum(checksum) => {
            if let Some(report) = lockstep.receive_checksum(&local.game, checksum.clone()) {
                fail_closed_on_desync(local, runtime, state, report);
            }
            return;
        }
        NetworkEvent::GameOver(game_over) => {
            verify_peer_game_over(local, runtime, state, game_over);
            return;
        }
        _ => return,
    };

    if let Err(error) = result {
        let message = format!("Network protocol error: {error:?}");
        local.status_message = Some(message.clone());
        state.last_error = Some(message.clone());
        state.push_message(message.clone());
        warn!("{message}");
        try_send_network_command(
            runtime,
            state,
            NetworkCommand::Disconnect { reason: message },
        );
    }
}

pub(super) fn set_local_result_status(local: &mut LocalGame, status: FinalResultStatus) {
    if let Some(session) = local.network_session.as_mut() {
        session.final_result_status = status.clone();
        local.status_message = Some(network_session_status_label(
            session,
            local.network_lockstep.as_ref(),
        ));
    }
}

pub(super) fn fail_closed_on_desync(
    local: &mut LocalGame,
    runtime: &mut ClientNetworkRuntime,
    state: &mut ClientNetworkState,
    report: battletris_client::net::DesyncReport,
) {
    let message = format!(
        "Desync detected. The online game stopped to avoid showing different results. Tick {}, local checksum {}, peer checksum {}",
        report.tick, report.local_checksum, report.remote_checksum
    );
    local.network_failed_closed = true;
    local.status_message = Some(message.clone());
    if let Some(session) = local.network_session.as_mut() {
        session.ranked = false;
        session.final_result_status = FinalResultStatus::Rejected("desynced".to_string());
    }
    state.last_error = Some(message.clone());
    state.result_status = FinalResultStatus::Rejected("desynced".to_string());
    state.push_message(message.clone());
    error!(
        "network desync tick={} local_checksum={} remote_checksum={} local_events={} remote_events={}",
        report.tick,
        report.local_checksum,
        report.remote_checksum,
        report.local_event_count,
        report.remote_event_count
    );
    try_send_network_command(
        runtime,
        state,
        NetworkCommand::Disconnect { reason: message },
    );
}

pub(super) fn verify_peer_game_over(
    local: &mut LocalGame,
    runtime: &mut ClientNetworkRuntime,
    state: &mut ClientNetworkState,
    peer: &GameOver,
) {
    let Some(local_game_over) = game_over_message(local) else {
        let message = format!(
            "Desync detected: peer game over at tick {} before local game over",
            peer.sequence
        );
        local.status_message = Some(message.clone());
        state.last_error = Some(message.clone());
        state.push_message(message.clone());
        try_send_network_command(
            runtime,
            state,
            NetworkCommand::Disconnect { reason: message },
        );
        local.network_failed_closed = true;
        return;
    };

    if local_game_over != *peer {
        let message = format!(
            "Desync detected: peer game over {:?} conflicts with local {:?}",
            peer, local_game_over
        );
        local.status_message = Some(message.clone());
        state.last_error = Some(message.clone());
        state.push_message(message.clone());
        try_send_network_command(
            runtime,
            state,
            NetworkCommand::Disconnect { reason: message },
        );
        local.network_failed_closed = true;
    }
}

pub(super) fn reduce_client_network_event(
    state: &mut ClientNetworkState,
    event: NetworkEvent,
) -> Option<NetworkSession> {
    let mut connected = None;
    match event {
        NetworkEvent::Listening {
            bind_addr,
            share_addr,
        } => {
            state.lifecycle = NetworkLifecycleState::Hosting { bind_addr };
            state.listening_bind_addr = Some(bind_addr);
            state.listening_share_addr = Some(share_addr);
            state.pending_challenge = None;
            state.push_message(format!("Listening on {bind_addr}; share {share_addr}"));
            info!("network listening bind={bind_addr} share={share_addr}");
        }
        NetworkEvent::LanAdvertisingStarted { advertisement } => {
            state.lan_advertising = true;
            state.push_message(format!(
                "LAN advertising started on port {}",
                advertisement.port
            ));
            info!(
                "network LAN advertising started service={} port={} txt={:?}",
                advertisement.service, advertisement.port, advertisement.txt
            );
        }
        NetworkEvent::LanAdvertisingStopped => {
            state.lan_advertising = false;
            state.push_message("LAN advertising stopped");
            info!("network LAN advertising stopped");
        }
        NetworkEvent::LanDiscoveryEntries { entries } => {
            let count = entries.len();
            state.lan_entries = entries;
            state.lan_selected_index = state.lan_selected_index.min(count.saturating_sub(1));
            state.push_message(format!("LAN browse returned {count} entries"));
            info!("network LAN browse entries={count}");
        }
        NetworkEvent::LanDiscoveryUnavailable { reason } => {
            state.lan_advertising = false;
            state.last_error = Some(reason.clone());
            state.push_message(format!("LAN discovery unavailable: {reason}"));
            warn!("network LAN discovery unavailable reason={reason}");
        }
        NetworkEvent::IncomingChallenge { challenge } => {
            state.lifecycle = NetworkLifecycleState::Challenged {
                challenge: challenge.clone(),
            };
            state.pending_challenge = Some(challenge.clone());
            state.push_message(format!(
                "Incoming challenge from {}",
                challenge.challenger.display_name
            ));
            info!(
                "network incoming challenge display_name={} hosted_session={:?}",
                challenge.challenger.display_name, challenge.hosted_session_id
            );
        }
        NetworkEvent::Connected { session } => {
            let session = *session;
            log_network_session(&session);
            state.lifecycle = NetworkLifecycleState::Connected {
                session: Box::new(session.clone()),
            };
            state.connected_session = Some(session.clone());
            state.result_status = session.final_result_status.clone();
            state.pending_challenge = None;
            state.push_message(format!(
                "Connected to {} as {:?}",
                session.peer_identity.display_name, session.local_slot
            ));
            connected = Some(session);
        }
        NetworkEvent::LobbyRegistered(entry) => {
            state.lobby_registration = Some(entry.clone());
            state.hosted_poll_elapsed_ms = 0;
            state.push_message(format!("Lobby registered as {}", entry.host.display_name));
            info!(
                "network lobby registered session={:?} display_name={} share={} ranked={}",
                entry.session_id, entry.host.display_name, entry.direct_addr, entry.ranked
            );
        }
        NetworkEvent::LobbyList(list) => {
            let count = list.entries.len();
            state.lobby_selected_index = state.lobby_selected_index.min(count.saturating_sub(1));
            state.lobby_list = Some(list);
            state.lobby_browse_elapsed_ms = 0;
            state.push_message(format!("Lobby returned {count} players"));
            info!("network lobby list entries={count}");
        }
        NetworkEvent::HostedGameStarted(start) => {
            state.hosted_start = Some(start.clone());
            state.push_message(format!("Hosted game started: {:?}", start.session_id));
            info!(
                "network hosted start session={:?} community={} seed={} ranked={}",
                start.session_id, start.community_label, start.seed, start.ranked
            );
        }
        NetworkEvent::HostedSessionStatus(status) => {
            if let HostedSessionStatusKind::Started(start) = &status.status {
                state.hosted_start = Some(start.clone());
            }
            state.hosted_status = Some(status.clone());
            if matches!(status.status, HostedSessionStatusKind::Unavailable { .. }) {
                state.lobby_registration = None;
            }
            state.push_message(format!("Hosted session status: {:?}", status.status));
            info!(
                "network hosted status session={:?} state={:?}",
                status.session_id, status.status
            );
        }
        NetworkEvent::InputReceived(input) => {
            state.last_input = Some(input.clone());
            debug!(
                "network input received player={:?} tick={} commands={}",
                input.player, input.tick, 1
            );
        }
        NetworkEvent::TickWatermark(watermark) => {
            state.last_tick_watermark = Some(watermark.clone());
            if let Some(session) = state.connected_session.as_mut() {
                session.peer_watermark = Some(watermark.through_tick);
            }
            debug!(
                "network tick watermark player={:?} tick={}",
                watermark.player, watermark.through_tick
            );
        }
        NetworkEvent::Heartbeat(heartbeat) => {
            state.last_heartbeat = Some(heartbeat.clone());
            if let Some(session) = state.connected_session.as_mut() {
                session.peer_watermark = Some(heartbeat.watermark_tick);
            }
            info!(
                "network heartbeat player={:?} current_tick={} watermark_tick={}",
                heartbeat.player, heartbeat.current_tick, heartbeat.watermark_tick
            );
        }
        NetworkEvent::BazaarBuy(buy) => {
            state.last_bazaar_buy = Some(buy.clone());
            info!(
                "network bazaar buy player={:?} weapon={:?}",
                buy.player, buy.weapon
            );
        }
        NetworkEvent::BazaarRemove(remove) => {
            state.last_bazaar_remove = Some(remove.clone());
            info!(
                "network bazaar remove player={:?} slot={}",
                remove.player, remove.slot_index
            );
        }
        NetworkEvent::BazaarDone { player } => {
            state.last_bazaar_done_player = Some(player);
            info!("network bazaar done player={player:?}");
        }
        NetworkEvent::Checksum(checksum) => {
            state.last_checksum = Some(checksum.clone());
            info!(
                "network checksum player={:?} tick={} checksum={} events={}",
                checksum.reporter, checksum.tick, checksum.checksum, checksum.event_count
            );
        }
        NetworkEvent::DesyncDetected(report) => {
            let message = format!(
                "Desync detected. The online game stopped to avoid showing different results. Tick {}, local checksum {}, peer checksum {}",
                report.tick, report.local_checksum, report.remote_checksum
            );
            state.last_error = Some(message.clone());
            state.push_message(message.clone());
            error!(
                "network desync tick={} local_checksum={} remote_checksum={} local_events={} remote_events={}",
                report.tick,
                report.local_checksum,
                report.remote_checksum,
                report.local_event_count,
                report.remote_event_count
            );
        }
        NetworkEvent::GameOver(game_over) => {
            state.last_game_over = Some(game_over.clone());
            state.push_message(format!(
                "Peer game over: winner {:?}, loser {:?}",
                game_over.winner, game_over.loser
            ));
            info!(
                "network game over winner={:?} loser={:?} tick={} events={}",
                game_over.winner, game_over.loser, game_over.sequence, 0
            );
        }
        NetworkEvent::PendingResult(pending) => {
            state.result_status = FinalResultStatus::Pending(pending.clone());
            state.push_message("Ranked result pending: waiting for the peer claim.");
            log_pending_result(&pending);
        }
        NetworkEvent::RecordedResult(accepted) => {
            state.result_status = FinalResultStatus::Recorded;
            state.push_message("Ranked result recorded");
            info!("network result recorded session={:?}", accepted.session_id);
        }
        NetworkEvent::RejectedResult(rejected) => {
            state.result_status = FinalResultStatus::Rejected(rejected.reason.clone());
            state.last_error = Some(rejected.reason.clone());
            state.push_message(format!("Ranked result rejected: {}", rejected.reason));
            log_rejected_result(&rejected);
        }
        NetworkEvent::RankedRecords(records) => {
            let count = records.records.len();
            state.ranked_records = Some(records);
            state.push_message(format!("Fetched {count} ranked records"));
            info!("network ranked records count={count}");
        }
        NetworkEvent::StateChanged(lifecycle) => {
            if matches!(lifecycle, NetworkLifecycleState::Idle) {
                state.listening_bind_addr = None;
                state.listening_share_addr = None;
                state.pending_challenge = None;
                state.connected_session = None;
                state.lobby_registration = None;
                state.lobby_server_addr = None;
                state.hosted_status = None;
                state.hosted_start = None;
                state.lan_advertising = false;
            }
            state.lifecycle = lifecycle.clone();
            state.push_message(format!("Network state: {lifecycle:?}"));
            info!("network lifecycle state={lifecycle:?}");
        }
        NetworkEvent::Error { message } => {
            let message = player_facing_network_error(&message);
            state.lifecycle = NetworkLifecycleState::Error {
                message: message.clone(),
            };
            state.last_error = Some(message.clone());
            state.push_message(message.clone());
            error!("network error: {message}");
        }
    }
    connected
}

pub(super) fn player_facing_network_error(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("address already in use") || lower.contains("addrinuse") {
        "Host bind failed: address already in use. Try another port or cancel the old host."
            .to_string()
    } else if lower.contains("timed out connecting to direct peer")
        || lower.contains("timed out connecting to hosted direct peer")
        || lower.contains("join timed out")
    {
        format!("Join timed out. Check the host share address and firewall. {message}")
    } else if lower.contains("lobby server") || lower.contains("hosted status") {
        format!("Lobby server unavailable. Direct IP can still be used. {message}")
    } else if lower.contains("challenge denied") || lower.contains("denied") {
        let reason = message
            .split_once(':')
            .map(|(_, reason)| reason.trim().trim_end_matches('.'))
            .unwrap_or(message.trim().trim_end_matches('.'));
        format!("Challenge denied: {reason}.")
    } else if lower.contains("peer disconnected") || lower.contains("peer idle timeout") {
        format!("Peer disconnected. The online game has ended. {message}")
    } else if lower.contains("desync") {
        format!(
            "Desync detected. The online game stopped to avoid showing different results. {message}"
        )
    } else if lower.contains("ranked result") && lower.contains("reject") {
        format!("Ranked result rejected: {message}")
    } else if lower.contains("channel") || lower.contains("runtime") {
        format!("Network channel failure. Return to the menu and try again. {message}")
    } else {
        message.to_string()
    }
}

#[allow(dead_code)]
pub(super) fn try_send_network_command(
    runtime: &mut ClientNetworkRuntime,
    state: &mut ClientNetworkState,
    command: NetworkCommand,
) -> bool {
    let command_label = format!("{command:?}");
    match runtime
        .runtime
        .channels_mut()
        .command_sender()
        .try_send(command)
    {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            let message = format!("Network command queue is full: {command_label}");
            state.last_error = Some(message.clone());
            state.push_message(message.clone());
            warn!("{message}");
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            let message = format!("Network runtime is closed: {command_label}");
            state.lifecycle = NetworkLifecycleState::Error {
                message: message.clone(),
            };
            state.last_error = Some(message.clone());
            state.push_message(message.clone());
            error!("{message}");
            false
        }
    }
}

pub(super) fn log_network_session(session: &NetworkSession) {
    info!(
        "network connected mode={:?} local_slot={:?} peer={} seed={} ranked={} community={:?} tick={} peer_watermark={:?} result={:?} hosted_session={:?}",
        session.mode,
        session.local_slot,
        session.peer_identity.display_name,
        session.base_seed,
        session.ranked,
        session.community_label,
        session.current_tick,
        session.peer_watermark,
        session.final_result_status,
        session.hosted.as_ref().map(|hosted| &hosted.session_id)
    );
}

pub(super) fn log_pending_result(pending: &RankedResultPending) {
    info!(
        "network result pending session={:?} reason={}",
        pending.session_id, pending.reason
    );
}

pub(super) fn log_rejected_result(rejected: &RankedResultRejected) {
    warn!(
        "network result rejected session={:?} reason={}",
        rejected.session_id, rejected.reason
    );
}

pub(super) fn refresh_hosted_lobby(
    time: Res<Time>,
    settings: Res<ClientSettings>,
    mut runtime: ResMut<ClientNetworkRuntime>,
    mut state: ResMut<ClientNetworkState>,
) {
    if !settings.lobby_enabled {
        return;
    }
    if settings.screen != ClientScreen::Challenge && settings.screen != ClientScreen::Sleep {
        return;
    }
    let can_refresh = matches!(
        state.lifecycle,
        NetworkLifecycleState::Idle | NetworkLifecycleState::Hosting { .. }
    );
    if !can_refresh {
        return;
    }
    let elapsed_ms = time.delta().as_millis().min(u128::from(u64::MAX)) as u64;
    if settings.screen == ClientScreen::Sleep
        || (settings.challenge_style == ChallengeStyle::Modern
            && settings.challenge_mode == ChallengeMode::HostViaLobby)
    {
        state.hosted_poll_elapsed_ms = state.hosted_poll_elapsed_ms.saturating_add(elapsed_ms);
        if state.hosted_poll_elapsed_ms >= HOSTED_STATUS_POLL_INTERVAL_MS {
            state.hosted_poll_elapsed_ms = 0;
            poll_registered_hosted_status(&settings, &mut runtime, &mut state);
        }
    }
    if settings.screen == ClientScreen::Challenge
        && settings.challenge_style == ChallengeStyle::Modern
        && settings.challenge_mode == ChallengeMode::BrowseLobby
        && state.lobby_list.is_some()
    {
        state.lobby_browse_elapsed_ms = state.lobby_browse_elapsed_ms.saturating_add(elapsed_ms);
        if state.lobby_browse_elapsed_ms >= LOBBY_BROWSE_REFRESH_INTERVAL_MS {
            state.lobby_browse_elapsed_ms = 0;
            browse_hosted_lobby(&settings, &mut runtime, &mut state);
        }
    }
}

pub(super) fn refresh_server_roster(
    settings: Res<ClientSettings>,
    mut runtime: ResMut<ClientNetworkRuntime>,
    mut state: ResMut<ClientNetworkState>,
    mut last_screen: Local<Option<ClientScreen>>,
) {
    if !settings.lobby_enabled {
        return;
    }
    let entered_roster =
        settings.screen == ClientScreen::Roster && *last_screen != Some(ClientScreen::Roster);
    *last_screen = Some(settings.screen);
    if !entered_roster {
        return;
    }
    let Ok(server_addr) = parse_network_addr(&settings.lobby_addr, "lobby address", &mut state)
    else {
        return;
    };
    try_send_network_command(
        &mut runtime,
        &mut state,
        NetworkCommand::FetchRankedRecords {
            server_addr,
            limit: 20,
        },
    );
}

pub(super) fn maintain_sleep_availability(
    settings: Res<ClientSettings>,
    mut runtime: ResMut<ClientNetworkRuntime>,
    mut state: ResMut<ClientNetworkState>,
    capture: Option<Res<VisualCapture>>,
) {
    if capture.is_some() {
        return;
    }
    if settings.screen == ClientScreen::Sleep {
        if !state.sleep_availability_attempted {
            start_sleep_availability(&settings, &mut runtime, &mut state);
        }
        return;
    }

    if state.sleep_availability_attempted {
        if !matches!(state.lifecycle, NetworkLifecycleState::Connected { .. }) {
            cancel_network_challenge(&mut runtime, &mut state);
        }
        state.sleep_availability_attempted = false;
    }
}
