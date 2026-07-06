//! Keyboard, mouse, settings, challenge, and Bazaar input handling.

use super::*;

#[derive(SystemParam)]
pub(in crate::app) struct KeyboardInputParams<'w> {
    pub(in crate::app) time: Res<'w, Time>,
    pub(in crate::app) keys: Res<'w, ButtonInput<KeyCode>>,
    pub(in crate::app) local: ResMut<'w, LocalGame>,
    pub(in crate::app) settings: ResMut<'w, ClientSettings>,
    pub(in crate::app) settings_edit: ResMut<'w, SettingsEditState>,
    pub(in crate::app) network_runtime: ResMut<'w, ClientNetworkRuntime>,
    pub(in crate::app) network_state: ResMut<'w, ClientNetworkState>,
    pub(in crate::app) sound: ResMut<'w, SoundEventState>,
    pub(in crate::app) repeat: ResMut<'w, InputRepeatState>,
    pub(in crate::app) recon: ResMut<'w, ReconPanel>,
    pub(in crate::app) bazaar_ui: ResMut<'w, BazaarUiState>,
    pub(in crate::app) capture: Option<Res<'w, VisualCapture>>,
}

pub(in crate::app) fn handle_keyboard_input(mut input: KeyboardInputParams) {
    if input.capture.is_some() {
        return;
    }
    handle_screen_shortcuts(&input.keys, &mut input.settings, &mut input.sound);
    let elapsed_ms = input.time.delta().as_millis().min(u128::from(u64::MAX)) as u64;

    match input.settings.screen {
        ClientScreen::Startup => handle_startup_input(
            &input.keys,
            &mut input.local,
            &mut input.settings,
            &mut input.sound,
        ),
        ClientScreen::Challenge => handle_challenge_input(
            &input.keys,
            &mut input.local,
            &mut input.settings,
            &mut input.network_runtime,
            &mut input.network_state,
            &mut input.sound,
        ),
        ClientScreen::Settings => {
            handle_settings_input(
                &input.keys,
                &mut input.settings,
                &mut input.settings_edit,
                &mut input.sound,
            );
        }
        ClientScreen::Game => handle_game_input(
            GameInputContext {
                keys: &input.keys,
                local: &mut input.local,
                network_runtime: &mut input.network_runtime,
                network_state: &mut input.network_state,
                settings: &input.settings,
                repeat: &mut input.repeat,
                recon: &mut input.recon,
                bazaar_ui: &mut input.bazaar_ui,
            },
            elapsed_ms,
        ),
        ClientScreen::Sleep => handle_sleep_input(
            &input.keys,
            &mut input.settings,
            &mut input.network_runtime,
            &mut input.network_state,
            &mut input.sound,
        ),
        ClientScreen::About | ClientScreen::Roster => {}
    }
}

#[derive(SystemParam)]
pub(in crate::app) struct MouseButtonParams<'w, 's> {
    pub(in crate::app) mouse: Res<'w, ButtonInput<MouseButton>>,
    pub(in crate::app) window: Single<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub(in crate::app) buttons: Query<'w, 's, &'static MenuButton>,
    pub(in crate::app) local: ResMut<'w, LocalGame>,
    pub(in crate::app) settings: ResMut<'w, ClientSettings>,
    pub(in crate::app) network_runtime: ResMut<'w, ClientNetworkRuntime>,
    pub(in crate::app) network_state: ResMut<'w, ClientNetworkState>,
    pub(in crate::app) themes: Res<'w, ThemePacks>,
    pub(in crate::app) sound: ResMut<'w, SoundEventState>,
    pub(in crate::app) bazaar_ui: ResMut<'w, BazaarUiState>,
    pub(in crate::app) app_exit: MessageWriter<'w, AppExit>,
    pub(in crate::app) capture: Option<Res<'w, VisualCapture>>,
}

pub(in crate::app) fn handle_mouse_buttons(mut input: MouseButtonParams) {
    if input.capture.is_some() {
        return;
    }
    if !input.mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = input.window.cursor_position() else {
        return;
    };
    let world = Vec2::new(
        cursor.x - input.window.width() / 2.0,
        input.window.height() / 2.0 - cursor.y,
    );
    if input.settings.screen == ClientScreen::Game && input.local.game.phase() == GamePhase::Bazaar
    {
        let theme = input.themes.get(input.settings.theme);
        handle_bazaar_click(
            world,
            theme,
            &mut input.local,
            &mut input.network_runtime,
            &mut input.network_state,
            &mut input.bazaar_ui,
            input.settings.content_mode,
        );
        return;
    }
    if input.settings.screen == ClientScreen::Challenge
        && if input.settings.challenge_style == ChallengeStyle::Legacy {
            select_lobby_entry_at_world(world, &mut input.network_state)
        } else {
            select_challenge_entry_at_world(
                world,
                input.settings.challenge_mode,
                &mut input.network_state,
            )
        }
    {
        queue_sound(&mut input.sound, SoundEvent::MenuAction);
        return;
    }
    if input.settings.screen == ClientScreen::Settings {
        return;
    }
    let Some(button) = input
        .buttons
        .iter()
        .find(|button| button.screen == input.settings.screen && button.rect.contains(world))
    else {
        return;
    };

    apply_menu_action(
        button.action,
        &mut input.local,
        &mut input.settings,
        &mut input.network_runtime,
        &mut input.network_state,
        &mut input.sound,
        &mut input.app_exit,
    );
}

pub(in crate::app) fn handle_settings_ui_interactions(
    mut settings: ResMut<ClientSettings>,
    mut edit: ResMut<SettingsEditState>,
    mut sound: ResMut<SoundEventState>,
    controls: Query<(&Interaction, &SettingsUiControlButton), Changed<Interaction>>,
    back_buttons: Query<&Interaction, (Changed<Interaction>, With<SettingsUiBackButton>)>,
) {
    if settings.screen != ClientScreen::Settings {
        return;
    }

    let previous = settings.persisted();
    let mut interacted = false;
    for (interaction, button) in &controls {
        if *interaction != Interaction::Pressed {
            continue;
        }
        interacted = true;
        match button.action {
            SettingsUiAction::Focus => edit.focus(button.control),
            SettingsUiAction::Activate => {
                edit.focus(button.control);
                activate_settings_control(&mut settings, button.control);
            }
            SettingsUiAction::ToggleDropdown => edit.toggle_dropdown(button.control),
            SettingsUiAction::Select(option) => {
                edit.focus(button.control);
                apply_settings_select_option(&mut settings, option);
                edit.close_dropdown();
            }
            SettingsUiAction::Decrement => {
                edit.focus(button.control);
                adjust_settings_pixel_scale(&mut settings, -0.25);
            }
            SettingsUiAction::Increment => {
                edit.focus(button.control);
                adjust_settings_pixel_scale(&mut settings, 0.25);
            }
        }
    }
    for interaction in &back_buttons {
        if *interaction == Interaction::Pressed {
            interacted = true;
            edit.close_dropdown();
            settings.screen = ClientScreen::Startup;
        }
    }

    if interacted {
        queue_sound(&mut sound, SoundEvent::MenuAction);
    }
    if settings.persisted() != previous {
        settings.save();
    }
}

pub(in crate::app) fn update_settings_ui_visibility(
    settings: Res<ClientSettings>,
    mut roots: Query<&mut Visibility, With<SettingsUiRoot>>,
) {
    for mut visibility in &mut roots {
        *visibility = if settings.screen == ClientScreen::Settings {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(in crate::app) fn update_settings_ui_dropdown_visibility(
    settings: Res<ClientSettings>,
    edit: Res<SettingsEditState>,
    mut menus: Query<(&SettingsUiDropdownMenu, &mut Node)>,
) {
    for (menu, mut node) in &mut menus {
        node.display = if settings.screen == ClientScreen::Settings
            && edit.open_dropdown == Some(menu.control)
        {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(in crate::app) fn update_settings_ui_text(
    settings: Res<ClientSettings>,
    edit: Res<SettingsEditState>,
    mut values: Query<(&SettingsUiValueText, &mut Text)>,
    mut statuses: Query<&mut Text, (With<SettingsUiStatusText>, Without<SettingsUiValueText>)>,
) {
    for (value, mut text) in &mut values {
        text.0 = settings_ui_value_label(&settings, &edit, value.value);
    }
    for mut status in &mut statuses {
        status.0 = settings_ui_status_label(&settings);
    }
}

pub(in crate::app) fn update_settings_ui_theme(
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    asset_server: Res<AssetServer>,
    mut surfaces: Query<(&SettingsUiSurface, &mut BackgroundColor, &mut BorderColor)>,
    mut text_colors: Query<(&SettingsUiTextColor, &mut TextColor)>,
    mut biff_images: Query<&mut ImageNode, With<SettingsUiBiffImage>>,
) {
    let theme = themes.get(settings.theme);
    for (surface, mut background, mut border) in &mut surfaces {
        let (surface_background, surface_border) =
            settings_ui_surface_style(settings.ui_style, theme, surface.role);
        background.0 = surface_background;
        *border = surface_border;
    }
    for (style_text, mut text_color) in &mut text_colors {
        text_color.0 = settings_ui_text_color(settings.ui_style, theme, style_text.role);
    }
    for mut image in &mut biff_images {
        image.image = asset_server.load(theme.sprites.biff.clone());
    }
}

pub(in crate::app) fn update_settings_ui_visuals(
    settings: Res<ClientSettings>,
    edit: Res<SettingsEditState>,
    themes: Res<ThemePacks>,
    mut controls: Query<(
        &SettingsUiControlButton,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut back_buttons: SettingsUiBackButtonVisualQuery,
) {
    let enabled = settings.screen == ClientScreen::Settings;
    let theme = themes.get(settings.theme);
    for (button, interaction, mut background, mut border) in &mut controls {
        let focused = enabled && edit.control == button.control;
        background.0 = settings_ui_control_background(
            settings.ui_style,
            button.action,
            *interaction,
            focused,
            enabled,
        );
        *border = settings_ui_control_border(
            settings.ui_style,
            button.action,
            *interaction,
            focused,
            enabled,
        );
    }
    for (interaction, mut background, mut border) in &mut back_buttons {
        background.0 =
            settings_ui_back_button_background(settings.ui_style, theme, *interaction, enabled);
        *border = settings_ui_back_button_border(settings.ui_style, *interaction, enabled);
    }
}

pub(in crate::app) fn settings_ui_value_label(
    settings: &ClientSettings,
    edit: &SettingsEditState,
    value: SettingsUiValue,
) -> String {
    match value {
        SettingsUiValue::UiStyle => settings.ui_style.label().to_string(),
        SettingsUiValue::Theme => settings.theme.label().to_string(),
        SettingsUiValue::SoundPack => settings.sound_pack.label().to_string(),
        SettingsUiValue::ChallengeStyle => settings.challenge_style.label().to_string(),
        SettingsUiValue::HostedRanked => {
            if settings.hosted_ranked {
                "x".to_string()
            } else {
                String::new()
            }
        }
        SettingsUiValue::PixelScale => format!("{:.2}x", settings.pixel_scale),
        SettingsUiValue::Field(field) => {
            let mut label = truncate_label(settings_field_value(settings, field), 40);
            if edit.control == SettingsControl::Text(field) {
                label.push('|');
            }
            label
        }
    }
}

pub(in crate::app) fn settings_ui_status_label(settings: &ClientSettings) -> String {
    let settings_path = settings
        .settings_path
        .as_ref()
        .map(|path| truncate_label(&path.display().to_string(), 54))
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "UI: {}    Controls: {}    Lobby: {}\nProtocol v{}.{}    Settings: {}",
        settings.ui_style.label(),
        controls_label(settings.controls),
        lobby_status_label(settings),
        PROTOCOL_MAJOR,
        PROTOCOL_MINOR,
        settings_path,
    )
}

pub(in crate::app) fn apply_menu_action(
    action: MenuAction,
    local: &mut LocalGame,
    settings: &mut ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    sound: &mut SoundEventState,
    app_exit: &mut MessageWriter<AppExit>,
) {
    match action {
        MenuAction::StartSelectedChallenge => {
            if settings.screen == ClientScreen::Challenge {
                start_selected_challenge_mode(
                    local,
                    settings,
                    network_runtime,
                    network_state,
                    sound,
                );
            }
        }
        MenuAction::UpdateChallenge => {
            if settings.screen == ClientScreen::Challenge {
                update_challenge_mode(settings, network_runtime, network_state);
            }
        }
        MenuAction::StartHumanVsComputer => {
            *local = LocalGame::new_human_vs_computer(settings.ernie_level);
            settings.screen = ClientScreen::Game;
            sound.next_log_index = 0;
        }
        MenuAction::GoTo(screen) => {
            if settings.screen == ClientScreen::Challenge && screen == ClientScreen::Startup {
                cancel_network_challenge(network_runtime, network_state);
            }
            if settings.screen == ClientScreen::Game
                && local.is_networked()
                && screen != ClientScreen::Game
            {
                leave_network_game(local, network_runtime, network_state, "left online game");
            }
            settings.screen = screen;
        }
        MenuAction::Quit => {
            app_exit.write(AppExit::Success);
        }
    }
    queue_sound(sound, SoundEvent::MenuAction);
}

pub(in crate::app) fn queue_sound(sound: &mut SoundEventState, event: SoundEvent) {
    sound.last_event = Some(event);
    sound.pending_events.push(event);
}

pub(in crate::app) fn handle_screen_shortcuts(
    keys: &ButtonInput<KeyCode>,
    settings: &mut ClientSettings,
    sound: &mut SoundEventState,
) {
    let target = if keys.just_pressed(KeyCode::F1) {
        Some(ClientScreen::Startup)
    } else if keys.just_pressed(KeyCode::F2) {
        Some(ClientScreen::Challenge)
    } else if keys.just_pressed(KeyCode::F3) {
        Some(ClientScreen::Settings)
    } else if keys.just_pressed(KeyCode::F4) {
        Some(ClientScreen::About)
    } else if keys.just_pressed(KeyCode::F5) {
        Some(ClientScreen::Roster)
    } else if keys.just_pressed(KeyCode::F6) {
        Some(ClientScreen::Sleep)
    } else if keys.just_pressed(KeyCode::Escape)
        && settings.screen != ClientScreen::Challenge
        && settings.screen != ClientScreen::Sleep
    {
        Some(ClientScreen::Game)
    } else {
        None
    };

    if let Some(screen) = target {
        settings.screen = screen;
        queue_sound(sound, SoundEvent::MenuAction);
    }
}

pub(in crate::app) fn handle_sleep_input(
    keys: &ButtonInput<KeyCode>,
    settings: &mut ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    sound: &mut SoundEventState,
) {
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyW) {
        cancel_network_challenge(network_runtime, network_state);
        network_state.sleep_availability_attempted = false;
        settings.screen = ClientScreen::Startup;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyD) && network_state.pending_challenge.is_some() {
        deny_pending_direct_challenge(network_runtime, network_state);
        queue_sound(sound, SoundEvent::ChallengeRejected);
    }
    if (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyC))
        && network_state.pending_challenge.is_some()
    {
        accept_pending_direct_challenge(network_runtime, network_state);
        queue_sound(sound, SoundEvent::MenuAction);
    }
}

pub(in crate::app) fn handle_startup_input(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    settings: &mut ClientSettings,
    sound: &mut SoundEventState,
) {
    if keys.just_pressed(KeyCode::KeyH) {
        *local = LocalGame::new_human_vs_human();
        settings.screen = ClientScreen::Game;
        sound.next_log_index = 0;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        *local = LocalGame::new_human_vs_computer(settings.ernie_level);
        settings.screen = ClientScreen::Game;
        sound.next_log_index = 0;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyT) {
        toggle_theme(settings);
        settings.save();
        queue_sound(sound, SoundEvent::MenuAction);
    }
}

pub(in crate::app) fn handle_challenge_input(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    settings: &mut ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    sound: &mut SoundEventState,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        settings.challenge_mode = ChallengeMode::ComputerOpponent;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        settings.challenge_mode = ChallengeMode::HostDirect;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        settings.challenge_mode = ChallengeMode::JoinDirect;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit4) {
        settings.challenge_mode = ChallengeMode::HostViaLobby;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit5) {
        settings.challenge_mode = ChallengeMode::BrowseLobby;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit6) {
        settings.challenge_mode = ChallengeMode::BrowseLan;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if settings.challenge_style == ChallengeStyle::Legacy
        && (keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyK))
    {
        select_lobby_entry(network_state, -1);
        queue_sound(sound, SoundEvent::MenuAction);
    } else if matches!(
        settings.challenge_mode,
        ChallengeMode::BrowseLobby | ChallengeMode::BrowseLan
    ) && (keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyK))
    {
        select_challenge_entry(network_state, settings.challenge_mode, -1);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if settings.challenge_style == ChallengeStyle::Legacy
        && (keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyI))
    {
        select_lobby_entry(network_state, 1);
        queue_sound(sound, SoundEvent::MenuAction);
    } else if matches!(
        settings.challenge_mode,
        ChallengeMode::BrowseLobby | ChallengeMode::BrowseLan
    ) && (keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyI))
    {
        select_challenge_entry(network_state, settings.challenge_mode, 1);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyJ) {
        adjust_ernie_level(settings, -1);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyL) {
        adjust_ernie_level(settings, 1);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Escape) {
        cancel_network_challenge(network_runtime, network_state);
        settings.screen = ClientScreen::Startup;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyD) && network_state.pending_challenge.is_some() {
        deny_pending_direct_challenge(network_runtime, network_state);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyC) {
        start_selected_challenge_mode(local, settings, network_runtime, network_state, sound);
        queue_sound(sound, SoundEvent::MenuAction);
    }
}

pub(in crate::app) fn start_selected_challenge_mode(
    local: &mut LocalGame,
    settings: &mut ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    sound: &mut SoundEventState,
) {
    if network_state.pending_challenge.is_some() {
        accept_pending_direct_challenge(network_runtime, network_state);
        return;
    }

    if settings.challenge_style == ChallengeStyle::Legacy {
        start_legacy_challenge_mode(settings, network_runtime, network_state);
        return;
    }

    match settings.challenge_mode {
        ChallengeMode::ComputerOpponent => {
            *local = LocalGame::new_human_vs_computer(settings.ernie_level);
            settings.screen = ClientScreen::Game;
            sound.next_log_index = 0;
        }
        ChallengeMode::HostDirect => {
            host_direct_challenge(settings, network_runtime, network_state)
        }
        ChallengeMode::JoinDirect => {
            join_direct_challenge(settings, network_runtime, network_state)
        }
        ChallengeMode::HostViaLobby => {
            if !settings.lobby_enabled {
                network_state.push_message("Lobby server disabled by -X/--no-server");
                return;
            }
            host_via_lobby_challenge(settings, network_runtime, network_state)
        }
        ChallengeMode::BrowseLobby => {
            if !settings.lobby_enabled {
                network_state.push_message("Lobby server disabled by -X/--no-server");
                return;
            }
            start_or_browse_hosted_lobby(settings, network_runtime, network_state)
        }
        ChallengeMode::BrowseLan => start_or_browse_lan(settings, network_runtime, network_state),
    }
}

pub(in crate::app) fn start_legacy_challenge_mode(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    let selected_addr = selected_lobby_entry(network_state).map(|entry| entry.direct_addr.clone());
    start_legacy_join_challenge(
        settings,
        network_runtime,
        network_state,
        selected_addr.as_deref(),
    );
}

pub(in crate::app) fn start_legacy_host_challenge(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    let Ok(bind_addr) = parse_network_addr(
        &settings.direct_listen_addr,
        "legacy host bind",
        network_state,
    ) else {
        return;
    };
    let server_addr = if settings.lobby_enabled {
        match parse_network_addr(
            &settings.legacy_server_addr,
            "legacy server address",
            network_state,
        ) {
            Ok(server_addr) => Some(server_addr),
            Err(_) => return,
        }
    } else {
        None
    };
    let share_addr = effective_direct_share_addr(settings, network_state);
    let Ok(share_addr) = parse_network_addr(&share_addr, "legacy share address", network_state)
    else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::LegacyHost {
            bind_addr,
            identity: direct_identity(settings),
            share_addr,
            server_addr,
        },
    );
}

pub(in crate::app) fn start_legacy_join_challenge(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    selected_addr: Option<&str>,
) {
    let join_addr = selected_addr.unwrap_or(&settings.direct_join_addr);
    let Ok(peer_addr) = parse_network_addr(join_addr, "legacy join address", network_state) else {
        return;
    };
    let share_addr = effective_direct_share_addr(settings, network_state);
    let Ok(share_addr) = parse_network_addr(&share_addr, "legacy share address", network_state)
    else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::LegacyJoin {
            peer_addr,
            identity: direct_identity(settings),
            share_addr,
        },
    );
}

pub(in crate::app) fn update_challenge_mode(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if settings.challenge_style == ChallengeStyle::Legacy {
        if matches!(
            network_state.lifecycle,
            NetworkLifecycleState::Hosting { .. }
        ) {
            if let Ok(server_addr) = parse_network_addr(
                &settings.legacy_server_addr,
                "legacy server address",
                network_state,
            ) {
                try_send_network_command(
                    network_runtime,
                    network_state,
                    NetworkCommand::BrowseLobby {
                        server_addr,
                        ranked_only: false,
                    },
                );
            }
            return;
        }
        start_legacy_host_challenge(settings, network_runtime, network_state);
        return;
    }

    match settings.challenge_mode {
        ChallengeMode::HostDirect | ChallengeMode::HostViaLobby => {
            if settings.challenge_mode == ChallengeMode::HostViaLobby {
                host_via_lobby_challenge(settings, network_runtime, network_state)
            } else {
                host_direct_challenge(settings, network_runtime, network_state)
            }
        }
        ChallengeMode::BrowseLobby => {
            if settings.lobby_enabled {
                browse_hosted_lobby(settings, network_runtime, network_state);
            } else {
                network_state.push_message("Lobby server disabled by -X/--no-server");
            }
        }
        ChallengeMode::BrowseLan => start_or_browse_lan(settings, network_runtime, network_state),
        ChallengeMode::ComputerOpponent | ChallengeMode::JoinDirect => {
            network_state.push_message("Update has no action for this mode")
        }
    }
}

pub(in crate::app) fn host_direct_challenge(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    let Ok(bind_addr) =
        parse_network_addr(&settings.direct_listen_addr, "host bind", network_state)
    else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::Host {
            bind_addr,
            identity: direct_identity(settings),
        },
    );
}

pub(in crate::app) fn start_lan_advertising(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if network_state.lan_advertising {
        return;
    }
    let share_addr = effective_direct_share_addr(settings, network_state);
    let Ok(share_addr) = parse_network_addr(&share_addr, "LAN share address", network_state) else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::StartLanAdvertising {
            identity: direct_identity(settings),
            share_addr,
        },
    );
}

pub(in crate::app) fn join_direct_challenge(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    let Ok(peer_addr) =
        parse_network_addr(&settings.direct_join_addr, "join address", network_state)
    else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::Join {
            peer_addr,
            identity: direct_identity(settings),
            challenge_text: "battle?".to_string(),
        },
    );
}

pub(in crate::app) fn host_via_lobby_challenge(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    let Ok(bind_addr) =
        parse_network_addr(&settings.direct_listen_addr, "host bind", network_state)
    else {
        return;
    };
    network_state.lobby_registration = None;
    network_state.hosted_status = None;
    network_state.hosted_start = None;
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::Host {
            bind_addr,
            identity: direct_identity(settings),
        },
    );
}

pub(in crate::app) fn start_sleep_availability(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    network_state.sleep_availability_attempted = true;
    let Ok(bind_addr) =
        parse_network_addr(&settings.direct_listen_addr, "sleep bind", network_state)
    else {
        return;
    };
    if !matches!(network_state.lifecycle, NetworkLifecycleState::Idle) {
        network_state.push_message(format!(
            "Sleep availability not started; network is {:?}",
            network_state.lifecycle
        ));
        return;
    }
    network_state.lobby_registration = None;
    network_state.hosted_status = None;
    network_state.hosted_start = None;
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::Host {
            bind_addr,
            identity: direct_identity(settings),
        },
    );
}

pub(in crate::app) fn register_hosted_lobby(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if !settings.lobby_enabled {
        return;
    }
    let Ok(server_addr) = parse_network_addr(
        &settings.modern_server_addr,
        "modern server address",
        network_state,
    ) else {
        return;
    };
    let direct_addr = effective_direct_share_addr(settings, network_state);
    network_state.lobby_server_addr = Some(server_addr);
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::RegisterLobby {
            server_addr,
            request: LobbyRegister {
                player: hosted_player(settings),
                direct_addr,
                ranked: settings.hosted_ranked,
            },
        },
    );
}

pub(in crate::app) fn browse_hosted_lobby(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if !settings.lobby_enabled {
        network_state.push_message("Lobby server disabled by -X/--no-server");
        return;
    }
    let Ok(server_addr) = parse_network_addr(
        &settings.modern_server_addr,
        "modern server address",
        network_state,
    ) else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::BrowseLobby {
            server_addr,
            ranked_only: false,
        },
    );
}

pub(in crate::app) fn browse_lan(
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    try_send_network_command(network_runtime, network_state, NetworkCommand::BrowseLan);
}

pub(in crate::app) fn start_or_browse_lan(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    let Some(entry) = selected_lan_entry(network_state).cloned() else {
        browse_lan(network_runtime, network_state);
        return;
    };
    if !entry.compatible {
        network_state.push_message(format!(
            "Selected LAN host uses incompatible protocol v{}.{}",
            entry.protocol_major, entry.protocol_minor
        ));
        return;
    }
    if entry.availability != LanAvailability::Available {
        network_state.push_message(format!(
            "Selected LAN host is not available: {:?}",
            entry.availability
        ));
        return;
    }
    let Some(peer_addr) = entry.addr else {
        network_state.push_message("Selected LAN host did not publish a usable address");
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::Join {
            peer_addr,
            identity: direct_identity(settings),
            challenge_text: "battle?".to_string(),
        },
    );
}

pub(in crate::app) fn start_or_browse_hosted_lobby(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if selected_lobby_entry(network_state).is_none() {
        browse_hosted_lobby(settings, network_runtime, network_state);
        return;
    }
    start_selected_hosted_game(settings, network_runtime, network_state);
}

pub(in crate::app) fn start_selected_hosted_game(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if !settings.lobby_enabled {
        network_state.push_message("Lobby server disabled by -X/--no-server");
        return;
    }
    let Some(entry) = selected_lobby_entry(network_state).cloned() else {
        network_state.push_message("No lobby opponent selected");
        return;
    };
    if entry.protocol_major != PROTOCOL_MAJOR {
        network_state.push_message(format!(
            "Selected lobby entry uses incompatible protocol v{}.{}",
            entry.protocol_major, entry.protocol_minor
        ));
        return;
    }
    if entry.host.player_id == hosted_player_id(settings) {
        network_state.push_message("Cannot challenge your own lobby entry");
        return;
    }
    let Ok(server_addr) = parse_network_addr(
        &settings.modern_server_addr,
        "modern server address",
        network_state,
    ) else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::StartHostedGame {
            server_addr,
            session_id: entry.session_id,
            joiner: hosted_player(settings),
        },
    );
}

pub(in crate::app) fn accept_pending_direct_challenge(
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if let Some(challenge) = &network_state.pending_challenge {
        if challenge.hosted_session_id.is_some() || challenge.hosted_player_id.is_some() {
            accept_pending_hosted_challenge(network_runtime, network_state);
            return;
        }
    }
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::Accept {
            seed: direct_accept_seed(),
            ranked: false,
        },
    );
}

pub(in crate::app) fn accept_pending_hosted_challenge(
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    let Some(start) = hosted_start_for_accept(network_state) else {
        network_state.push_message("Hosted challenge does not match server-owned session status");
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::AcceptHosted {
            hosted_start: start,
        },
    );
}

pub(in crate::app) fn poll_registered_hosted_status(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    if !settings.lobby_enabled {
        return;
    }
    let Some(entry) = network_state.lobby_registration.clone() else {
        return;
    };
    let server_addr = network_state
        .lobby_server_addr
        .or_else(|| settings.modern_server_addr.parse::<SocketAddr>().ok());
    let Some(server_addr) = server_addr else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::PollHostedStatus {
            server_addr,
            session_id: entry.session_id,
            requester_player_id: entry.host.player_id,
        },
    );
}

pub(in crate::app) fn join_hosted_direct_after_start(
    settings: &ClientSettings,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    start: HostedGameStart,
) {
    let Some(entry) = lobby_entry_for_session(network_state, &start.session_id).cloned() else {
        network_state
            .push_message("Hosted start returned for an entry no longer in the lobby list");
        return;
    };
    let Ok(peer_addr) =
        parse_network_addr(&entry.direct_addr, "hosted direct address", network_state)
    else {
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::JoinHostedDirect {
            peer_addr,
            identity: direct_identity(settings),
            challenge_text: "hosted battle?".to_string(),
            hosted_session_id: start.session_id.clone(),
            hosted_player_id: hosted_player_id(settings),
            hosted_start: start,
        },
    );
}

pub(in crate::app) fn hosted_start_for_accept(
    network_state: &ClientNetworkState,
) -> Option<HostedGameStart> {
    let challenge = network_state.pending_challenge.as_ref()?;
    let start = network_state.hosted_start.clone().or_else(|| {
        let status = network_state.hosted_status.as_ref()?;
        match &status.status {
            HostedSessionStatusKind::Started(start) => Some(start.clone()),
            _ => None,
        }
    })?;
    if challenge.hosted_session_id.as_ref() != Some(&start.session_id) {
        return None;
    }
    if challenge.hosted_player_id.as_deref() != Some(start.player_two.player_id.as_str()) {
        return None;
    }
    Some(start)
}

pub(in crate::app) fn selected_lobby_entry(
    network_state: &ClientNetworkState,
) -> Option<&LobbyEntry> {
    network_state
        .lobby_list
        .as_ref()?
        .entries
        .get(network_state.lobby_selected_index)
}

pub(in crate::app) fn selected_lan_entry(
    network_state: &ClientNetworkState,
) -> Option<&LanDiscoveryEntry> {
    network_state
        .lan_entries
        .get(network_state.lan_selected_index)
}

pub(in crate::app) fn select_challenge_entry(
    network_state: &mut ClientNetworkState,
    mode: ChallengeMode,
    direction: isize,
) {
    match mode {
        ChallengeMode::BrowseLobby => select_lobby_entry(network_state, direction),
        ChallengeMode::BrowseLan => select_lan_entry(network_state, direction),
        _ => {}
    }
}

pub(in crate::app) fn lobby_entry_for_session<'a>(
    network_state: &'a ClientNetworkState,
    session_id: &battletris_protocol::HostedSessionId,
) -> Option<&'a LobbyEntry> {
    network_state
        .lobby_list
        .as_ref()?
        .entries
        .iter()
        .find(|entry| &entry.session_id == session_id)
}

pub(in crate::app) fn select_lobby_entry(network_state: &mut ClientNetworkState, direction: isize) {
    let Some(list) = &network_state.lobby_list else {
        return;
    };
    if list.entries.is_empty() {
        network_state.lobby_selected_index = 0;
        return;
    }
    let len = list.entries.len() as isize;
    let next = (network_state.lobby_selected_index as isize + direction).rem_euclid(len);
    network_state.lobby_selected_index = next as usize;
}

pub(in crate::app) fn select_lan_entry(network_state: &mut ClientNetworkState, direction: isize) {
    if network_state.lan_entries.is_empty() {
        network_state.lan_selected_index = 0;
        return;
    }
    let len = network_state.lan_entries.len() as isize;
    let next = (network_state.lan_selected_index as isize + direction).rem_euclid(len);
    network_state.lan_selected_index = next as usize;
}

pub(in crate::app) fn select_challenge_entry_at_world(
    world: Vec2,
    mode: ChallengeMode,
    network_state: &mut ClientNetworkState,
) -> bool {
    match mode {
        ChallengeMode::BrowseLobby => select_lobby_entry_at_world(world, network_state),
        ChallengeMode::BrowseLan => select_lan_entry_at_world(world, network_state),
        _ => false,
    }
}

pub(in crate::app) fn select_lobby_entry_at_world(
    world: Vec2,
    network_state: &mut ClientNetworkState,
) -> bool {
    let Some(list) = &network_state.lobby_list else {
        return false;
    };
    if list.entries.is_empty() {
        return false;
    }
    let screen_x = (world.x + 320.0) / 0.8;
    let screen_y = (300.0 - world.y) * 7.0 / 6.0;
    if !(38.0..=382.0).contains(&screen_x) || !(44.0..=470.0).contains(&screen_y) {
        return false;
    }
    let row = ((screen_y - 92.0) / 32.0).floor() as isize;
    if row < 0 {
        return false;
    }
    let index = row as usize;
    if index >= list.entries.len().min(8) {
        return false;
    }
    network_state.lobby_selected_index = index;
    true
}

pub(in crate::app) fn select_lan_entry_at_world(
    world: Vec2,
    network_state: &mut ClientNetworkState,
) -> bool {
    if network_state.lan_entries.is_empty() {
        return false;
    }
    let Some(index) = challenge_entry_index_at_world(world) else {
        return false;
    };
    if index >= network_state.lan_entries.len().min(8) {
        return false;
    }
    network_state.lan_selected_index = index;
    true
}

pub(in crate::app) fn challenge_entry_index_at_world(world: Vec2) -> Option<usize> {
    let screen_x = (world.x + 320.0) / 0.8;
    let screen_y = (300.0 - world.y) * 7.0 / 6.0;
    if !(38.0..=382.0).contains(&screen_x) || !(44.0..=470.0).contains(&screen_y) {
        return None;
    }
    let row = ((screen_y - 92.0) / 32.0).floor() as isize;
    (row >= 0).then_some(row as usize)
}

pub(in crate::app) fn deny_pending_direct_challenge(
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::Deny {
            reason: "Challenge denied by host".to_string(),
        },
    );
}

pub(in crate::app) fn cancel_network_challenge(
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    cancel_hosted_registration(network_runtime, network_state);
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::StopLanAdvertising,
    );
    if network_operation_can_cancel(&network_state.lifecycle) {
        try_send_network_command(network_runtime, network_state, NetworkCommand::Cancel);
    }
}

pub(in crate::app) fn cancel_hosted_registration(
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
) {
    let Some(entry) = network_state.lobby_registration.clone() else {
        return;
    };
    let Some(server_addr) = network_state.lobby_server_addr else {
        network_state.lobby_registration = None;
        return;
    };
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::CancelHostedSession {
            server_addr,
            session_id: entry.session_id,
            requester_player_id: entry.host.player_id,
        },
    );
}

pub(in crate::app) fn leave_network_game(
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    reason: &str,
) {
    if local.is_networked() {
        try_send_network_command(
            network_runtime,
            network_state,
            NetworkCommand::Disconnect {
                reason: reason.to_string(),
            },
        );
        local.network_session = None;
        local.network_lockstep = None;
        local.network_failed_closed = true;
        local.status_message = Some(reason.to_string());
    }
}

pub(in crate::app) fn network_operation_can_cancel(lifecycle: &NetworkLifecycleState) -> bool {
    matches!(
        lifecycle,
        NetworkLifecycleState::Hosting { .. }
            | NetworkLifecycleState::Joining { .. }
            | NetworkLifecycleState::Challenged { .. }
            | NetworkLifecycleState::Error { .. }
    )
}

pub(in crate::app) fn parse_network_addr(
    value: &str,
    label: &str,
    network_state: &mut ClientNetworkState,
) -> Result<SocketAddr, ()> {
    value.parse::<SocketAddr>().map_err(|error| {
        let message = format!("Invalid {label} '{value}': {error}");
        network_state.last_error = Some(message.clone());
        network_state.push_message(message);
    })
}

pub(in crate::app) fn direct_identity(settings: &ClientSettings) -> PlayerIdentity {
    PlayerIdentity {
        display_name: settings.display_name.clone(),
    }
}

pub(in crate::app) fn direct_accept_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x00B4_771E_7415)
}

pub(in crate::app) fn handle_settings_input(
    keys: &ButtonInput<KeyCode>,
    settings: &mut ClientSettings,
    edit: &mut SettingsEditState,
    sound: &mut SoundEventState,
) {
    let previous = settings.persisted();

    if keys.just_pressed(KeyCode::Tab) {
        let backwards = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        edit.focus(next_settings_control(edit.control, backwards));
        queue_sound(sound, SoundEvent::MenuAction);
    }

    if let Some(field) = edit.control.text_field() {
        if keys.just_pressed(KeyCode::Backspace) || keys.just_pressed(KeyCode::Delete) {
            settings_field_value_mut(settings, field).pop();
            queue_sound(sound, SoundEvent::MenuAction);
        }
        if keys.just_pressed(KeyCode::Enter) {
            sanitize_settings_after_edit(settings, field);
            queue_sound(sound, SoundEvent::MenuAction);
        }
        if let Some(ch) = text_entry_character(keys) {
            settings_field_value_mut(settings, field).push(ch);
            queue_sound(sound, SoundEvent::MenuAction);
        }
    } else if handle_focused_settings_control(keys, settings, edit.control)
        || handle_settings_shortcut(keys, settings)
    {
        queue_sound(sound, SoundEvent::MenuAction);
    }

    if settings.persisted() != previous {
        settings.save();
    }
}

pub(in crate::app) fn handle_focused_settings_control(
    keys: &ButtonInput<KeyCode>,
    settings: &mut ClientSettings,
    control: SettingsControl,
) -> bool {
    let activate = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::ArrowLeft)
        || keys.just_pressed(KeyCode::ArrowRight);
    match control {
        SettingsControl::UiStyle
        | SettingsControl::Theme
        | SettingsControl::SoundPack
        | SettingsControl::ChallengeStyle
        | SettingsControl::HostedRanked
            if activate =>
        {
            activate_settings_control(settings, control)
        }
        SettingsControl::PixelScale
            if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::Minus) =>
        {
            adjust_settings_pixel_scale(settings, -0.25)
        }
        SettingsControl::PixelScale
            if keys.just_pressed(KeyCode::ArrowRight)
                || keys.just_pressed(KeyCode::Equal)
                || keys.just_pressed(KeyCode::Enter)
                || keys.just_pressed(KeyCode::Space) =>
        {
            adjust_settings_pixel_scale(settings, 0.25)
        }
        SettingsControl::UiStyle
        | SettingsControl::Theme
        | SettingsControl::SoundPack
        | SettingsControl::ChallengeStyle
        | SettingsControl::HostedRanked
        | SettingsControl::PixelScale
        | SettingsControl::Text(_) => false,
    }
}

pub(in crate::app) fn handle_settings_shortcut(
    keys: &ButtonInput<KeyCode>,
    settings: &mut ClientSettings,
) -> bool {
    if keys.just_pressed(KeyCode::KeyU) {
        activate_settings_control(settings, SettingsControl::UiStyle)
    } else if keys.just_pressed(KeyCode::KeyT) {
        activate_settings_control(settings, SettingsControl::Theme)
    } else if keys.just_pressed(KeyCode::KeyO) {
        activate_settings_control(settings, SettingsControl::SoundPack)
    } else if keys.just_pressed(KeyCode::KeyM) {
        activate_settings_control(settings, SettingsControl::ChallengeStyle)
    } else if keys.just_pressed(KeyCode::KeyR) {
        activate_settings_control(settings, SettingsControl::HostedRanked)
    } else if keys.just_pressed(KeyCode::Equal) {
        adjust_settings_pixel_scale(settings, 0.25)
    } else if keys.just_pressed(KeyCode::Minus) {
        adjust_settings_pixel_scale(settings, -0.25)
    } else {
        false
    }
}

pub(in crate::app) fn activate_settings_control(
    settings: &mut ClientSettings,
    control: SettingsControl,
) -> bool {
    match control {
        SettingsControl::UiStyle => {
            settings.ui_style = settings.ui_style.toggled();
            true
        }
        SettingsControl::Theme => {
            toggle_theme(settings);
            true
        }
        SettingsControl::SoundPack => {
            settings.sound_pack = settings.sound_pack.toggled();
            true
        }
        SettingsControl::ChallengeStyle => {
            settings.challenge_style = settings.challenge_style.toggled();
            true
        }
        SettingsControl::HostedRanked => {
            settings.hosted_ranked = !settings.hosted_ranked;
            true
        }
        SettingsControl::PixelScale | SettingsControl::Text(_) => false,
    }
}

pub(in crate::app) fn apply_settings_select_option(
    settings: &mut ClientSettings,
    option: SettingsSelectOption,
) -> bool {
    match option {
        SettingsSelectOption::UiStyle(ui_style) => {
            let changed = settings.ui_style != ui_style;
            settings.ui_style = ui_style;
            changed
        }
        SettingsSelectOption::Theme(theme) => {
            let changed = settings.theme != theme;
            settings.theme = theme;
            changed
        }
        SettingsSelectOption::SoundPack(sound_pack) => {
            let changed = settings.sound_pack != sound_pack;
            settings.sound_pack = sound_pack;
            changed
        }
        SettingsSelectOption::ChallengeStyle(challenge_style) => {
            let changed = settings.challenge_style != challenge_style;
            settings.challenge_style = challenge_style;
            changed
        }
    }
}

pub(in crate::app) fn adjust_settings_pixel_scale(
    settings: &mut ClientSettings,
    delta: f32,
) -> bool {
    let previous = settings.pixel_scale;
    settings.pixel_scale = sanitize_pixel_scale(settings.pixel_scale + delta).clamp(0.75, 2.0);
    settings.pixel_scale != previous
}

pub(in crate::app) fn next_settings_control(
    control: SettingsControl,
    backwards: bool,
) -> SettingsControl {
    let index = SettingsControl::ALL
        .iter()
        .position(|candidate| *candidate == control)
        .unwrap_or_default();
    let len = SettingsControl::ALL.len();
    let next_index = if backwards {
        (index + len - 1) % len
    } else {
        (index + 1) % len
    };
    SettingsControl::ALL[next_index]
}

pub(in crate::app) fn text_entry_character(keys: &ButtonInput<KeyCode>) -> Option<char> {
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for (key, ch) in text_entry_keys(shifted) {
        if keys.just_pressed(key) {
            return Some(ch);
        }
    }
    None
}

pub(in crate::app) fn text_entry_keys(shifted: bool) -> [(KeyCode, char); 44] {
    [
        (KeyCode::KeyA, if shifted { 'A' } else { 'a' }),
        (KeyCode::KeyB, if shifted { 'B' } else { 'b' }),
        (KeyCode::KeyC, if shifted { 'C' } else { 'c' }),
        (KeyCode::KeyD, if shifted { 'D' } else { 'd' }),
        (KeyCode::KeyE, if shifted { 'E' } else { 'e' }),
        (KeyCode::KeyF, if shifted { 'F' } else { 'f' }),
        (KeyCode::KeyG, if shifted { 'G' } else { 'g' }),
        (KeyCode::KeyH, if shifted { 'H' } else { 'h' }),
        (KeyCode::KeyI, if shifted { 'I' } else { 'i' }),
        (KeyCode::KeyJ, if shifted { 'J' } else { 'j' }),
        (KeyCode::KeyK, if shifted { 'K' } else { 'k' }),
        (KeyCode::KeyL, if shifted { 'L' } else { 'l' }),
        (KeyCode::KeyM, if shifted { 'M' } else { 'm' }),
        (KeyCode::KeyN, if shifted { 'N' } else { 'n' }),
        (KeyCode::KeyO, if shifted { 'O' } else { 'o' }),
        (KeyCode::KeyP, if shifted { 'P' } else { 'p' }),
        (KeyCode::KeyQ, if shifted { 'Q' } else { 'q' }),
        (KeyCode::KeyR, if shifted { 'R' } else { 'r' }),
        (KeyCode::KeyS, if shifted { 'S' } else { 's' }),
        (KeyCode::KeyT, if shifted { 'T' } else { 't' }),
        (KeyCode::KeyU, if shifted { 'U' } else { 'u' }),
        (KeyCode::KeyV, if shifted { 'V' } else { 'v' }),
        (KeyCode::KeyW, if shifted { 'W' } else { 'w' }),
        (KeyCode::KeyX, if shifted { 'X' } else { 'x' }),
        (KeyCode::KeyY, if shifted { 'Y' } else { 'y' }),
        (KeyCode::KeyZ, if shifted { 'Z' } else { 'z' }),
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Period, '.'),
        (KeyCode::Comma, ','),
        (KeyCode::Minus, '-'),
        (KeyCode::Equal, '='),
        (KeyCode::Slash, '/'),
        (KeyCode::Semicolon, if shifted { ':' } else { ';' }),
        (KeyCode::Space, ' '),
        (KeyCode::Backslash, '\\'),
    ]
}

pub(in crate::app) struct GameInputContext<'a> {
    pub(in crate::app) keys: &'a ButtonInput<KeyCode>,
    pub(in crate::app) local: &'a mut LocalGame,
    pub(in crate::app) network_runtime: &'a mut ClientNetworkRuntime,
    pub(in crate::app) network_state: &'a mut ClientNetworkState,
    pub(in crate::app) settings: &'a ClientSettings,
    pub(in crate::app) repeat: &'a mut InputRepeatState,
    pub(in crate::app) recon: &'a mut ReconPanel,
    pub(in crate::app) bazaar_ui: &'a mut BazaarUiState,
}

pub(in crate::app) fn handle_game_input(ctx: GameInputContext<'_>, elapsed_ms: u64) {
    if ctx.keys.just_pressed(KeyCode::KeyR) {
        if ctx.local.is_networked() {
            ctx.local.status_message =
                Some("Restart is unavailable during online play.".to_string());
            return;
        }
        ctx.local.restart();
        return;
    }

    if ctx.keys.just_pressed(KeyCode::KeyP) {
        if ctx.local.is_networked() {
            ctx.local.status_message = Some("Pause is unavailable during online play.".to_string());
            return;
        }
        if ctx.local.game.phase() == GamePhase::Paused {
            let _ = ctx.local.game.resume();
        } else {
            let _ = ctx.local.game.pause();
        }
    }

    if ctx.keys.just_pressed(KeyCode::KeyQ) {
        ctx.local.status_message =
            Some("BattleTris is owned and operated by the legacy crew.".to_string());
    }

    if ctx.keys.just_pressed(KeyCode::KeyC) && ctx.local.computer.is_some() {
        ctx.recon.manual_condor = !ctx.recon.manual_condor;
        if !ctx.recon.manual_condor {
            ctx.recon.snapshot = None;
        }
    }

    if ctx.local.game.phase() == GamePhase::Bazaar {
        handle_bazaar_input(
            ctx.keys,
            ctx.local,
            ctx.network_runtime,
            ctx.network_state,
            ctx.bazaar_ui,
            ctx.settings.content_mode,
        );
        flush_legacy_outbound_events(ctx.local, ctx.network_runtime, ctx.network_state);
        return;
    }

    if ctx.local.game.phase() != GamePhase::Playing {
        return;
    }

    if ctx.local.is_lockstep_networked() {
        send_network_player_controls(
            ctx.keys,
            ctx.local,
            ctx.network_runtime,
            ctx.network_state,
            ctx.settings.controls,
            ctx.repeat,
            elapsed_ms,
        );
        for (label, key) in slot_keys() {
            if ctx.keys.just_pressed(key) {
                schedule_network_input(
                    ctx.local,
                    ctx.network_runtime,
                    ctx.network_state,
                    InputCommand::LaunchWeapon {
                        slot_index: slot_label_to_index(label),
                    },
                );
            }
        }
        return;
    }

    let controlled_players: &[PlayerId] = if ctx.local.is_legacy_networked() {
        std::slice::from_ref(&ctx.local.local_player)
    } else {
        &[PlayerId::One, PlayerId::Two]
    };
    for &player in controlled_players {
        if ctx
            .local
            .computer
            .as_ref()
            .is_some_and(|computer| computer.player == player)
        {
            continue;
        }
        send_player_controls(
            ctx.keys,
            &mut ctx.local.game,
            player,
            ctx.settings.controls,
            ctx.repeat,
            elapsed_ms,
        );
    }

    for (label, key) in slot_keys() {
        if ctx.keys.just_pressed(key) {
            let player = if ctx.local.is_legacy_networked() {
                ctx.local.local_player
            } else if ctx.keys.pressed(KeyCode::ShiftLeft) || ctx.keys.pressed(KeyCode::ShiftRight)
            {
                PlayerId::Two
            } else {
                PlayerId::One
            };
            if ctx
                .local
                .computer
                .as_ref()
                .is_none_or(|computer| computer.player != player)
            {
                let _ = ctx.local.game.launch_weapon_slot(player, label);
            }
        }
    }
    flush_legacy_outbound_events(ctx.local, ctx.network_runtime, ctx.network_state);
}

pub(in crate::app) fn send_player_controls(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    player: PlayerId,
    scheme: ControlScheme,
    repeat: &mut InputRepeatState,
    elapsed_ms: u64,
) {
    let controls = controls_for(player, scheme);
    send_repeat_command(
        keys,
        game,
        player,
        controls.left,
        Command::MoveLeft,
        &mut repeat.left[client_player_index(player)],
        elapsed_ms,
    );
    send_repeat_command(
        keys,
        game,
        player,
        controls.right,
        Command::MoveRight,
        &mut repeat.right[client_player_index(player)],
        elapsed_ms,
    );
    send_press_command(
        keys,
        game,
        player,
        controls.rotate_cw,
        Command::RotateClockwise,
    );
    send_press_command(
        keys,
        game,
        player,
        controls.rotate_ccw,
        Command::RotateCounterClockwise,
    );
    send_fast_drop(keys, game, player, controls.fast_drop);
}

pub(in crate::app) fn send_network_player_controls(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    scheme: ControlScheme,
    repeat: &mut InputRepeatState,
    elapsed_ms: u64,
) {
    let player = local.local_player;
    let controls = controls_for(player, scheme);
    send_network_repeat_command(
        keys,
        local,
        network_runtime,
        network_state,
        (controls.left, InputCommand::MoveLeft),
        &mut repeat.left[client_player_index(player)],
        elapsed_ms,
    );
    send_network_repeat_command(
        keys,
        local,
        network_runtime,
        network_state,
        (controls.right, InputCommand::MoveRight),
        &mut repeat.right[client_player_index(player)],
        elapsed_ms,
    );
    send_network_press_command(
        keys,
        local,
        network_runtime,
        network_state,
        controls.rotate_cw,
        InputCommand::RotateClockwise,
    );
    send_network_press_command(
        keys,
        local,
        network_runtime,
        network_state,
        controls.rotate_ccw,
        InputCommand::RotateCounterClockwise,
    );
    send_network_fast_drop(
        keys,
        local,
        network_runtime,
        network_state,
        controls.fast_drop,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct PlayerControls {
    pub(in crate::app) left: KeyCode,
    pub(in crate::app) right: KeyCode,
    pub(in crate::app) rotate_cw: KeyCode,
    pub(in crate::app) rotate_ccw: KeyCode,
    pub(in crate::app) fast_drop: KeyCode,
}

pub(in crate::app) fn controls_for(_player: PlayerId, _scheme: ControlScheme) -> PlayerControls {
    PlayerControls {
        left: KeyCode::KeyJ,
        right: KeyCode::KeyL,
        rotate_cw: KeyCode::KeyK,
        rotate_ccw: KeyCode::KeyI,
        fast_drop: KeyCode::Space,
    }
}

pub(in crate::app) fn handle_bazaar_input(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    bazaar_ui: &mut BazaarUiState,
    content_mode: ContentMode,
) {
    let is_networked = local.is_networked();
    let is_lockstep_networked = local.is_lockstep_networked();
    let primary_player = if is_networked {
        local.local_player
    } else {
        PlayerId::One
    };
    if keys.just_pressed(KeyCode::Enter) {
        let events = if is_lockstep_networked {
            send_network_bazaar_done(local, network_runtime, network_state, bazaar_ui)
        } else {
            local.game.bazaar_done(primary_player)
        };
        match events {
            events if events.is_empty() => {
                bazaar_ui.last_message = UiTextTone::bazaar_waiting_copy(
                    content_mode,
                    BazaarWaitingText::PlayerRepeated(primary_player),
                )
            }
            _ => {
                bazaar_ui.last_message = UiTextTone::bazaar_waiting_copy(
                    content_mode,
                    BazaarWaitingText::PlayerWaiting(primary_player),
                )
            }
        }
    }
    if keys.just_pressed(KeyCode::Space) && !is_networked {
        match local.game.bazaar_done(PlayerId::Two) {
            events if events.is_empty() => {
                bazaar_ui.last_message = UiTextTone::bazaar_waiting_copy(
                    content_mode,
                    BazaarWaitingText::PlayerRepeated(PlayerId::Two),
                )
            }
            _ => {
                bazaar_ui.last_message = UiTextTone::bazaar_waiting_copy(
                    content_mode,
                    BazaarWaitingText::PlayerWaiting(PlayerId::Two),
                )
            }
        }
    }

    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        bazaar_ui.selected = adjacent_catalog_token(bazaar_ui.selected, -1);
    }
    if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        bazaar_ui.selected = adjacent_catalog_token(bazaar_ui.selected, 1);
    }
    if keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::Equal) {
        buy_selected_bazaar_weapon(
            local,
            network_runtime,
            network_state,
            bazaar_ui,
            primary_player,
        );
    }
    if keys.just_pressed(KeyCode::KeyX) || keys.just_pressed(KeyCode::Minus) {
        remove_selected_bazaar_weapon(
            local,
            network_runtime,
            network_state,
            bazaar_ui,
            primary_player,
        );
    }

    for (token, key) in bazaar_catalog_keys() {
        if keys.just_pressed(key) {
            let player = if local.is_legacy_networked() {
                local.local_player
            } else if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                PlayerId::Two
            } else {
                PlayerId::One
            };
            bazaar_ui.selected = token;
            if is_lockstep_networked {
                buy_bazaar_weapon(
                    local,
                    network_runtime,
                    network_state,
                    bazaar_ui,
                    local.local_player,
                    token,
                );
            } else {
                buy_bazaar_weapon(
                    local,
                    network_runtime,
                    network_state,
                    bazaar_ui,
                    player,
                    token,
                );
            }
        }
    }
}

pub(in crate::app) fn send_press_command(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    player: PlayerId,
    key: KeyCode,
    command: Command,
) {
    if keys.just_pressed(key) {
        let _ = game.command(player, command);
    }
}

pub(in crate::app) fn send_network_press_command(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    key: KeyCode,
    command: InputCommand,
) {
    if keys.just_pressed(key) {
        schedule_network_input(local, network_runtime, network_state, command);
    }
}

pub(in crate::app) fn send_repeat_command(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    player: PlayerId,
    key: KeyCode,
    command: Command,
    repeat: &mut HeldKeyRepeat,
    elapsed_ms: u64,
) {
    let (next, emit) = repeat.observe(keys.pressed(key), keys.just_pressed(key), elapsed_ms);
    *repeat = next;
    if emit {
        let _ = game.command(player, command);
    }
}

pub(in crate::app) fn send_network_repeat_command(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    binding: (KeyCode, InputCommand),
    repeat: &mut HeldKeyRepeat,
    elapsed_ms: u64,
) {
    let (key, command) = binding;
    let (next, emit) = repeat.observe(keys.pressed(key), keys.just_pressed(key), elapsed_ms);
    *repeat = next;
    if emit {
        schedule_network_input(local, network_runtime, network_state, command);
    }
}

pub(in crate::app) fn send_fast_drop(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    player: PlayerId,
    key: KeyCode,
) {
    let command = if keys.pressed(key) {
        Command::StartFastDrop
    } else {
        Command::StopFastDrop
    };
    let _ = game.command(player, command);
}

pub(in crate::app) fn send_network_fast_drop(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    key: KeyCode,
) {
    if keys.just_pressed(key) {
        schedule_network_input(
            local,
            network_runtime,
            network_state,
            InputCommand::StartFastDrop,
        );
    }
    if keys.just_released(key) {
        schedule_network_input(
            local,
            network_runtime,
            network_state,
            InputCommand::StopFastDrop,
        );
    }
}

pub(in crate::app) fn schedule_network_input(
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    command: InputCommand,
) -> bool {
    let Some(lockstep) = local.network_lockstep.as_mut() else {
        return false;
    };
    let input = lockstep.schedule_local_input(command);
    if let Some(session) = local.network_session.as_mut() {
        session.current_tick = lockstep.current_tick();
    }
    try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::SendScheduledInput(input),
    )
}

pub(in crate::app) fn slot_label_to_index(label: u8) -> u8 {
    if label == 0 {
        9
    } else {
        label - 1
    }
}

pub(in crate::app) fn slot_keys() -> [(u8, KeyCode); 10] {
    [
        (1, KeyCode::Digit1),
        (2, KeyCode::Digit2),
        (3, KeyCode::Digit3),
        (4, KeyCode::Digit4),
        (5, KeyCode::Digit5),
        (6, KeyCode::Digit6),
        (7, KeyCode::Digit7),
        (8, KeyCode::Digit8),
        (9, KeyCode::Digit9),
        (0, KeyCode::Digit0),
    ]
}

pub(in crate::app) fn bazaar_catalog_keys() -> [(WeaponToken, KeyCode); 10] {
    [
        (WeaponToken::FlipOut, KeyCode::Digit1),
        (WeaponToken::Gimp, KeyCode::Digit2),
        (WeaponToken::Missing, KeyCode::Digit3),
        (WeaponToken::NiceDay, KeyCode::Digit4),
        (WeaponToken::RiseUp, KeyCode::Digit5),
        (WeaponToken::PieceIt, KeyCode::Digit6),
        (WeaponToken::Ace, KeyCode::Digit7),
        (WeaponToken::SoLong, KeyCode::Digit8),
        (WeaponToken::Upbyside, KeyCode::Digit9),
        (WeaponToken::NoSlide, KeyCode::Digit0),
    ]
}

pub(in crate::app) fn send_network_bazaar_buy(
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    player: PlayerId,
    token: WeaponToken,
) -> Result<usize, String> {
    if player != local.local_player {
        return Err("online Bazaar only accepts local player purchases".to_string());
    }
    let buy = BazaarBuy {
        player: protocol_slot_for_player(player),
        weapon: token.legacy_id(),
        sequence: local.game.event_log().len() as u64,
    };
    let Some(lockstep) = local.network_lockstep.as_ref() else {
        return Err("network lockstep state is missing".to_string());
    };
    lockstep
        .apply_bazaar_buy(&mut local.game, buy.clone())
        .map_err(|error| format!("{error:?}"))?;
    let index = staged_slot_index_for_token(&local.game, player, token).unwrap_or(0);
    if !try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::SendBazaarBuy(buy),
    ) {
        return Err("network send failed".to_string());
    }
    Ok(index)
}

pub(in crate::app) fn send_network_bazaar_remove(
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    player: PlayerId,
    token: WeaponToken,
) -> Result<(), String> {
    if player != local.local_player {
        return Err("online Bazaar only accepts local player removals".to_string());
    }
    let slot_index = staged_slot_index_for_token(&local.game, player, token)
        .ok_or_else(|| "no staged weapon in requested slot".to_string())?;
    let remove = BazaarRemove {
        player: protocol_slot_for_player(player),
        slot_index: slot_index.try_into().expect("Bazaar slot index fits in u8"),
        sequence: local.game.event_log().len() as u64,
    };
    let Some(lockstep) = local.network_lockstep.as_ref() else {
        return Err("network lockstep state is missing".to_string());
    };
    lockstep
        .apply_bazaar_remove(&mut local.game, remove.clone())
        .map_err(|error| format!("{error:?}"))?;
    if !try_send_network_command(
        network_runtime,
        network_state,
        NetworkCommand::SendBazaarRemove(remove),
    ) {
        return Err("network send failed".to_string());
    }
    Ok(())
}

pub(in crate::app) fn send_network_bazaar_done(
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    _ui: &mut BazaarUiState,
) -> Vec<LoggedEvent> {
    let player = local.local_player;
    let done = BazaarDone {
        player: protocol_slot_for_player(player),
    };
    let Some(lockstep) = local.network_lockstep.as_ref() else {
        return Vec::new();
    };
    let events = lockstep.apply_bazaar_done(&mut local.game, done.clone());
    if !events.is_empty() {
        try_send_network_command(
            network_runtime,
            network_state,
            NetworkCommand::SendBazaarDone {
                player: done.player,
            },
        );
        send_network_checksum(local, lockstep, network_runtime, network_state);
    }
    events
}

pub(in crate::app) fn staged_slot_index_for_token(
    game: &TwoPlayerGame,
    player: PlayerId,
    token: WeaponToken,
) -> Option<usize> {
    game.bazaar_session(player).and_then(|session| {
        session
            .staged_arsenal()
            .slots()
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|slot| slot.token == token))
    })
}

pub(in crate::app) const fn protocol_slot_for_player(player: PlayerId) -> PlayerSlot {
    match player {
        PlayerId::One => PlayerSlot::One,
        PlayerId::Two => PlayerSlot::Two,
    }
}

pub(in crate::app) fn handle_bazaar_click(
    world: Vec2,
    theme: &LoadedTheme,
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    ui: &mut BazaarUiState,
    content_mode: ContentMode,
) {
    let player = if local.is_networked() {
        local.local_player
    } else {
        PlayerId::One
    };
    if let Some(token) = bazaar_catalog_token_at(world, theme) {
        ui.selected = token;
        ui.last_message = format!("Selected {}.", weapon_spec(token).name);
        return;
    }
    if bazaar_add_rect(theme).contains(world) {
        buy_selected_bazaar_weapon(local, network_runtime, network_state, ui, player);
        return;
    }
    if bazaar_remove_rect(theme).contains(world) {
        remove_selected_bazaar_weapon(local, network_runtime, network_state, ui, player);
        return;
    }
    if bazaar_done_rect(theme).contains(world) {
        let events = if local.is_lockstep_networked() {
            send_network_bazaar_done(local, network_runtime, network_state, ui)
        } else {
            local.game.bazaar_done(player)
        };
        match events {
            events if events.is_empty() => {
                ui.last_message =
                    UiTextTone::bazaar_waiting_copy(content_mode, BazaarWaitingText::LocalRepeated)
            }
            _ => {
                ui.last_message =
                    UiTextTone::bazaar_waiting_copy(content_mode, BazaarWaitingText::LocalWaiting)
            }
        }
        return;
    }
    if let Some(token) = bazaar_arsenal_token_at(world, theme, &local.game, player) {
        ui.selected = token;
        remove_selected_bazaar_weapon(local, network_runtime, network_state, ui, player);
    }
}

pub(in crate::app) fn buy_selected_bazaar_weapon(
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    ui: &mut BazaarUiState,
    player: PlayerId,
) {
    buy_bazaar_weapon(
        local,
        network_runtime,
        network_state,
        ui,
        player,
        ui.selected,
    );
}

pub(in crate::app) fn buy_bazaar_weapon(
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    ui: &mut BazaarUiState,
    player: PlayerId,
    token: WeaponToken,
) {
    let result = if local.is_lockstep_networked() {
        send_network_bazaar_buy(local, network_runtime, network_state, player, token)
    } else {
        local
            .game
            .bazaar_buy(player, token)
            .map_err(|error| format!("{error:?}"))
    };
    match result {
        Ok(index) => {
            ui.last_message = format!(
                "Added {} to slot {}.",
                weapon_spec(token).name,
                arsenal_slot_label(index),
            );
        }
        Err(error) => {
            ui.last_message = format!("Could not add {}: {error}.", weapon_spec(token).name);
        }
    }
}

pub(in crate::app) fn remove_selected_bazaar_weapon(
    local: &mut LocalGame,
    network_runtime: &mut ClientNetworkRuntime,
    network_state: &mut ClientNetworkState,
    ui: &mut BazaarUiState,
    player: PlayerId,
) {
    let token = ui.selected;
    let result = if local.is_lockstep_networked() {
        send_network_bazaar_remove(local, network_runtime, network_state, player, token)
    } else {
        local
            .game
            .bazaar_remove_staged(player, token)
            .map_err(|error| format!("{error:?}"))
    };
    match result {
        Ok(()) => {
            ui.last_message = format!(
                "Removed staged {} and refunded its entry price.",
                weapon_spec(token).name
            );
        }
        Err(error) => {
            ui.last_message = format!(
                "Could not remove {}: only newly staged purchases can be refunded ({error}).",
                weapon_spec(token).name,
            );
        }
    }
}

pub(in crate::app) fn adjacent_catalog_token(current: WeaponToken, step: isize) -> WeaponToken {
    let rows = sorted_weapon_catalog();
    let index = rows
        .iter()
        .position(|spec| spec.token == current)
        .unwrap_or_default() as isize;
    let next = (index + step).rem_euclid(rows.len() as isize) as usize;
    rows[next].token
}

pub(in crate::app) fn bazaar_catalog_token_at(
    world: Vec2,
    theme: &LoadedTheme,
) -> Option<WeaponToken> {
    let rows = sorted_weapon_catalog();
    let rect = theme.layout.rects.bazaar_catalog.rect();
    if !rect.contains(world) {
        return None;
    }
    let row_height = rect.height() / rows.len() as f32;
    let row = ((rect.max.y - world.y) / row_height).floor() as usize;
    rows.get(row).map(|spec| spec.token)
}

pub(in crate::app) fn bazaar_arsenal_token_at(
    world: Vec2,
    theme: &LoadedTheme,
    game: &TwoPlayerGame,
    player: PlayerId,
) -> Option<WeaponToken> {
    let rect = theme.layout.rects.bazaar_arsenal.rect();
    if !rect.contains(world) {
        return None;
    }
    let slot_width = rect.width() / 10.0;
    let index = ((world.x - rect.min.x) / slot_width).floor() as usize;
    game.bazaar_session(player)
        .and_then(|bazaar| bazaar.staged_arsenal().slots().get(index))
        .copied()
        .flatten()
        .map(|slot| slot.token)
}

pub(in crate::app) fn bazaar_add_rect(theme: &LoadedTheme) -> Rect {
    theme.layout.rects.bazaar_add.rect()
}

pub(in crate::app) fn bazaar_remove_rect(theme: &LoadedTheme) -> Rect {
    theme.layout.rects.bazaar_remove.rect()
}

pub(in crate::app) fn bazaar_done_rect(theme: &LoadedTheme) -> Rect {
    theme.layout.rects.bazaar_done.rect()
}

pub(in crate::app) fn arsenal_slot_label(index: usize) -> String {
    if index == 9 {
        "0".to_string()
    } else {
        (index + 1).to_string()
    }
}
