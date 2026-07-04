//! Render labels and player-facing UI copy.

use super::*;

pub(in crate::app) fn player_hud(
    local: &LocalGame,
    recon: &ReconPanel,
    player: PlayerId,
) -> String {
    let game = &local.game;
    if player != local.local_player && local.computer.is_some() {
        return recon_hud(game, recon, player);
    }

    let loop_state = game.player(player);
    let mut text = format!(
        "score {}  funds {}  lines {}  bazaar in {}\nnext {}\narsenal {}\neffects {}",
        loop_state.score(),
        loop_state.funds(),
        loop_state.lines(),
        game.lines_until_bazaar(),
        piece_label(loop_state.next_piece_kind_preview()),
        arsenal_label(game, player),
        active_effects_label(game, player),
    );

    if let Some(bazaar) = game.bazaar_session(player) {
        let _ = write!(
            text,
            "\nbazaar funds {}\n{}",
            bazaar.staged_funds(),
            bazaar_catalog_label(bazaar)
        );
        let _ = write!(
            text,
            "\nstaged {}",
            arsenal_slots_label(bazaar.staged_arsenal())
        );
    }

    text
}

pub(in crate::app) fn recon_hud(
    game: &TwoPlayerGame,
    recon: &ReconPanel,
    player: PlayerId,
) -> String {
    if recon.manual_condor {
        return format!(
            "Condor recon\nopponent score {}  funds {}  lines {}",
            game.player(player).score(),
            game.player(player).funds(),
            game.player(player).lines()
        );
    }
    if let Some(snapshot) = &recon.snapshot {
        return format!(
            "{:?} recon snapshot\nopponent funds {}  lines {}",
            snapshot.level,
            snapshot.funds,
            game.player(player).lines()
        );
    }
    "opponent hidden\nuse Ames/Ace/Condor or press C for Condor in computer mode".to_string()
}

pub(in crate::app) fn phase_label(
    _local: &LocalGame,
    settings: &ClientSettings,
    _sound: &SoundEventState,
) -> String {
    if settings.screen != ClientScreen::Game {
        return String::new();
    }
    String::new()
}

pub(in crate::app) fn legacy_game_text_label(
    local: &LocalGame,
    settings: &ClientSettings,
    role: LegacyGameTextRole,
) -> String {
    if settings.screen != ClientScreen::Game || local.game.phase() == GamePhase::Bazaar {
        return String::new();
    }
    match role {
        LegacyGameTextRole::Score => legacy_score_label(local),
        LegacyGameTextRole::ArsenalSlot(slot) => legacy_arsenal_slot_label(local, slot),
        LegacyGameTextRole::Message => legacy_game_message_label(local, settings.content_mode),
    }
}

pub(in crate::app) fn legacy_score_label(local: &LocalGame) -> String {
    let game = &local.game;
    let player = local.local_player;
    let opponent = opponent_player(player);
    let own = game.player(player);
    let other = game.player(opponent);
    let mut text = format!(
        "Your score:          {}\nOpponent's score:    {}\n\nYour lines:          {}\nOpponent's lines:    {}\n\nYour funds:          {}\nOpponent's funds:    {}\n\nLines 'til bazaar:   {}",
        own.score(),
        other.score(),
        own.lines(),
        other.lines(),
        own.funds(),
        other.funds(),
        game.lines_until_bazaar(),
    );
    if let Some(session) = &local.network_session {
        text.push_str("\n\n");
        text.push_str(&network_session_status_label(
            session,
            local.network_lockstep.as_ref(),
        ));
    }
    text
}

pub(in crate::app) fn network_session_status_label(
    session: &NetworkSession,
    lockstep: Option<&NetworkLockstep>,
) -> String {
    let mode = if session.hosted.is_some() {
        "Hosted Play"
    } else {
        "Direct-Connect"
    };
    let community = session
        .community_label
        .as_deref()
        .unwrap_or("unranked direct");
    let tick = lockstep
        .map(NetworkLockstep::current_tick)
        .unwrap_or(session.current_tick);
    let local_watermark = lockstep
        .map(NetworkLockstep::local_watermark)
        .unwrap_or(session.current_tick);
    let input_delay = lockstep
        .map(NetworkLockstep::input_delay_ticks)
        .unwrap_or_default();
    let peer_watermark = lockstep
        .and_then(NetworkLockstep::peer_watermark)
        .or(session.peer_watermark)
        .map(|tick| tick.to_string())
        .unwrap_or_else(|| "none".to_string());
    format!(
        "Network: {mode}  slot {:?}  peer {}\nSeed {}  tick {}  input delay {}  local watermark {}  peer watermark {}\nCommunity: {}  ranked: {}  result: {:?}",
        session.local_slot,
        session.peer_identity.display_name,
        session.base_seed,
        tick,
        input_delay,
        local_watermark,
        peer_watermark,
        community,
        session.ranked,
        session.final_result_status,
    )
}

pub(in crate::app) fn legacy_arsenal_slot_label(local: &LocalGame, slot: usize) -> String {
    let label = if slot == 9 { 0 } else { slot + 1 };
    let Some(slot) = local
        .game
        .player(local.local_player)
        .arsenal()
        .slots()
        .get(slot)
        .copied()
        .flatten()
    else {
        return format!(" {label}.  < Empty >");
    };
    let suffix = if slot.quantity > 1 {
        format!(" ({})", slot.quantity)
    } else {
        String::new()
    };
    format!(" {label}.  {}{suffix}", weapon_spec(slot.token).name)
}

pub(in crate::app) fn legacy_game_message_label(
    local: &LocalGame,
    content_mode: ContentMode,
) -> String {
    let game = &local.game;
    if let Some(message) = latest_weapon_feedback(game) {
        return message;
    }
    if let Some(message) = &local.status_message {
        return message.clone();
    }
    match game.phase() {
        GamePhase::Playing => match game.mode() {
            GameMode::HumanVsComputer { difficulty, .. } => {
                format!("Playing {} Ernie", difficulty.name)
            }
            GameMode::HumanVsHuman => match local.mode {
                LocalGameMode::DirectConnect | LocalGameMode::HostedPlay => local
                    .network_session
                    .as_ref()
                    .map(|session| {
                        network_session_status_label(session, local.network_lockstep.as_ref())
                    })
                    .unwrap_or_else(|| "Playing network game".to_string()),
                LocalGameMode::LocalHumanVsHuman => "Playing local game".to_string(),
                LocalGameMode::ComputerOpponent => "Playing computer game".to_string(),
            },
        },
        GamePhase::Paused => "Paused...".to_string(),
        GamePhase::Bazaar => String::new(),
        GamePhase::GameOver => UiTextTone::game_result_copy(
            content_mode,
            local_game_result_for(local.game.event_log(), local.local_player),
        )
        .to_string(),
    }
}

pub(in crate::app) fn menu_label(
    _game: &TwoPlayerGame,
    settings: &ClientSettings,
    _settings_edit: &SettingsEditState,
) -> String {
    match settings.screen {
        ClientScreen::Startup => String::new(),
        ClientScreen::Game => String::new(),
        ClientScreen::Challenge => "Challenge".to_string(),
        ClientScreen::Sleep => "Sleep".to_string(),
        ClientScreen::About => "About BattleTris".to_string(),
        ClientScreen::Roster => String::new(),
        ClientScreen::Settings => "Settings".to_string(),
    }
}

pub(in crate::app) fn screen_body_label(
    _game: &TwoPlayerGame,
    settings: &ClientSettings,
    _settings_edit: &SettingsEditState,
    network_state: &ClientNetworkState,
    _roster: &RosterRecords,
) -> String {
    match settings.screen {
        ClientScreen::Startup => String::new(),
        ClientScreen::Challenge => {
            challenge_screen_body_label(settings)
        }
        ClientScreen::Sleep => {
            let share_addr = effective_direct_share_addr(settings, network_state);
            let status = sleep_network_status_label(network_state);
            format!(
                "Sleep\n{} is available for BattleTris challenges.\n\nIdentity: {} ({})\nCommunity: {}\nLobby: {}  Hosted ranked: {}\nBind: {}\nShare: {}\nProtocol v{}.{} ({}, {})\n\n{}\n\nIncoming challenge controls: Enter/C accept, D deny.\nWake/Escape cancels availability and returns to Startup.",
                settings.display_name,
                hosted_player(settings).display_name,
                hosted_player_id(settings),
                settings.community_label,
                lobby_status_label(settings),
                settings.hosted_ranked,
                settings.direct_listen_addr,
                share_addr,
                PROTOCOL_MAJOR,
                PROTOCOL_MINOR,
                CAPABILITY_DIRECT_TCP,
                CAPABILITY_SELF_HOSTED_LOBBY,
                status,
            )
        }
        ClientScreen::About => {
            "BattleTris\nVersion 1.0\nBryan Cantrill\nCharlie Hoecker\nMike Shapiro\nbattletris@cs.brown.edu\nBattleTris Copyright (c) 1993-1997 Bryan Cantrill, Charles Hoecker, Michael Shapiro.\nSpecial thanks to:\nLibby \"Hoss the Camel\" Cantrill, for many ideas and extensive play-testing\nDrew Davis, for great advice early on\nTony, for cleaning up our empty Mountain Dew bottles\nbotrytis, pebbles and barney for many long and passionate nights\nThe original BT beta testers:  Ben, Caffer, Masi, Dave, Scott and Todd\nand of course\nKevin \"shouldn't there be a paren there?\" Regan"
                .to_string()
        }
        ClientScreen::Roster => " ".to_string(),
        ClientScreen::Settings => String::new(),
        ClientScreen::Game => String::new(),
    }
}

pub(in crate::app) fn challenge_screen_body_label(settings: &ClientSettings) -> String {
    match settings.challenge_style {
        ChallengeStyle::Legacy => format!(
            "Challenge\nStyle: Legacy\n\nLeft panel: available legacy players.\nRight panel: selected player information and legacy Direct IP details.\n\nControls: Enter/C challenges the selected legacy peer or accepts an incoming challenge. D denies. Escape/Cancel backs out.\n\nIdentity: {} ({})\nLegacy peer address: {}\nLegacy listen address: {}",
            settings.display_name,
            hosted_player_id(settings),
            settings.direct_join_addr,
            settings.direct_listen_addr,
        ),
        ChallengeStyle::Modern => format!(
            "Challenge\nStyle: Modern  Mode: {}\n\nLeft panel: choose Computer, Direct IP, hosted availability, a lobby opponent, LAN discovery, or legacy compatibility.\nRight panel: shows the selected mode, addresses, challenge state, and next action.\n\nControls: 1-8 choose mode. Up/Down or mouse selects opponents. Enter/C starts, refreshes, challenges, or accepts. D denies. Escape/Cancel backs out.\n\nIdentity: {} ({})  Community: {}\nProtocol v{}.{} ({}, {})",
            settings.challenge_mode.label(),
            settings.display_name,
            hosted_player_id(settings),
            settings.community_label,
            PROTOCOL_MAJOR,
            PROTOCOL_MINOR,
            CAPABILITY_DIRECT_TCP,
            CAPABILITY_SELF_HOSTED_LOBBY,
        ),
    }
}

pub(in crate::app) fn lobby_status_label(settings: &ClientSettings) -> String {
    if settings.lobby_enabled {
        settings.lobby_addr.clone()
    } else {
        "disabled by -X/--no-server".to_string()
    }
}

pub(in crate::app) fn challenge_label(
    role: ChallengeTextRole,
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    match role {
        ChallengeTextRole::UserList => challenge_opponent_list_label(settings, network_state),
        ChallengeTextRole::UserInfo => challenge_mode_panel_label(settings, network_state),
        ChallengeTextRole::ComputerStatus => {
            challenge_compact_status_label(settings, network_state)
        }
    }
}

pub(in crate::app) fn challenge_opponent_list_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    if settings.challenge_style == ChallengeStyle::Legacy {
        return legacy_challenge_player_list_label(settings, network_state);
    }

    let mut text = String::from("Modes\n");
    for (mode, key) in [
        (ChallengeMode::ComputerOpponent, "1"),
        (ChallengeMode::HostDirect, "2"),
        (ChallengeMode::JoinDirect, "3"),
        (ChallengeMode::LegacyHost, "4"),
        (ChallengeMode::LegacyJoin, "5"),
        (ChallengeMode::HostViaLobby, "6"),
        (ChallengeMode::BrowseLobby, "7"),
        (ChallengeMode::BrowseLan, "8"),
    ] {
        let marker = if settings.challenge_mode == mode {
            ">"
        } else {
            " "
        };
        let _ = writeln!(text, "{marker} {key} {}", mode.label());
    }

    let _ = write!(text, "\nOpponents\n");
    if settings.challenge_mode == ChallengeMode::BrowseLan {
        if network_state.lan_entries.is_empty() {
            text.push_str("No LAN hosts loaded.\nPress Enter or Update to browse.");
            return text;
        }
        for (index, entry) in network_state.lan_entries.iter().enumerate().take(8) {
            let marker = if index == network_state.lan_selected_index {
                ">"
            } else {
                " "
            };
            let compatibility = if entry.compatible {
                "ok"
            } else {
                "bad version"
            };
            let availability = match entry.availability {
                LanAvailability::Available => "available",
                LanAvailability::Busy => "busy",
                LanAvailability::Unknown => "unknown",
            };
            let address = entry
                .addr
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| format!("{}:{}", entry.hostname, entry.port));
            let _ = writeln!(
                text,
                "{marker} {}\n  {}  {}  {}",
                truncate_label(&entry.display_name, 24),
                availability,
                compatibility,
                address,
            );
        }
        return text;
    }
    if settings.challenge_mode != ChallengeMode::BrowseLobby {
        text.push_str("Browse Lobby or Browse LAN lists\navailable players here.");
        return text;
    }
    if !settings.lobby_enabled {
        text.push_str("Lobby server disabled by -X.\nUse Direct IP or Browse LAN.");
        return text;
    }
    let Some(list) = &network_state.lobby_list else {
        text.push_str("No lobby list loaded.\nPress Enter or Update to browse.");
        return text;
    };
    if list.entries.is_empty() {
        text.push_str("No available players.\nPress Update to refresh.");
        return text;
    }
    for (index, entry) in list.entries.iter().enumerate().take(8) {
        let marker = if index == network_state.lobby_selected_index {
            ">"
        } else {
            " "
        };
        let compatibility = if entry.protocol_major == PROTOCOL_MAJOR {
            "ok"
        } else {
            "bad version"
        };
        let ranked = if entry.ranked { "ranked" } else { "casual" };
        let _ = writeln!(
            text,
            "{marker} {}\n  {}  {}  v{}.{}",
            truncate_label(&entry.host.display_name, 24),
            ranked,
            compatibility,
            entry.protocol_major,
            entry.protocol_minor,
        );
    }
    text
}

pub(in crate::app) fn legacy_challenge_player_list_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    let mut text = String::new();
    let marker = if network_state.pending_challenge.is_none() {
        ">"
    } else {
        " "
    };
    let _ = writeln!(
        text,
        "{marker} {:<8} {:<24} {:<8}",
        truncate_label(&settings.display_name, 8),
        truncate_label(&settings.direct_join_addr, 24),
        "Waiting",
    );
    text.push_str("\nNo legacy server roster is configured.\nThis entry uses manual Direct IP.\n");
    if let Some(challenge) = &network_state.pending_challenge {
        let _ = write!(
            text,
            "\nIncoming\n> {:<8} wants to play",
            truncate_label(&challenge.challenger.display_name, 8),
        );
    }
    text
}

pub(in crate::app) fn challenge_mode_panel_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    if let Some(challenge) = &network_state.pending_challenge {
        return incoming_challenge_panel_label(challenge, network_state);
    }

    if settings.challenge_style == ChallengeStyle::Legacy {
        return legacy_challenge_info_panel_label(settings, network_state);
    }

    match settings.challenge_mode {
        ChallengeMode::ComputerOpponent => computer_challenge_panel_label(settings),
        ChallengeMode::HostDirect => host_direct_panel_label(settings, network_state),
        ChallengeMode::JoinDirect => join_direct_panel_label(settings, network_state),
        ChallengeMode::LegacyHost => legacy_host_panel_label(settings, network_state),
        ChallengeMode::LegacyJoin => legacy_join_panel_label(settings, network_state),
        ChallengeMode::HostViaLobby => host_via_lobby_panel_label(settings, network_state),
        ChallengeMode::BrowseLobby => browse_lobby_panel_label(settings, network_state),
        ChallengeMode::BrowseLan => browse_lan_panel_label(network_state),
    }
}

pub(in crate::app) fn incoming_challenge_panel_label(
    challenge: &Challenge,
    network_state: &ClientNetworkState,
) -> String {
    let hosted = challenge
        .hosted_session_id
        .as_ref()
        .map(|session| format!("Hosted session: {}", session.0))
        .unwrap_or_else(|| "Direct IP challenge".to_string());
    format!(
        "Incoming Challenge\n\n{} wants to play.\nMessage: {}\n{}\n\nEnter/C accepts.\nD denies.\nEscape cancels listening.\n\n{}",
        challenge.challenger.display_name,
        if challenge.message.is_empty() { "battle?" } else { &challenge.message },
        hosted,
        challenge_status_lifecycle_label(network_state),
    )
}

pub(in crate::app) fn computer_challenge_panel_label(settings: &ClientSettings) -> String {
    let difficulty = selected_ernie_difficulty(settings);
    let rated_copy = UiTextTone::challenge_copy(settings.content_mode);
    let rated_line = if rated_copy.is_empty() {
        String::new()
    } else {
        format!("\nErnie {rated_copy}")
    };
    format!(
        "Computer Opponent\n\nPlay against Ernie locally.\nComputer games are unranked and do not use the network.{}\n\nErnie level {} of {}\n{}\n{} ms delay\n\nUse J/Left and L/Right to change level.\nEnter/C starts the game.",
        rated_line,
        difficulty.level,
        COMPUTER_DIFFICULTIES.len() - 1,
        difficulty.name,
        difficulty.delay_ms,
    )
}

pub(in crate::app) fn host_direct_panel_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    format!(
        "Host Direct\n\nListen for one manual Direct IP challenge.\nThis fallback does not use lobby presence and is always unranked.\n\nBind: {}\nShare: {}\n\nGive the share address to the joiner. Never give 0.0.0.0 to another machine.\n\n{}",
        settings.direct_listen_addr,
        effective_direct_share_addr(settings, network_state),
        challenge_status_lifecycle_label(network_state),
    )
}

pub(in crate::app) fn join_direct_panel_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    format!(
        "Join Direct\n\nConnect to a host by manual Direct IP.\nUse this when lobby browse or LAN discovery is unavailable.\n\nJoin address: {}\nYour name: {}\n\nThe host must accept the challenge before the game starts.\n\n{}",
        settings.direct_join_addr,
        settings.display_name,
        challenge_status_lifecycle_label(network_state),
    )
}

pub(in crate::app) fn legacy_challenge_info_panel_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    format!(
        "User Information\n\nName: {}\nHost: {}\nStatus: Waiting\nProtocol: legacy packet transport\n\nLegacy Challenge mirrors the original screen: choose a player on the left, inspect details here, then press Challenge.\n\nManual Direct IP\nChallenge peer: {}\nAvailable at: {}\n\n{}",
        settings.display_name,
        settings.direct_join_addr,
        settings.direct_join_addr,
        settings.direct_listen_addr,
        legacy_transport_status_label(network_state),
    )
}

pub(in crate::app) fn legacy_host_panel_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    format!(
        "Legacy Host\n\nListen for an original BattleTris peer using the legacy packet framing.\n\nBind: {}\nShare: {}\n\nLegacy gameplay compatibility still needs the old wire adapter before this can accept real legacy clients.\n\n{}",
        settings.direct_listen_addr,
        effective_direct_share_addr(settings, network_state),
        legacy_transport_status_label(network_state),
    )
}

pub(in crate::app) fn legacy_join_panel_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    format!(
        "Legacy Join\n\nChallenge an original BattleTris peer by manual Direct IP.\n\nJoin address: {}\nYour name: {}\n\nLegacy gameplay compatibility still needs the old wire adapter before this can challenge real legacy clients.\n\n{}",
        settings.direct_join_addr,
        settings.display_name,
        legacy_transport_status_label(network_state),
    )
}

pub(in crate::app) fn legacy_transport_status_label(network_state: &ClientNetworkState) -> String {
    let mut status = "Status: legacy compatibility adapter not wired yet".to_string();
    if let Some(message) = network_state.transient_messages.last() {
        let _ = write!(status, "\nLast status: {message}");
    }
    if let Some(error) = &network_state.last_error {
        let _ = write!(status, "\nLast error: {error}");
    }
    status
}

pub(in crate::app) fn host_via_lobby_panel_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    if !settings.lobby_enabled {
        return format!(
            "Host Via Lobby\n\nLobby server disabled by -X/--no-server.\nUse Host Direct or Browse LAN for serverless play.\n\n{}",
            challenge_status_lifecycle_label(network_state),
        );
    }
    let registration = network_state.lobby_registration.as_ref().map_or_else(
        || "Not registered yet".to_string(),
        |entry| {
            format!(
                "Registered as {}\nSession: {}",
                entry.host.display_name, entry.session_id.0
            )
        },
    );
    format!(
        "Host Via Lobby\n\nBecome available in the self-hosted lobby, then wait for a challenger to connect directly.\n\nLobby: {}\nCommunity: {}\nShare: {}\nRanked requested: {}\n{}\n\n{}",
        settings.lobby_addr,
        settings.community_label,
        effective_direct_share_addr(settings, network_state),
        settings.hosted_ranked,
        registration,
        challenge_status_lifecycle_label(network_state),
    )
}

pub(in crate::app) fn browse_lobby_panel_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    if !settings.lobby_enabled {
        return format!(
            "Browse Lobby\n\nLobby server disabled by -X/--no-server.\nUse Join Direct or Browse LAN for serverless play.\n\n{}",
            challenge_status_lifecycle_label(network_state),
        );
    }
    let selected = selected_lobby_entry(network_state).map_or_else(
        || "No opponent selected".to_string(),
        |entry| {
            format!(
                "Selected: {} ({})\nDirect: {}\nSession: {}\nRanked: {}\nProtocol: v{}.{}",
                entry.host.display_name,
                entry.host.player_id,
                entry.direct_addr,
                entry.session_id.0,
                entry.ranked,
                entry.protocol_major,
                entry.protocol_minor,
            )
        },
    );
    format!(
        "Browse Lobby\n\nFind available players from the self-hosted lobby and challenge one. Gameplay still connects directly to the host.\n\nLobby: {}\nCommunity: {}\n{}\n\nUp/Down or mouse selects.\nEnter/C refreshes when empty or challenges the selected player.\n\n{}",
        settings.lobby_addr,
        settings.community_label,
        selected,
        challenge_status_lifecycle_label(network_state),
    )
}

pub(in crate::app) fn browse_lan_panel_label(network_state: &ClientNetworkState) -> String {
    let selected = selected_lan_entry(network_state).map_or_else(
        || "No LAN host selected".to_string(),
        |entry| {
            let address = entry
                .addr
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| format!("{}:{}", entry.hostname, entry.port));
            format!(
                "Selected: {}\nDirect: {}\nAvailability: {:?}\nProtocol: v{}.{}",
                entry.display_name,
                address,
                entry.availability,
                entry.protocol_major,
                entry.protocol_minor,
            )
        },
    );
    format!(
        "Browse LAN\n\nFind direct hosts advertised on the local network. This is best-effort; firewalls or mDNS blocking may hide hosts. Manual Direct IP remains the fallback.\n\n{}\n\nUp/Down or mouse selects.\nEnter/C refreshes when empty or challenges the selected host.\n\n{}",
        selected,
        challenge_status_lifecycle_label(network_state),
    )
}

pub(in crate::app) fn challenge_compact_status_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    if settings.challenge_style == ChallengeStyle::Legacy {
        return "Legacy challenge view".to_string();
    }

    match settings.challenge_mode {
        ChallengeMode::ComputerOpponent => {
            let difficulty = selected_ernie_difficulty(settings);
            format!("Ernie: {}  level {}", difficulty.name, difficulty.level)
        }
        ChallengeMode::HostViaLobby | ChallengeMode::BrowseLobby if !settings.lobby_enabled => {
            "Lobby disabled".to_string()
        }
        ChallengeMode::LegacyHost => "Legacy host pending adapter".to_string(),
        ChallengeMode::LegacyJoin => "Legacy join pending adapter".to_string(),
        ChallengeMode::BrowseLan if !network_state.lan_entries.is_empty() => {
            format!("LAN hosts: {}", network_state.lan_entries.len())
        }
        _ => challenge_status_lifecycle_label(network_state),
    }
}

pub(in crate::app) fn challenge_primary_button_label(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    if network_state.pending_challenge.is_some() {
        return "Accept".to_string();
    }
    if settings.challenge_style == ChallengeStyle::Legacy {
        return "Challenge".to_string();
    }
    match settings.challenge_mode {
        ChallengeMode::ComputerOpponent => {
            format!("Play {} Ernie", selected_ernie_difficulty(settings).name)
        }
        ChallengeMode::HostDirect => "Host Direct".to_string(),
        ChallengeMode::JoinDirect => "Join Direct".to_string(),
        ChallengeMode::LegacyHost => "Legacy Host".to_string(),
        ChallengeMode::LegacyJoin => "Legacy Join".to_string(),
        ChallengeMode::HostViaLobby if !settings.lobby_enabled => "Lobby Disabled".to_string(),
        ChallengeMode::HostViaLobby => "Host Via Lobby".to_string(),
        ChallengeMode::BrowseLobby if !settings.lobby_enabled => "Lobby Disabled".to_string(),
        ChallengeMode::BrowseLobby => selected_lobby_entry(network_state).map_or_else(
            || "Browse Lobby".to_string(),
            |entry| format!("Challenge {}", entry.host.display_name),
        ),
        ChallengeMode::BrowseLan => selected_lan_entry(network_state).map_or_else(
            || "Browse LAN".to_string(),
            |entry| format!("Challenge {}", entry.display_name),
        ),
    }
}

pub(in crate::app) fn challenge_status_lifecycle_label(
    network_state: &ClientNetworkState,
) -> String {
    let mut status = match &network_state.lifecycle {
        NetworkLifecycleState::Idle => {
            if network_state.lobby_list.is_some() {
                "Status: browsing results ready".to_string()
            } else {
                "Status: idle".to_string()
            }
        }
        NetworkLifecycleState::Hosting { bind_addr } => {
            if network_state.lobby_registration.is_some() {
                format!("Status: available via lobby; awaiting challenger on {bind_addr}")
            } else {
                format!("Status: listening on {bind_addr}; awaiting challenge")
            }
        }
        NetworkLifecycleState::Joining { peer_addr } => {
            format!("Status: challenging {peer_addr}; awaiting host response")
        }
        NetworkLifecycleState::Challenged { challenge } => format!(
            "Status: incoming challenge from {}; accept or deny",
            challenge.challenger.display_name
        ),
        NetworkLifecycleState::Connected { session } => format!(
            "Status: accepted; starting game with {} as {:?}",
            session.peer_identity.display_name, session.local_slot
        ),
        NetworkLifecycleState::Disconnecting => "Status: disconnecting".to_string(),
        NetworkLifecycleState::Error { message } => {
            if message.contains("denied") {
                format!("Status: denied - {message}")
            } else {
                format!("Status: error - {message}")
            }
        }
    };
    if let Some(entry) = &network_state.lobby_registration {
        let _ = write!(
            status,
            "\nLobby session: {} at {}",
            entry.session_id.0, entry.direct_addr
        );
    }
    if let Some(hosted_status) = &network_state.hosted_status {
        let _ = write!(status, "\nHosted status: {:?}", hosted_status.status);
    }
    if let Some(start) = &network_state.hosted_start {
        let _ = write!(
            status,
            "\nHosted start: {} seed {} ranked {}",
            start.session_id.0, start.seed, start.ranked
        );
    }
    if let Some(error) = &network_state.last_error {
        let _ = write!(status, "\nLast error: {error}");
    }
    if let Some(message) = network_state.transient_messages.last() {
        let _ = write!(status, "\nLast status: {message}");
    }
    status
}

pub(in crate::app) fn challenge_network_status_label(network_state: &ClientNetworkState) -> String {
    let mut status = match &network_state.lifecycle {
        NetworkLifecycleState::Idle => "Network: idle".to_string(),
        NetworkLifecycleState::Hosting { bind_addr } => {
            format!("Network: hosting on {bind_addr}")
        }
        NetworkLifecycleState::Joining { peer_addr } => {
            format!("Network: joining {peer_addr}")
        }
        NetworkLifecycleState::Challenged { challenge } => format!(
            "Incoming challenge from {}: Enter/C accept, D deny, Escape cancel",
            challenge.challenger.display_name
        ),
        NetworkLifecycleState::Connected { session } => format!(
            "Network: connected to {} as {:?}; starting game",
            session.peer_identity.display_name, session.local_slot
        ),
        NetworkLifecycleState::Disconnecting => "Network: disconnecting".to_string(),
        NetworkLifecycleState::Error { message } => format!("Network error: {message}"),
    };
    if let Some(bind_addr) = network_state.listening_bind_addr {
        let share_addr = network_state
            .listening_share_addr
            .map(|addr| addr.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let _ = write!(
            status,
            "\nListening bind: {bind_addr}\nListening share: {share_addr}"
        );
    }
    if let Some(error) = &network_state.last_error {
        let _ = write!(status, "\nLast error: {error}");
    }
    if let Some(message) = network_state.transient_messages.last() {
        let _ = write!(status, "\nLast status: {message}");
    }
    status
}

pub(in crate::app) fn sleep_network_status_label(network_state: &ClientNetworkState) -> String {
    let mut status = challenge_network_status_label(network_state);
    if let Some(entry) = &network_state.lobby_registration {
        let _ = write!(
            status,
            "\nLobby available as {} at {}",
            entry.host.display_name, entry.direct_addr
        );
    }
    if let Some(status_response) = &network_state.hosted_status {
        let _ = write!(
            status,
            "\nHosted session {:?}: {:?}",
            status_response.session_id, status_response.status
        );
    }
    if let Some(challenge) = &network_state.pending_challenge {
        let _ = write!(
            status,
            "\n\nChallenge prompt: {} says '{}' hosted session {:?}",
            challenge.challenger.display_name, challenge.message, challenge.hosted_session_id
        );
    }
    status
}

pub(in crate::app) fn effective_direct_share_addr(
    settings: &ClientSettings,
    network_state: &ClientNetworkState,
) -> String {
    if let Ok(addr) = settings.direct_share_addr.parse::<SocketAddr>() {
        if !addr.ip().is_unspecified() {
            return addr.to_string();
        }
    }
    if let Some(addr) = network_state.listening_share_addr {
        if !addr.ip().is_unspecified() {
            return addr.to_string();
        }
    }
    suggested_share_addr_for(&settings.direct_listen_addr)
}

pub(in crate::app) fn challenge_ernie_slider_x(settings: &ClientSettings) -> f32 {
    let max_level = (COMPUTER_DIFFICULTIES.len() - 1).max(1) as f32;
    let fraction = settings.ernie_level as f32 / max_level;
    challenge_screen_world(46.0 + 244.0 * fraction, 509.0).x
}

pub(in crate::app) fn bazaar_selection_marker_y(
    ui: &BazaarUiState,
    role: BazaarSelectionMarkerRole,
) -> f32 {
    let rows = sorted_weapon_catalog();
    let index = rows
        .iter()
        .position(|spec| spec.token == ui.selected)
        .unwrap_or_default() as f32;
    let legacy_y = match role {
        BazaarSelectionMarkerRole::Background => 210.0,
        BazaarSelectionMarkerRole::Text => 205.0,
    } + index * 16.2;
    bazaar_world(0.0, legacy_y).y
}

pub(in crate::app) fn roster_text_label(
    roster: &RosterRecords,
    network_state: &ClientNetworkState,
    role: RosterTextRole,
) -> String {
    if let Some(records) = &network_state.ranked_records {
        return hosted_roster_text_label(records, role);
    }
    match role {
        RosterTextRole::UserList => roster_user_list_label(roster),
        RosterTextRole::UserInfo1 => roster_user_info_label(roster, 0),
        RosterTextRole::UserInfo2 => " ".to_string(),
        RosterTextRole::Player1Name => roster_player_name_label(roster, 0),
        RosterTextRole::Player2Name => " ".to_string(),
        RosterTextRole::Player1Score | RosterTextRole::Player2Score => " ".to_string(),
    }
}

pub(in crate::app) fn hosted_roster_text_label(
    records: &RankedRecords,
    role: RosterTextRole,
) -> String {
    let rows = hosted_roster_rows(records);
    match role {
        RosterTextRole::UserList => {
            if rows.is_empty() {
                return format!(
                    "Community: {}\nNo hosted records",
                    truncate_label(&records.community_label, 18)
                );
            }
            let mut text = format!(
                "Community: {}\n",
                truncate_label(&records.community_label, 18)
            );
            text.push_str(
                &rows
                    .iter()
                    .take(19)
                    .map(|row| truncate_label(&row.player_key, 20))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            text
        }
        RosterTextRole::UserInfo1 => hosted_roster_user_info_label(&rows, 0),
        RosterTextRole::UserInfo2 => " ".to_string(),
        RosterTextRole::Player1Name => rows
            .first()
            .map(|row| truncate_label(&row.player_key, 14))
            .unwrap_or_else(|| " ".to_string()),
        RosterTextRole::Player2Name
        | RosterTextRole::Player1Score
        | RosterTextRole::Player2Score => " ".to_string(),
    }
}

pub(in crate::app) fn hosted_roster_user_info_label(rows: &[RosterRow], index: usize) -> String {
    let Some(row) = rows.get(index) else {
        return "No hosted Community\nrecords have been fetched.\n\nNickname: hosted\nPlan: server-owned".to_string();
    };

    format!(
        "Source: hosted Community\n          Name: {}\n          Rank: {}\n          Wins: {}\n        Losses: {}\n Highest score: {}\n Highest lines: {}\n Highest funds: {}\n        Streak: {}\n\nNickname: hosted\nPlan: server-owned records",
        truncate_label(&row.display_name, 20),
        row.rank,
        row.wins,
        row.losses,
        row.high_score,
        row.high_lines,
        row.high_funds,
        row.streak,
    )
}

pub(in crate::app) fn hosted_roster_rows(records: &RankedRecords) -> Vec<RosterRow> {
    records
        .records
        .iter()
        .map(|record| RosterRow {
            player_key: record.player_id.clone(),
            rank: record.rank,
            display_name: record.display_name.clone(),
            wins: record.wins,
            losses: record.losses,
            high_score: record.high_score,
            high_lines: record.high_lines,
            high_funds: record.high_funds,
            streak: "server".to_string(),
            fastest_kill_secs: None,
            quickest_death_secs: None,
            longest_game_secs: None,
        })
        .collect()
}

pub(in crate::app) fn roster_user_list_label(roster: &RosterRecords) -> String {
    if let Some(error) = &roster.error {
        return format!(
            "Local/offline records unavailable\n{}",
            truncate_label(error, 22)
        );
    }
    if roster.rows.is_empty() {
        return "Local/offline records\nNo records".to_string();
    }

    let mut text = String::from("Local/offline records\n");
    text.push_str(
        &roster
            .rows
            .iter()
            .take(19)
            .map(|row| truncate_label(&row.player_key, 20))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    text
}

pub(in crate::app) fn roster_user_info_label(roster: &RosterRecords, index: usize) -> String {
    if let Some(error) = &roster.error {
        return format!("Records unavailable:\n{}", truncate_label(error, 34));
    }
    let Some(row) = roster.rows.get(index) else {
        return if index == 0 {
            "No local/offline ranked\nhuman-vs-human results have\nbeen recorded.\n\nNickname: local\nPlan: offline records"
                .to_string()
        } else {
            " ".to_string()
        };
    };

    format!(
        "Source: local/offline\n          Name: {}\n          Rank: {}\n          Wins: {}\n        Losses: {}\n Highest score: {}\n Highest lines: {}\n Highest funds: {}\n        Streak: {}\n  Fastest kill: {}\nQuickest death: {}\n  Longest game: {}\n\nNickname: local\nPlan: offline records",
        truncate_label(&row.display_name, 20),
        row.rank,
        row.wins,
        row.losses,
        row.high_score,
        row.high_lines,
        row.high_funds,
        row.streak,
        roster_duration_label(row.fastest_kill_secs),
        roster_duration_label(row.quickest_death_secs),
        roster_duration_label(row.longest_game_secs),
    )
}

pub(in crate::app) fn roster_duration_label(secs: Option<u64>) -> String {
    let Some(secs) = secs else {
        return "None".to_string();
    };
    let hours = (secs / 3600).min(99);
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

pub(in crate::app) fn roster_player_name_label(roster: &RosterRecords, index: usize) -> String {
    roster
        .rows
        .get(index)
        .map(|row| truncate_label(&row.player_key, 14))
        .unwrap_or_else(|| " ".to_string())
}

pub(in crate::app) fn streak_label(kind: StreakKind, count: u64) -> String {
    match kind {
        StreakKind::None => "0 wins".to_string(),
        StreakKind::Wins => format!("{count} win{}", if count == 1 { "" } else { "s" }),
        StreakKind::Losses => format!("{count} loss{}", if count == 1 { "" } else { "es" }),
    }
}

pub(in crate::app) fn truncate_label(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!(
            "{}~",
            truncated
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        truncated
    }
}

pub(in crate::app) fn controls_label(_scheme: ControlScheme) -> &'static str {
    "original (J/L/K+Space)"
}

pub(in crate::app) fn active_effects_label(game: &TwoPlayerGame, player: PlayerId) -> String {
    let mut labels = Vec::new();
    let effects = game.player(player).active_effects();
    for spec in WEAPON_CATALOG {
        let remaining = effects.remaining_lines(spec.token);
        if remaining > 0 {
            labels.push(format!("{}:{remaining}", short_weapon_name(spec.token)));
        }
    }

    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.join(", ")
    }
}

pub(in crate::app) fn latest_weapon_feedback(game: &TwoPlayerGame) -> Option<String> {
    game.event_log()
        .iter()
        .rev()
        .find_map(|logged| match &logged.event {
            BattleEvent::WeaponLaunched {
                launcher,
                target,
                token,
            } => Some(format!(
                "{:?} launched {} at {:?}",
                launcher,
                weapon_spec(*token).name,
                target,
            )),
            BattleEvent::OneShotWeaponApplied {
                launcher,
                target,
                token,
            } => Some(format!(
                "{} from {:?} hit {:?}",
                weapon_spec(*token).name,
                launcher,
                target,
            )),
            BattleEvent::TimedWeaponActivated {
                launcher,
                target,
                token,
                remaining_lines,
            } => Some(format!(
                "{} active on {:?} for {} lines from {:?}",
                weapon_spec(*token).name,
                target,
                remaining_lines,
                launcher,
            )),
            BattleEvent::TimedWeaponExpired { player, token } => Some(format!(
                "{} expired for {:?}",
                weapon_spec(*token).name,
                player,
            )),
            BattleEvent::IncomingWeaponQueued {
                launcher,
                target,
                token,
            } => Some(format!(
                "{} incoming from {:?} to {:?}",
                weapon_spec(*token).name,
                launcher,
                target,
            )),
            BattleEvent::WeaponReflected { player, token } => Some(format!(
                "Mirror reflected {} back onto {:?}",
                weapon_spec(*token).name,
                player,
            )),
            BattleEvent::WeaponNullified { player, token } => Some(format!(
                "Mirror nullified {} for {:?}",
                weapon_spec(*token).name,
                player,
            )),
            _ => None,
        })
}

pub(in crate::app) fn arsenal_label(game: &TwoPlayerGame, player: PlayerId) -> String {
    arsenal_slots_label(game.player(player).arsenal())
}

pub(in crate::app) fn arsenal_slots_label(arsenal: &battletris_core::weapons::Arsenal) -> String {
    let labels = arsenal
        .slots()
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| {
            slot.map(|slot| {
                let label = if index == 9 {
                    "0".to_string()
                } else {
                    (index + 1).to_string()
                };
                format!(
                    "{label}:{}x{}",
                    short_weapon_name(slot.token),
                    slot.quantity
                )
            })
        })
        .collect::<Vec<_>>();

    if labels.is_empty() {
        "empty".to_string()
    } else {
        labels.join(" ")
    }
}

pub(in crate::app) fn bazaar_catalog_label(bazaar: &battletris_core::weapons::Bazaar) -> String {
    bazaar_catalog_keys()
        .into_iter()
        .map(|(token, key)| {
            format!(
                "{} {:?} ${}",
                short_weapon_name(token),
                key,
                bazaar.price(token)
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}

pub(in crate::app) fn bazaar_text_label(
    role: BazaarTextRole,
    local: &LocalGame,
    ui: &BazaarUiState,
    content_mode: ContentMode,
) -> String {
    if local.game.phase() != GamePhase::Bazaar {
        return String::new();
    }

    let Some(bazaar) = local.game.bazaar_session(local.local_player) else {
        return "Bazaar closed".to_string();
    };
    match role {
        BazaarTextRole::Catalog => bazaar_catalog_widget_label(bazaar),
        BazaarTextRole::SelectedCatalogRow => weapon_spec(ui.selected).name.to_string(),
        BazaarTextRole::Funds => {
            if bazaar.carter_prices() {
                format!("{}\nCarter prices", bazaar.staged_funds())
            } else {
                bazaar.staged_funds().to_string()
            }
        }
        BazaarTextRole::ArsenalSlot(slot) => bazaar_arsenal_slot_widget_label(bazaar, ui, slot),
        BazaarTextRole::Message => bazaar_message_widget_label(local, ui, content_mode),
        BazaarTextRole::Description => bazaar_description_widget_label(bazaar, ui.selected),
    }
}

pub(in crate::app) fn bazaar_catalog_widget_label(
    bazaar: &battletris_core::weapons::Bazaar,
) -> String {
    sorted_weapon_catalog()
        .into_iter()
        .map(|spec| {
            let price_marker = if bazaar.price(spec.token) <= bazaar.staged_funds() {
                ""
            } else {
                " *"
            };
            format!("{}{price_marker}", spec.name)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(in crate::app) fn bazaar_arsenal_slot_widget_label(
    bazaar: &battletris_core::weapons::Bazaar,
    ui: &BazaarUiState,
    slot: usize,
) -> String {
    if let Some(visual_arsenal) = &ui.visual_arsenal {
        return visual_arsenal[slot]
            .map(|token| weapon_spec(token).name.to_string())
            .unwrap_or_else(|| "< Empty >".to_string());
    }
    let Some(slot) = bazaar.staged_arsenal().slots().get(slot).copied().flatten() else {
        return "< Empty >".to_string();
    };
    if slot.quantity > 1 {
        format!("{} ({})", weapon_spec(slot.token).name, slot.quantity)
    } else {
        weapon_spec(slot.token).name.to_string()
    }
}

pub(in crate::app) fn bazaar_message_widget_label(
    local: &LocalGame,
    ui: &BazaarUiState,
    content_mode: ContentMode,
) -> String {
    if local.game.bazaar_player_done(local.local_player) {
        wrap_bazaar_description(UiTextTone::bazaar_done_overlay_copy(content_mode), 34)
    } else if ui.last_message.trim().is_empty() {
        wrap_bazaar_description(UiTextTone::bazaar_instructions_copy(content_mode), 34)
    } else {
        ui.last_message.clone()
    }
}

pub(in crate::app) fn bazaar_description_widget_label(
    bazaar: &battletris_core::weapons::Bazaar,
    selected: WeaponToken,
) -> String {
    let spec = weapon_spec(selected);
    let mut text = format!(
        "Price:    {}\nDuration: {} lines\n\n",
        bazaar.price(spec.token),
        spec.line_duration,
    );
    text.push_str(&wrap_bazaar_description(spec.description, 37));
    text
}

pub(in crate::app) fn wrap_bazaar_description(description: &str, width: usize) -> String {
    let mut output = String::new();
    let mut line_len = 0_usize;
    for word in description.split_whitespace() {
        let word_len = word.chars().count();
        if line_len > 0 && line_len + 1 + word_len > width {
            output.push('\n');
            line_len = 0;
        } else if line_len > 0 {
            output.push(' ');
            line_len += 1;
        }
        output.push_str(word);
        line_len += word_len;
    }
    output
}

pub(in crate::app) fn sorted_weapon_catalog() -> Vec<&'static battletris_core::weapons::WeaponSpec>
{
    let mut rows = WEAPON_CATALOG.iter().collect::<Vec<_>>();
    rows.sort_by_key(|spec| (spec.price, spec.token.legacy_id()));
    rows
}

pub(in crate::app) fn short_weapon_name(token: WeaponToken) -> &'static str {
    match token {
        WeaponToken::FearedWeird => "Weird",
        WeaponToken::FourByFour => "4x4",
        WeaponToken::Hatter => "Hatter",
        WeaponToken::Upbyside => "Upside",
        WeaponToken::FallOut => "Fallout",
        WeaponToken::Swap => "Swap",
        WeaponToken::Lawyers => "Lawyer",
        WeaponToken::RiseUp => "Rise",
        WeaponToken::FlipOut => "Flip",
        WeaponToken::Speedy => "Speedy",
        WeaponToken::Missing => "Miss",
        WeaponToken::PieceIt => "Piece",
        WeaponToken::Blind => "Blind",
        WeaponToken::Mondale => "Tax",
        WeaponToken::Keating => "Keating",
        WeaponToken::Carter => "Carter",
        WeaponToken::Reagan => "Reagan",
        WeaponToken::Ames => "Ames",
        WeaponToken::Ace => "Ace",
        WeaponToken::Condor => "Condor",
        WeaponToken::NiceDay => "Nice",
        WeaponToken::SoLong => "NoLong",
        WeaponToken::NoDice => "NoDice",
        WeaponToken::Bug => "Bug",
        WeaponToken::Bottle => "Bottle",
        WeaponToken::NoSlide => "NoSlide",
        WeaponToken::Susan => "Susan",
        WeaponToken::Meadow => "Meadow",
        WeaponToken::Mirror => "Mirror",
        WeaponToken::Twilight => "Twilight",
        WeaponToken::Slick => "Slick",
        WeaponToken::Broken => "Broken",
        WeaponToken::Force => "Force",
        WeaponToken::Gimp => "Gimp",
    }
}

pub(in crate::app) fn piece_label(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::El => "L",
        PieceKind::ReverseEl => "reverse L",
        PieceKind::SlantRight => "slant right",
        PieceKind::SlantLeft => "slant left",
        PieceKind::Long => "long",
        PieceKind::Plug => "plug",
        PieceKind::Box => "box",
        PieceKind::Die => "die",
        PieceKind::Happy => "happy",
        PieceKind::Dog => "dog",
        PieceKind::ReverseDog => "reverse dog",
        PieceKind::Cap => "cap",
        PieceKind::Wall => "wall",
        PieceKind::Tower => "tower",
        PieceKind::Star => "star",
        PieceKind::WeirdLong => "weird long",
        PieceKind::FourByFour => "four-by-four",
        PieceKind::LongDong => "long dong",
    }
}

pub(in crate::app) fn cell_sprite(
    cell: Cell,
    _active: bool,
    theme: &LoadedTheme,
) -> RenderedCellSprite {
    match cell {
        Cell::Visible { color } => {
            let index = usize::from(color.get().saturating_sub(1))
                % theme.palette.visible_colors.len().max(1);
            RenderedCellSprite {
                atlas_index: theme.cell_atlas.cells.visible_colors[index],
                tint: theme
                    .palette
                    .visible_colors
                    .get(index)
                    .copied()
                    .unwrap_or(theme.palette.text_accent),
            }
        }
        Cell::Structure => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.structure,
            tint: theme.palette.structure,
        },
        Cell::Happy => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.happy,
            tint: theme.palette.happy,
        },
        Cell::Frown => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.frown,
            tint: theme.palette.frown,
        },
        Cell::Gimp { .. } => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.gimp,
            tint: theme.palette.gimp,
        },
        Cell::Die { pip } => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.die[usize::from(pip.get() - 1)],
            tint: theme.palette.die,
        },
        Cell::Invisible => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.invisible,
            tint: theme.palette.invisible,
        },
        Cell::Hidden { .. } => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.hidden,
            tint: theme.palette.hidden,
        },
    }
}

pub(in crate::app) fn empty_cell_sprite(theme: &LoadedTheme) -> RenderedCellSprite {
    RenderedCellSprite {
        atlas_index: theme.cell_atlas.cells.empty,
        tint: theme.palette.empty,
    }
}

pub(in crate::app) fn local_game_result_for(
    log: &[battletris_core::game::LoggedEvent],
    local_player: PlayerId,
) -> Option<bool> {
    log.iter().rev().find_map(|logged| match logged.event {
        BattleEvent::GameOver { winner, loser }
            if winner == local_player || loser == local_player =>
        {
            Some(winner == local_player)
        }
        _ => None,
    })
}

pub(in crate::app) const fn player_label(player: PlayerId) -> &'static str {
    match player {
        PlayerId::One => "Player 1",
        PlayerId::Two => "Player 2",
    }
}

pub(in crate::app) fn cell_x(theme: &LoadedTheme, left: f32, x: usize) -> f32 {
    left + x as f32 * theme.cell.size + theme.cell.size / 2.0
}

pub(in crate::app) fn cell_y(theme: &LoadedTheme, y: usize) -> f32 {
    theme.layout.board.top - y as f32 * theme.cell.size - theme.cell.size / 2.0
}

pub(in crate::app) const fn opponent_player(player: PlayerId) -> PlayerId {
    match player {
        PlayerId::One => PlayerId::Two,
        PlayerId::Two => PlayerId::One,
    }
}

pub(in crate::app) const fn client_player_index(player: PlayerId) -> usize {
    match player {
        PlayerId::One => 0,
        PlayerId::Two => 1,
    }
}
