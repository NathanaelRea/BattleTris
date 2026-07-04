//! UI rendering systems, visibility, window layout, and board sprite mapping.

use super::*;

pub(in crate::app) type HudTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static HudText, &'static mut Text2d),
    (Without<PhaseText>, Without<MenuText>, Without<RosterText>),
>;

pub(in crate::app) type PhaseTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text2d,
    (
        With<PhaseText>,
        Without<HudText>,
        Without<MenuText>,
        Without<RosterText>,
    ),
>;

pub(in crate::app) type MenuTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text2d,
    (
        With<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<RosterText>,
    ),
>;

pub(in crate::app) type ScreenTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text2d,
    (
        With<ScreenText>,
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<RosterText>,
    ),
>;

pub(in crate::app) type BazaarTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static BazaarText, &'static mut Text2d),
    (
        With<BazaarText>,
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<RosterText>,
    ),
>;

pub(in crate::app) type LegacyGameTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static LegacyGameText, &'static mut Text2d),
    (
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<BazaarText>,
        Without<ChallengeText>,
        Without<MenuButton>,
        Without<RosterText>,
    ),
>;

pub(in crate::app) type ChallengeTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static ChallengeText, &'static mut Text2d),
    (
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<BazaarText>,
        Without<MenuButton>,
        Without<RosterText>,
    ),
>;

pub(in crate::app) type RosterTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static RosterText, &'static mut Text2d),
    (
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<BazaarText>,
        Without<LegacyGameText>,
        Without<ChallengeText>,
        Without<MenuButton>,
    ),
>;

pub(in crate::app) type MenuButtonTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static MenuButton, &'static mut Text2d),
    (
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<BazaarText>,
        Without<ChallengeText>,
        Without<RosterText>,
    ),
>;

pub(in crate::app) type ChallengeSliderKnobQuery<'w, 's> =
    Query<'w, 's, (&'static ChallengeSliderKnob, &'static mut Transform), Without<Text2d>>;

pub(in crate::app) type BazaarSelectionMarkerQuery<'w, 's> = Query<
    'w,
    's,
    (&'static BazaarSelectionMarker, &'static mut Transform),
    Without<ChallengeSliderKnob>,
>;

pub(in crate::app) type TextMetricsQuery<'w, 's> =
    Query<'w, 's, (&'static mut LineHeight, &'static mut LetterSpacing), With<ThemedTextMetrics>>;

pub(in crate::app) type ShellVisibilityQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        Option<&'static MenuButton>,
        Option<&'static GenericScreenShell>,
        Option<&'static StartupOnlyShell>,
        Option<&'static AboutShell>,
        Option<&'static ChallengeShell>,
        Option<&'static RosterShell>,
    ),
    (With<ScreenShell>, Without<GameEntity>),
>;

pub(in crate::app) type GameVisibilityQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        Option<&'static PlayerViewEntity>,
        Option<&'static BazaarEntity>,
        Option<&'static PlayingGameEntity>,
    ),
    With<GameEntity>,
>;

#[derive(SystemParam)]
pub(in crate::app) struct RenderGameParams<'w, 's> {
    pub(in crate::app) local: Res<'w, LocalGame>,
    pub(in crate::app) settings: Res<'w, ClientSettings>,
    pub(in crate::app) settings_edit: Res<'w, SettingsEditState>,
    pub(in crate::app) network_state: Res<'w, ClientNetworkState>,
    pub(in crate::app) roster: Res<'w, RosterRecords>,
    pub(in crate::app) themes: Res<'w, ThemePacks>,
    pub(in crate::app) atlases: Res<'w, ThemeAtlasHandles>,
    pub(in crate::app) sound: Res<'w, SoundEventState>,
    pub(in crate::app) bazaar_ui: Res<'w, BazaarUiState>,
    pub(in crate::app) clear_color: ResMut<'w, ClearColor>,
    pub(in crate::app) recon: Res<'w, ReconPanel>,
    pub(in crate::app) cells: Query<'w, 's, (&'static BoardCell, &'static mut Sprite)>,
    pub(in crate::app) text_metrics: TextMetricsQuery<'w, 's>,
    pub(in crate::app) hud: HudTextQuery<'w, 's>,
    pub(in crate::app) phase_text: PhaseTextSingle<'w, 's>,
    pub(in crate::app) menu_text: MenuTextSingle<'w, 's>,
    pub(in crate::app) screen_text: ScreenTextSingle<'w, 's>,
    pub(in crate::app) bazaar_text: BazaarTextQuery<'w, 's>,
    pub(in crate::app) legacy_game_text: LegacyGameTextQuery<'w, 's>,
    pub(in crate::app) challenge_text: ChallengeTextQuery<'w, 's>,
    pub(in crate::app) roster_text: RosterTextQuery<'w, 's>,
    pub(in crate::app) menu_button_text: MenuButtonTextQuery<'w, 's>,
    pub(in crate::app) challenge_slider_knob: ChallengeSliderKnobQuery<'w, 's>,
    pub(in crate::app) bazaar_selection_marker: BazaarSelectionMarkerQuery<'w, 's>,
    pub(in crate::app) reported_startup_render: Local<'s, bool>,
}

pub(in crate::app) fn render_game(mut render: RenderGameParams) {
    let theme = render.themes.get(render.settings.theme);
    let atlas = render.atlases.get(
        render.settings.theme,
        render.settings.content_mode,
        &render.themes,
    );
    for (cell, mut sprite) in &mut render.cells {
        let cell_sprite = render_cell_sprite(
            &render.local,
            &render.recon,
            cell.player,
            cell.x,
            cell.y,
            theme,
        );
        sprite.image = atlas.image.clone();
        sprite.texture_atlas = Some(TextureAtlas {
            layout: atlas.layout.clone(),
            index: cell_sprite.atlas_index,
        });
        sprite.color = cell_sprite.tint;
        sprite.custom_size = Some(Vec2::splat(
            ((theme.cell.size - theme.cell.gap) * render.settings.pixel_scale).max(1.0),
        ));
    }

    for (mut line_height, mut letter_spacing) in &mut render.text_metrics {
        *line_height = LineHeight::RelativeToFont(theme.fonts.line_height);
        *letter_spacing = LetterSpacing::Px(theme.fonts.tracking);
    }

    for (hud, mut text) in &mut render.hud {
        text.0 = player_hud(&render.local, &render.recon, hud.player);
    }

    render.phase_text.0 = phase_label(&render.local, &render.settings, &render.sound);
    render.menu_text.0 = menu_label(&render.local.game, &render.settings, &render.settings_edit);
    render.screen_text.0 = screen_body_label(
        &render.local.game,
        &render.settings,
        &render.settings_edit,
        &render.network_state,
        &render.roster,
    );
    for (bazaar_text, mut text) in &mut render.bazaar_text {
        text.0 = bazaar_text_label(
            bazaar_text.role,
            &render.local,
            &render.bazaar_ui,
            render.settings.content_mode,
        );
    }
    for (legacy_text, mut text) in &mut render.legacy_game_text {
        text.0 = legacy_game_text_label(&render.local, &render.settings, legacy_text.role);
    }
    for (challenge_text, mut text) in &mut render.challenge_text {
        text.0 = challenge_label(challenge_text.role, &render.settings, &render.network_state);
    }
    for (roster_text, mut text) in &mut render.roster_text {
        text.0 = roster_text_label(&render.roster, &render.network_state, roster_text.role);
    }
    for (button, mut text) in &mut render.menu_button_text {
        if button.screen == ClientScreen::Challenge
            && matches!(button.action, MenuAction::StartSelectedChallenge)
        {
            text.0 = challenge_primary_button_label(&render.settings, &render.network_state);
        } else if button.screen == ClientScreen::Challenge
            && matches!(button.action, MenuAction::StartHumanVsComputer)
        {
            text.0 = format!(
                "Play {} Ernie",
                selected_ernie_difficulty(&render.settings).name
            );
        }
    }
    for (knob, mut transform) in &mut render.challenge_slider_knob {
        transform.translation.x = challenge_ernie_slider_x(&render.settings) + knob.x_offset;
    }
    for (marker, mut transform) in &mut render.bazaar_selection_marker {
        transform.translation.y = bazaar_selection_marker_y(&render.bazaar_ui, marker.role);
    }
    let menu_label_chars =
        render.menu_text.0.chars().count() + render.screen_text.0.chars().count();
    let menu_is_unhealthy = render.settings.screen != ClientScreen::Game
        && render.settings.screen != ClientScreen::Startup
        && menu_label_chars == 0;
    render.clear_color.0 = if menu_is_unhealthy {
        Color::srgb(0.5, 0.0, 0.28)
    } else if render.settings.screen == ClientScreen::Startup {
        Color::BLACK
    } else if render.settings.screen == ClientScreen::About {
        theme.about.background
    } else if render.settings.screen == ClientScreen::Settings {
        settings_ui_page_background(render.settings.ui_style, theme)
    } else {
        theme.screen.background
    };

    if !*render.reported_startup_render {
        report_startup_render_health(render.settings.screen, menu_label_chars);
        *render.reported_startup_render = true;
    }
}

pub(in crate::app) fn update_window_layout(
    settings: Res<ClientSettings>,
    local: Res<LocalGame>,
    recon: Res<ReconPanel>,
    themes: Res<ThemePacks>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    let theme = themes.get(settings.theme);
    let layout = active_window_layout(&settings, &local, &recon, theme);
    let width = layout.width.round().max(1.0);
    let height = layout.height.round().max(1.0);
    if (window.resolution.width() - width).abs() > f32::EPSILON
        || (window.resolution.height() - height).abs() > f32::EPSILON
    {
        window.resolution.set(width, height);
    }
}

pub(in crate::app) fn active_window_layout(
    settings: &ClientSettings,
    local: &LocalGame,
    recon: &ReconPanel,
    theme: &LoadedTheme,
) -> ThemeWindowLayout {
    if settings.screen == ClientScreen::Game {
        if local.game.phase() == GamePhase::Bazaar {
            return theme.layout.screens.bazaar;
        }
        if recon.manual_condor || recon.snapshot.is_some() {
            return theme.layout.screens.game_recon;
        }
    }
    theme.layout.screen(settings.screen)
}

pub(in crate::app) fn update_screen_visibility(
    settings: Res<ClientSettings>,
    local: Res<LocalGame>,
    recon: Res<ReconPanel>,
    mut game_entities: GameVisibilityQuery,
    mut shell_entities: ShellVisibilityQuery,
) {
    let game_visible = settings.screen == ClientScreen::Game;
    let bazaar_visible = game_visible && local.game.phase() == GamePhase::Bazaar;
    for (mut visibility, player_view, bazaar_entity, playing_entity) in &mut game_entities {
        let entity_visible = if bazaar_entity.is_some() {
            bazaar_visible
        } else if bazaar_visible {
            false
        } else if playing_entity.is_some() {
            true
        } else {
            player_view.is_none_or(|view| player_view_visible(&local, &recon, view.player))
        };
        *visibility = if game_visible && entity_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (
        mut visibility,
        button,
        generic_shell,
        startup_only,
        about_shell,
        challenge_shell,
        roster_shell,
    ) in &mut shell_entities
    {
        let visible = !game_visible
            && if let Some(button) = button {
                settings.screen != ClientScreen::Settings && button.screen == settings.screen
            } else if challenge_shell.is_some() {
                settings.screen == ClientScreen::Challenge
            } else if about_shell.is_some() {
                settings.screen == ClientScreen::About
            } else if roster_shell.is_some() {
                settings.screen == ClientScreen::Roster
            } else if startup_only.is_some() {
                settings.screen == ClientScreen::Startup
            } else if generic_shell.is_some() {
                settings.screen != ClientScreen::Startup
                    && settings.screen != ClientScreen::About
                    && settings.screen != ClientScreen::Challenge
                    && settings.screen != ClientScreen::Roster
                    && settings.screen != ClientScreen::Settings
            } else {
                true
            };
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(in crate::app) fn player_view_visible(
    local: &LocalGame,
    recon: &ReconPanel,
    player: PlayerId,
) -> bool {
    player == local.local_player
        || (player == opponent_player(local.local_player)
            && (local.computer.is_some() || recon.snapshot.is_some()))
}

pub(in crate::app) fn update_menu_button_visuals(
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut buttons: Query<(&MenuButton, &mut Sprite), With<ButtonFace>>,
) {
    let theme = themes.get(settings.theme);
    let cursor = window.cursor_position().map(|cursor| {
        Vec2::new(
            cursor.x - window.width() / 2.0,
            window.height() / 2.0 - cursor.y,
        )
    });

    for (button, mut sprite) in &mut buttons {
        let hovered = button.screen == settings.screen
            && cursor.is_some_and(|cursor| button.rect.contains(cursor));
        sprite.color = if button.screen == ClientScreen::About {
            if hovered && mouse.pressed(MouseButton::Left) {
                theme.about.button_shadow
            } else if hovered {
                theme.about.button_highlight
            } else {
                theme.about.button_face
            }
        } else if button.screen == ClientScreen::Startup
            || button.screen == ClientScreen::Challenge
            || button.screen == ClientScreen::Roster
        {
            if hovered && mouse.pressed(MouseButton::Left) {
                motif_button_pressed_color()
            } else if hovered {
                motif_button_hover_color()
            } else {
                motif_button_face_color()
            }
        } else if hovered && mouse.pressed(MouseButton::Left) {
            theme.button.pressed
        } else if hovered {
            theme.button.hover
        } else {
            theme.button.normal
        };
    }
}

pub(in crate::app) fn report_startup_render_health(screen: ClientScreen, menu_label_chars: usize) {
    info!("BattleTris render health: screen={screen:?} menu_label_chars={menu_label_chars}");
    if screen != ClientScreen::Game && screen != ClientScreen::Startup && menu_label_chars == 0 {
        error!("BattleTris render health: non-game screen has empty menu text");
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct RenderedCellSprite {
    pub(in crate::app) atlas_index: usize,
    pub(in crate::app) tint: Color,
}

pub(in crate::app) fn board_cell_sprite(
    theme: &LoadedTheme,
    atlas: &ThemeAtlasImageHandle,
    cell_sprite: RenderedCellSprite,
) -> Sprite {
    let mut sprite = Sprite::from_atlas_image(
        atlas.image.clone(),
        TextureAtlas {
            layout: atlas.layout.clone(),
            index: cell_sprite.atlas_index,
        },
    );
    sprite.color = cell_sprite.tint;
    sprite.custom_size = Some(Vec2::splat((theme.cell.size - theme.cell.gap).max(1.0)));
    sprite
}

pub(in crate::app) fn render_cell_sprite(
    local: &LocalGame,
    recon: &ReconPanel,
    player: PlayerId,
    x: usize,
    y: usize,
    theme: &LoadedTheme,
) -> RenderedCellSprite {
    if player != local.local_player {
        if local.computer.is_some() && recon.manual_condor {
            return local
                .game
                .player(player)
                .board()
                .get(Coord { x, y })
                .map_or_else(
                    || empty_cell_sprite(theme),
                    |cell| cell_sprite(cell, false, theme),
                );
        }
        if let Some(snapshot) = &recon.snapshot {
            return snapshot
                .board
                .cells
                .get(y * snapshot.board.width + x)
                .copied()
                .flatten()
                .map_or_else(
                    || empty_cell_sprite(theme),
                    |cell| cell_sprite(cell, false, theme),
                );
        }
        return empty_cell_sprite(theme);
    }

    let piece_cell = local
        .game
        .player(player)
        .active_piece()
        .and_then(|piece| {
            piece
                .cells()
                .into_iter()
                .find(|((px, py), _)| *px == x as isize && *py == y as isize)
        })
        .map(|(_, cell)| cell);

    if let Some(cell) = piece_cell {
        return cell_sprite(cell, true, theme);
    }

    let Some(coord) = Coord::new(x, y) else {
        return empty_cell_sprite(theme);
    };
    local.game.player(player).board().get(coord).map_or_else(
        || empty_cell_sprite(theme),
        |cell| cell_sprite(cell, false, theme),
    )
}
