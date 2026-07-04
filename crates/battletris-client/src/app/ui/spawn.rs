//! UI entity spawning and legacy screen layout construction.

use super::*;

pub(in crate::app) fn setup(
    mut commands: Commands,
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    atlases: Res<ThemeAtlasHandles>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((Camera2d, Msaa::Off));
    let theme = themes.get(settings.theme);

    spawn_screen_shell(&mut commands, theme, &asset_server);
    spawn_settings_ui_shell(&mut commands, theme, &asset_server);
    spawn_challenge_shell(&mut commands, theme, &asset_server);
    spawn_about_shell(&mut commands, theme, &asset_server);
    spawn_roster_shell(&mut commands, theme, &asset_server);

    spawn_player_view(
        &mut commands,
        theme,
        atlases.get(settings.theme, settings.content_mode, &themes),
        PlayerId::One,
        theme.layout.board.player_one_left,
        "Player 1",
    );
    spawn_player_view(
        &mut commands,
        theme,
        atlases.get(settings.theme, settings.content_mode, &themes),
        PlayerId::Two,
        theme.layout.board.player_two_left,
        "Player 2 / Computer",
    );
    spawn_bazaar_overlay(&mut commands, theme, &asset_server);
    spawn_legacy_game_hud(&mut commands, theme, &asset_server);

    commands.spawn((
        Text2d::new("BattleTris"),
        themed_text_font_at_size(theme, ThemedTextFontRole::Body, 22.0, &asset_server),
        TextColor(theme.palette.text_secondary),
        ThemedTextColor {
            role: ThemedTextColorRole::Secondary,
        },
        ThemedTextMetrics,
        Transform::from_xyz(0.0, -300.0, 5.0),
        PhaseText,
        PlayingGameEntity,
        GameEntity,
    ));

    commands.spawn((
        Text2d::new(""),
        themed_text_font(theme, ThemedTextFontRole::Title, &asset_server),
        TextColor(theme.screen.title_text),
        ThemedTextColor {
            role: ThemedTextColorRole::ScreenTitle,
        },
        ThemedTextFont {
            role: ThemedTextFontRole::Title,
        },
        ThemedTextMetrics,
        Transform::from_xyz(0.0, 245.0, 10.0),
        MenuText,
        GenericScreenShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn spawn_bazaar_overlay(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
) {
    let text_assets = ThemeTextAssets {
        theme,
        asset_server,
    };
    let mut backdrop = Sprite::from_image(asset_server.load(theme.sprites.bazaar.clone()));
    backdrop.custom_size = Some(Vec2::new(490.0, 164.0));
    let art_center = bazaar_world(20.0 + 245.0, 18.0 + 82.0);
    commands.spawn((
        backdrop,
        Transform::from_xyz(art_center.x, art_center.y, 20.0),
        Visibility::Hidden,
        ThemedSprite {
            role: ThemedSpriteRole::Bazaar,
        },
        BazaarEntity,
        GameEntity,
    ));

    spawn_bazaar_panel(
        commands,
        bazaar_rect(20.0, 200.0, 300.0, 780.0),
        MotifBevel::Inset,
    );
    spawn_bazaar_scrollbar(commands, 20.0, 200.0, 300.0, 780.0, true, true);
    commands.spawn((
        Sprite::from_color(motif_red3_color(), Vec2::new(254.0, 16.0)),
        Transform::from_xyz(-249.0, bazaar_world(0.0, 210.0).y, 23.0),
        Visibility::Hidden,
        BazaarSelectionMarker {
            role: BazaarSelectionMarkerRole::Background,
        },
        BazaarEntity,
        GameEntity,
    ));
    spawn_bazaar_panel(
        commands,
        bazaar_rect(340.0, 600.0, 780.0, 780.0),
        MotifBevel::Inset,
    );
    spawn_bazaar_scrollbar(commands, 340.0, 600.0, 780.0, 780.0, true, false);
    spawn_bazaar_panel(
        commands,
        bazaar_rect(325.0, 215.0, 475.0, 245.0),
        MotifBevel::Raised,
    );
    spawn_bazaar_panel(
        commands,
        bazaar_rect(325.0, 245.0, 475.0, 315.0),
        MotifBevel::Inset,
    );
    spawn_bazaar_static_text(
        commands,
        text_assets,
        "Funds",
        bazaar_world(372.0, 235.0),
        12.0,
        motif_blue_color(),
        Anchor::CENTER,
    );
    spawn_bazaar_dynamic_text(
        commands,
        text_assets,
        BazaarTextRole::Funds,
        bazaar_world(400.0, 282.0),
        12.0,
        motif_red3_color(),
        Anchor::CENTER,
    );

    for slot in 0..10 {
        let y1 = 204.0 + slot as f32 * 30.0;
        let y2 = y1 + 24.0;
        spawn_bazaar_panel(
            commands,
            bazaar_rect(503.0, y1, 778.0, y2),
            MotifBevel::Raised,
        );
        spawn_bazaar_dynamic_text(
            commands,
            text_assets,
            BazaarTextRole::ArsenalSlot(slot),
            bazaar_world(512.0, y1 + 12.0),
            12.0,
            motif_dim_text_color(),
            Anchor::TOP_LEFT,
        );
    }

    for (label, rect) in [
        ("Add >>", bazaar_rect(340.0, 365.0, 460.0, 415.0)),
        ("<< Remove", bazaar_rect(340.0, 435.0, 460.0, 485.0)),
        ("DONE", bazaar_rect(340.0, 505.0, 460.0, 575.0)),
    ] {
        spawn_bazaar_panel(commands, rect, MotifBevel::Raised);
        let (center, _) = rect;
        spawn_bazaar_static_text(
            commands,
            text_assets,
            label,
            center,
            12.0,
            motif_blue_color(),
            Anchor::CENTER,
        );
    }

    spawn_bazaar_dynamic_text(
        commands,
        text_assets,
        BazaarTextRole::Catalog,
        bazaar_world(24.0, 205.0),
        13.3,
        motif_red3_color(),
        Anchor::TOP_LEFT,
    );
    let selected_text_position = bazaar_world(24.0, 205.0);
    commands.spawn((
        Text2d::new(""),
        text_assets.font(ThemedTextFontRole::Body, 13.3),
        TextColor(Color::WHITE),
        ThemedTextMetrics,
        Anchor::TOP_LEFT,
        Transform::from_xyz(selected_text_position.x, selected_text_position.y, 24.5),
        Visibility::Hidden,
        BazaarEntity,
        BazaarText {
            role: BazaarTextRole::SelectedCatalogRow,
        },
        BazaarSelectionMarker {
            role: BazaarSelectionMarkerRole::Text,
        },
        GameEntity,
    ));
    spawn_bazaar_dynamic_text(
        commands,
        text_assets,
        BazaarTextRole::Message,
        bazaar_world(640.0, 552.0),
        12.0,
        motif_message_green_color(),
        Anchor::CENTER,
    );
    spawn_bazaar_dynamic_text(
        commands,
        text_assets,
        BazaarTextRole::Description,
        bazaar_world(348.0, 606.0),
        12.0,
        motif_red3_color(),
        Anchor::TOP_LEFT,
    );
}

pub(in crate::app) fn spawn_bazaar_panel(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    bevel: MotifBevel,
) {
    spawn_bazaar_rect(commands, center, size, motif_text_panel_color(), 21.0);
    spawn_bazaar_bevel(commands, center, size, 22.0, bevel);
}

pub(in crate::app) fn spawn_bazaar_rect(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    color: Color,
    z: f32,
) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        BazaarEntity,
        GameEntity,
    ));
}

pub(in crate::app) fn spawn_bazaar_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_bazaar_rect(
            commands,
            Vec2::new(center.x + offset.x, center.y + offset.y),
            bevel_size,
            bevel_color,
            z,
        );
    }
}

pub(in crate::app) fn spawn_bazaar_scrollbar(
    commands: &mut Commands,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    vertical: bool,
    horizontal: bool,
) {
    let vertical_y2 = if horizontal { y2 - 18.0 } else { y2 };
    let horizontal_x2 = if vertical { x2 - 18.0 } else { x2 };
    if vertical {
        let bar_x1 = x2 - 18.0;
        spawn_bazaar_legacy_scrollbar(commands, bazaar_rect(bar_x1, y1, x2, vertical_y2), true);
    }
    if horizontal {
        let bar_y1 = y2 - 18.0;
        spawn_bazaar_legacy_scrollbar(commands, bazaar_rect(x1, bar_y1, horizontal_x2, y2), false);
    }
}

pub(in crate::app) fn spawn_bazaar_legacy_scrollbar(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    vertical: bool,
) {
    let parts = legacy_scrollbar_parts(center, size, vertical);
    spawn_bazaar_scrollbar_panel(
        commands,
        center,
        size,
        motif_text_panel_color(),
        21.0,
        MotifBevel::Inset,
    );
    spawn_bazaar_scrollbar_panel(
        commands,
        parts.thumb_center,
        parts.thumb_size,
        motif_button_face_color(),
        22.2,
        MotifBevel::Inset,
    );
    spawn_bazaar_arrow_button(
        commands,
        parts.leading_arrow_center,
        parts.arrow_size,
        if vertical {
            MotifArrowDirection::Up
        } else {
            MotifArrowDirection::Left
        },
    );
    spawn_bazaar_arrow_button(
        commands,
        parts.trailing_arrow_center,
        parts.arrow_size,
        if vertical {
            MotifArrowDirection::Down
        } else {
            MotifArrowDirection::Right
        },
    );
}

pub(in crate::app) fn spawn_bazaar_scrollbar_panel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    color: Color,
    z: f32,
    bevel: MotifBevel,
) {
    spawn_bazaar_rect(commands, center, size, color, z);
    spawn_bazaar_bevel(commands, center, size, z + 0.5, bevel);
}

pub(in crate::app) fn spawn_bazaar_arrow_button(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    direction: MotifArrowDirection,
) {
    spawn_bazaar_scrollbar_panel(
        commands,
        center,
        size,
        motif_button_face_color(),
        23.0,
        MotifBevel::Inset,
    );
    spawn_bazaar_arrow_glyph(commands, center, direction);
}

pub(in crate::app) fn spawn_bazaar_arrow_glyph(
    commands: &mut Commands,
    center: Vec2,
    direction: MotifArrowDirection,
) {
    for index in 0..3 {
        let spread = 1.0 + index as f32 * 2.0;
        let step = index as f32 * 1.6;
        let (offset, size) = match direction {
            MotifArrowDirection::Up => (Vec2::new(0.0, 2.4 - step), Vec2::new(spread, 1.0)),
            MotifArrowDirection::Down => (Vec2::new(0.0, -2.4 + step), Vec2::new(spread, 1.0)),
            MotifArrowDirection::Left => (Vec2::new(-2.4 + step, 0.0), Vec2::new(1.0, spread)),
            MotifArrowDirection::Right => (Vec2::new(2.4 - step, 0.0), Vec2::new(1.0, spread)),
        };
        spawn_bazaar_rect(
            commands,
            Vec2::new(center.x + offset.x, center.y + offset.y),
            size,
            Color::BLACK,
            24.0,
        );
    }
}

pub(in crate::app) fn spawn_bazaar_static_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    label: &'static str,
    position: Vec2,
    font_size: f32,
    color: Color,
    anchor: Anchor,
) {
    commands.spawn((
        Text2d::new(label),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextMetrics,
        anchor,
        Transform::from_xyz(position.x, position.y, 24.0),
        Visibility::Hidden,
        BazaarEntity,
        GameEntity,
    ));
}

pub(in crate::app) fn spawn_bazaar_dynamic_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    role: BazaarTextRole,
    position: Vec2,
    font_size: f32,
    color: Color,
    anchor: Anchor,
) {
    let font_role = bazaar_text_font_role(role);
    commands.spawn((
        Text2d::new(""),
        text_assets.font(font_role, font_size),
        TextColor(color),
        ThemedTextMetrics,
        anchor,
        Transform::from_xyz(position.x, position.y, 24.0),
        Visibility::Hidden,
        BazaarEntity,
        BazaarText { role },
        GameEntity,
    ));
}

pub(in crate::app) fn bazaar_text_font_role(role: BazaarTextRole) -> ThemedTextFontRole {
    match role {
        BazaarTextRole::Funds => ThemedTextFontRole::Mono,
        BazaarTextRole::Catalog
        | BazaarTextRole::SelectedCatalogRow
        | BazaarTextRole::ArsenalSlot(_)
        | BazaarTextRole::Message
        | BazaarTextRole::Description => ThemedTextFontRole::Body,
    }
}

pub(in crate::app) fn bazaar_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> (Vec2, Vec2) {
    let center = Vec2::new(
        (x1 + x2) / 2.0 - LEGACY_BAZAAR_WIDTH / 2.0,
        LEGACY_BAZAAR_HEIGHT / 2.0 - (y1 + y2) / 2.0,
    );
    let size = Vec2::new(x2 - x1, y2 - y1);
    (center, size)
}

pub(in crate::app) fn bazaar_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(
        x - LEGACY_BAZAAR_WIDTH / 2.0,
        LEGACY_BAZAAR_HEIGHT / 2.0 - y,
    )
}

pub(in crate::app) fn spawn_legacy_game_hud(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
) {
    spawn_game_panel(
        commands,
        game_screen_rect(
            LEGACY_GAME_SCORE_X,
            LEGACY_GAME_SCORE_Y,
            LEGACY_GAME_SCORE_WIDTH,
            LEGACY_GAME_SCORE_HEIGHT,
        ),
        motif_text_panel_color(),
        1.0,
        MotifBevel::Inset,
    );
    spawn_legacy_game_text(
        commands,
        theme,
        asset_server,
        LegacyGameTextRole::Score,
        309.0,
        43.0,
        10.0,
    );

    for slot in 0..10 {
        let y = LEGACY_GAME_ARSENAL_Y + slot as f32 * LEGACY_GAME_ARSENAL_ROW_HEIGHT + 3.0;
        spawn_game_panel(
            commands,
            game_screen_rect(LEGACY_GAME_ARSENAL_X, y, LEGACY_GAME_ARSENAL_WIDTH, 30.0),
            motif_button_face_color(),
            1.0,
            MotifBevel::Raised,
        );
        spawn_legacy_game_text(
            commands,
            theme,
            asset_server,
            LegacyGameTextRole::ArsenalSlot(slot),
            311.0,
            y + 8.0,
            11.0,
        );
    }

    spawn_legacy_game_text(
        commands,
        theme,
        asset_server,
        LegacyGameTextRole::Message,
        305.0,
        630.0,
        11.0,
    );
}

pub(in crate::app) fn spawn_game_panel(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
    bevel: MotifBevel,
) {
    spawn_game_rect(commands, center, size, color, z);
    spawn_game_bevel(commands, center, size, z + 0.1, bevel);
}

pub(in crate::app) fn spawn_game_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_game_rect(
            commands,
            Vec2::new(center.x + offset.x, center.y + offset.y),
            bevel_size,
            bevel_color,
            z,
        );
    }
}

pub(in crate::app) fn spawn_game_rect(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    color: Color,
    z: f32,
) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        PlayingGameEntity,
        GameEntity,
    ));
}

pub(in crate::app) fn spawn_legacy_game_text(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
    role: LegacyGameTextRole,
    x: f32,
    y: f32,
    font_size: f32,
) {
    let position = game_screen_world(x, y);
    let color = match role {
        LegacyGameTextRole::Message => Color::BLACK,
        LegacyGameTextRole::Score | LegacyGameTextRole::ArsenalSlot(_) => motif_blue_color(),
    };
    commands.spawn((
        Text2d::new(""),
        themed_text_font_at_size(
            theme,
            legacy_game_text_font_role(role),
            font_size,
            asset_server,
        ),
        TextColor(color),
        ThemedTextMetrics,
        Anchor::TOP_LEFT,
        Transform::from_xyz(position.x, position.y, 5.0),
        LegacyGameText { role },
        PlayingGameEntity,
        GameEntity,
    ));
}

pub(in crate::app) fn legacy_game_text_font_role(role: LegacyGameTextRole) -> ThemedTextFontRole {
    match role {
        LegacyGameTextRole::Score | LegacyGameTextRole::ArsenalSlot(_) => ThemedTextFontRole::Mono,
        LegacyGameTextRole::Message => ThemedTextFontRole::Body,
    }
}

pub(in crate::app) fn game_screen_rect(x: f32, y: f32, width: f32, height: f32) -> (Vec2, Vec2) {
    let center = game_screen_world(x + width / 2.0, y + height / 2.0);
    (center, Vec2::new(width, height))
}

pub(in crate::app) fn game_screen_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x - LEGACY_GAME_WIDTH / 2.0, LEGACY_GAME_HEIGHT / 2.0 - y)
}

pub(in crate::app) fn spawn_screen_shell(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
) {
    commands.spawn((
        Sprite::from_color(theme.screen.background, Vec2::new(640.0, 600.0)),
        Transform::from_xyz(0.0, 0.0, -3.0),
        ThemedColorSprite {
            role: ThemedColorSpriteRole::ScreenBackground,
        },
        GenericScreenShell,
        ScreenShell,
    ));

    let mut startup_sprite = Sprite::from_image(asset_server.load(theme.sprites.startup.clone()));
    startup_sprite.custom_size = Some(Vec2::new(640.0, 440.0));
    commands.spawn((
        startup_sprite,
        Transform::from_xyz(0.0, 80.0, -2.0),
        ThemedSprite {
            role: ThemedSpriteRole::Startup,
        },
        StartupOnlyShell,
        GenericScreenShell,
        ScreenShell,
    ));

    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.biff.clone())),
        Transform::from_xyz(-220.0, -155.0, 1.0),
        ThemedSprite {
            role: ThemedSpriteRole::Biff,
        },
        GenericScreenShell,
        ScreenShell,
    ));

    commands.spawn((
        Text2d::new(""),
        themed_text_font(theme, ThemedTextFontRole::Body, asset_server),
        TextColor(theme.screen.body_text),
        ThemedTextColor {
            role: ThemedTextColorRole::ScreenBody,
        },
        ThemedTextFont {
            role: ThemedTextFontRole::Body,
        },
        ThemedTextMetrics,
        Transform::from_xyz(55.0, 70.0, 4.0),
        ScreenText,
        GenericScreenShell,
        ScreenShell,
    ));

    for spec in startup_buttons(theme) {
        spawn_menu_button(commands, theme, asset_server, spec);
    }
    for spec in secondary_screen_buttons(theme) {
        spawn_menu_button(commands, theme, asset_server, spec);
    }
}

pub(in crate::app) fn spawn_settings_ui_shell(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
) {
    commands
        .spawn((
            SettingsUiRoot,
            SettingsUiBackground,
            SettingsUiSurface {
                role: SettingsUiSurfaceRole::Background,
            },
            Node {
                width: percent(100),
                height: percent(100),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(14),
                padding: UiRect::px(34.0, 34.0, 28.0, 28.0),
                ..default()
            },
            BackgroundColor(theme.screen.background),
            BorderColor::all(Color::NONE),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                SettingsUiTitleText,
                Text::new("Settings"),
                themed_text_font_at_size(theme, ThemedTextFontRole::Title, 21.0, asset_server),
                TextColor(theme.screen.title_text),
                SettingsUiTextColor {
                    role: SettingsUiTextColorRole::Title,
                },
                ThemedTextColor {
                    role: ThemedTextColorRole::ScreenTitle,
                },
                ThemedTextFont {
                    role: ThemedTextFontRole::Title,
                },
            ));
            parent.spawn((
                SettingsUiBiffImage,
                ImageNode::new(asset_server.load(theme.sprites.biff.clone())),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(LEGACY_ROSTER_BIFF_WIDTH),
                    height: px(LEGACY_ROSTER_BIFF_HEIGHT),
                    left: px(78),
                    bottom: px(118),
                    ..default()
                },
            ));
            parent
                .spawn((
                    SettingsUiSurface {
                        role: SettingsUiSurfaceRole::Panel,
                    },
                    Node {
                        width: px(572),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        padding: UiRect::all(px(10)),
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                ))
                .with_children(|parent| {
                    parent.spawn(settings_ui_text(
                        "General",
                        13.0,
                        Color::srgb(0.82, 0.9, 1.0),
                        SettingsUiTextColorRole::Section,
                    ));
                    spawn_settings_dropdown_ui_row(
                        parent,
                        "UI Style",
                        SettingsControl::UiStyle,
                        SettingsUiValue::UiStyle,
                    );
                    spawn_settings_dropdown_ui_row(
                        parent,
                        "Theme",
                        SettingsControl::Theme,
                        SettingsUiValue::Theme,
                    );
                    spawn_settings_dropdown_ui_row(
                        parent,
                        "Sound",
                        SettingsControl::SoundPack,
                        SettingsUiValue::SoundPack,
                    );
                    spawn_settings_dropdown_ui_row(
                        parent,
                        "Challenge Style",
                        SettingsControl::ChallengeStyle,
                        SettingsUiValue::ChallengeStyle,
                    );
                    spawn_settings_checkbox_ui_row(
                        parent,
                        "Hosted Ranked",
                        SettingsControl::HostedRanked,
                    );
                    spawn_settings_scale_ui_row(parent);

                    parent.spawn((
                        Node {
                            height: px(6),
                            ..default()
                        },
                    ));
                    parent.spawn(settings_ui_text(
                        "Text Inputs",
                        13.0,
                        Color::srgb(0.82, 0.9, 1.0),
                        SettingsUiTextColorRole::Section,
                    ));
                    for (label, field) in [
                        ("Display Name", SettingsField::DisplayName),
                        ("Community", SettingsField::CommunityLabel),
                        ("Host Bind", SettingsField::HostBindAddress),
                        ("Share Address", SettingsField::ShareAddress),
                        ("Join Address", SettingsField::JoinAddress),
                        ("Lobby Address", SettingsField::LobbyAddress),
                    ] {
                        spawn_settings_text_input_ui_row(parent, label, field);
                    }

                    parent.spawn((
                        Text::new("Tab/Shift+Tab focus. Type in text boxes. Enter validates/toggles. Click controls to focus."),
                        pixel_text_font(10.0),
                        TextColor(Color::srgb(0.72, 0.76, 0.82)),
                        SettingsUiTextColor {
                            role: SettingsUiTextColorRole::Hint,
                        },
                        Node {
                            width: percent(100),
                            margin: UiRect::top(px(4)),
                            ..default()
                        },
                    ));
                    parent.spawn((
                        SettingsUiStatusText,
                        Text::new(""),
                        pixel_text_font(10.0),
                        TextColor(Color::srgb(0.58, 0.66, 0.76)),
                        SettingsUiTextColor {
                            role: SettingsUiTextColorRole::Status,
                        },
                        Node {
                            width: percent(100),
                            ..default()
                        },
                    ));
                });
            parent
                .spawn(settings_ui_back_button_node(theme))
                .with_child((
                    Text::new("Back"),
                    themed_text_font_at_size(theme, ThemedTextFontRole::Button, 12.0, asset_server),
                    TextColor(theme.button.text),
                    SettingsUiTextColor {
                        role: SettingsUiTextColorRole::Button,
                    },
                    ThemedTextColor {
                        role: ThemedTextColorRole::Button,
                    },
                    ThemedTextFont {
                        role: ThemedTextFontRole::Button,
                    },
                ));
        });
}

pub(in crate::app) fn settings_ui_back_button_node(theme: &LoadedTheme) -> impl Bundle {
    (
        Button,
        SettingsUiBackButton,
        Node {
            width: px(theme.layout.rects.settings_back.width),
            height: px(theme.layout.rects.settings_back.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(px(2)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.52, 0.58, 0.66)),
        BackgroundColor(theme.button.normal),
    )
}

pub(in crate::app) fn spawn_settings_dropdown_ui_row(
    parent: &mut ChildSpawnerCommands,
    label: &'static str,
    control: SettingsControl,
    value: SettingsUiValue,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            ..default()
        })
        .with_children(|parent| {
            spawn_settings_control_row(parent, label, |parent| {
                parent
                    .spawn(settings_ui_button_node(
                        control,
                        SettingsUiAction::ToggleDropdown,
                        px(312),
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            SettingsUiValueText { value },
                            Text::new(""),
                            pixel_text_font(11.0),
                            TextColor(Color::srgb(0.94, 0.96, 1.0)),
                            SettingsUiTextColor {
                                role: SettingsUiTextColorRole::Value,
                            },
                            Node {
                                flex_grow: 1.0,
                                ..default()
                            },
                        ));
                        parent.spawn(settings_ui_text(
                            "v",
                            11.0,
                            Color::srgb(0.94, 0.96, 1.0),
                            SettingsUiTextColorRole::Value,
                        ));
                    });
            });

            parent
                .spawn((
                    SettingsUiDropdownMenu { control },
                    SettingsUiSurface {
                        role: SettingsUiSurfaceRole::Dropdown,
                    },
                    Node {
                        display: Display::None,
                        position_type: PositionType::Absolute,
                        flex_direction: FlexDirection::Column,
                        width: px(312),
                        left: px(148),
                        top: px(22),
                        row_gap: px(2),
                        padding: UiRect::all(px(2)),
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    GlobalZIndex(10),
                    BorderColor::all(Color::srgb(0.52, 0.58, 0.66)),
                    BackgroundColor(Color::srgb(0.04, 0.05, 0.07)),
                ))
                .with_children(|parent| {
                    spawn_settings_dropdown_options(parent, control);
                });
        });
}

pub(in crate::app) fn spawn_settings_dropdown_options(
    parent: &mut ChildSpawnerCommands,
    control: SettingsControl,
) {
    match control {
        SettingsControl::UiStyle => {
            for (label, option) in [
                (
                    "Original",
                    SettingsSelectOption::UiStyle(UiStyleChoice::Original),
                ),
                (
                    "Modern",
                    SettingsSelectOption::UiStyle(UiStyleChoice::Modern),
                ),
            ] {
                spawn_settings_dropdown_option(parent, control, label, option);
            }
        }
        SettingsControl::Theme => {
            for (label, option) in [
                (
                    "Original",
                    SettingsSelectOption::Theme(ThemeChoice::Original),
                ),
                (
                    "High Contrast",
                    SettingsSelectOption::Theme(ThemeChoice::HighContrast),
                ),
            ] {
                spawn_settings_dropdown_option(parent, control, label, option);
            }
        }
        SettingsControl::SoundPack => {
            for (label, option) in [
                (
                    "Generated Default",
                    SettingsSelectOption::SoundPack(SoundPackChoice::GeneratedDefault),
                ),
                (
                    "Muted",
                    SettingsSelectOption::SoundPack(SoundPackChoice::Muted),
                ),
            ] {
                spawn_settings_dropdown_option(parent, control, label, option);
            }
        }
        SettingsControl::ChallengeStyle => {
            for (label, option) in [
                (
                    "Legacy",
                    SettingsSelectOption::ChallengeStyle(ChallengeStyle::Legacy),
                ),
                (
                    "Modern",
                    SettingsSelectOption::ChallengeStyle(ChallengeStyle::Modern),
                ),
            ] {
                spawn_settings_dropdown_option(parent, control, label, option);
            }
        }
        SettingsControl::HostedRanked | SettingsControl::PixelScale | SettingsControl::Text(_) => {}
    }
}

pub(in crate::app) fn spawn_settings_dropdown_option(
    parent: &mut ChildSpawnerCommands,
    control: SettingsControl,
    label: &'static str,
    option: SettingsSelectOption,
) {
    parent
        .spawn(settings_ui_button_node(
            control,
            SettingsUiAction::Select(option),
            px(312),
        ))
        .with_child(settings_ui_text(
            label,
            11.0,
            Color::srgb(0.94, 0.96, 1.0),
            SettingsUiTextColorRole::Value,
        ));
}

pub(in crate::app) fn spawn_settings_checkbox_ui_row(
    parent: &mut ChildSpawnerCommands,
    label: &'static str,
    control: SettingsControl,
) {
    spawn_settings_control_row(parent, label, |parent| {
        parent
            .spawn(settings_ui_button_node(
                control,
                SettingsUiAction::Activate,
                px(34),
            ))
            .with_children(|parent| {
                parent.spawn((
                    SettingsUiValueText {
                        value: SettingsUiValue::HostedRanked,
                    },
                    Text::new(""),
                    pixel_text_font(13.0),
                    TextColor(Color::srgb(0.94, 0.96, 1.0)),
                    SettingsUiTextColor {
                        role: SettingsUiTextColorRole::Value,
                    },
                ));
            });
    });
}

pub(in crate::app) fn spawn_settings_scale_ui_row(parent: &mut ChildSpawnerCommands) {
    spawn_settings_control_row(parent, "Pixel Scale", |parent| {
        parent
            .spawn(settings_ui_button_node(
                SettingsControl::PixelScale,
                SettingsUiAction::Decrement,
                px(34),
            ))
            .with_child(settings_ui_text(
                "-",
                12.0,
                Color::srgb(0.94, 0.96, 1.0),
                SettingsUiTextColorRole::Button,
            ));
        parent.spawn((
            SettingsUiValueText {
                value: SettingsUiValue::PixelScale,
            },
            Text::new(""),
            pixel_text_font(11.0),
            TextColor(Color::srgb(0.94, 0.96, 1.0)),
            SettingsUiTextColor {
                role: SettingsUiTextColorRole::Value,
            },
            Node {
                width: px(80),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ));
        parent
            .spawn(settings_ui_button_node(
                SettingsControl::PixelScale,
                SettingsUiAction::Increment,
                px(34),
            ))
            .with_child(settings_ui_text(
                "+",
                12.0,
                Color::srgb(0.94, 0.96, 1.0),
                SettingsUiTextColorRole::Button,
            ));
    });
}

pub(in crate::app) fn spawn_settings_text_input_ui_row(
    parent: &mut ChildSpawnerCommands,
    label: &'static str,
    field: SettingsField,
) {
    spawn_settings_control_row(parent, label, |parent| {
        parent
            .spawn(settings_ui_button_node(
                SettingsControl::Text(field),
                SettingsUiAction::Focus,
                px(360),
            ))
            .with_children(|parent| {
                parent.spawn((
                    SettingsUiValueText {
                        value: SettingsUiValue::Field(field),
                    },
                    Text::new(""),
                    pixel_text_font(11.0),
                    TextColor(Color::srgb(0.94, 0.96, 1.0)),
                    SettingsUiTextColor {
                        role: SettingsUiTextColorRole::Value,
                    },
                ));
            });
    });
}

pub(in crate::app) fn spawn_settings_control_row(
    parent: &mut ChildSpawnerCommands,
    label: &'static str,
    spawn_control: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            height: px(22),
            align_items: AlignItems::Center,
            column_gap: px(10),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                pixel_text_font(11.0),
                TextColor(Color::srgb(0.78, 0.82, 0.88)),
                SettingsUiTextColor {
                    role: SettingsUiTextColorRole::Label,
                },
                Node {
                    width: px(138),
                    ..default()
                },
            ));
            spawn_control(parent);
        });
}

pub(in crate::app) fn settings_ui_button_node(
    control: SettingsControl,
    action: SettingsUiAction,
    width: Val,
) -> impl Bundle {
    (
        Button,
        SettingsUiControlButton { control, action },
        Node {
            width,
            height: px(22),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::horizontal(px(7)),
            border: UiRect::all(px(2)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.52, 0.58, 0.66)),
        BackgroundColor(Color::srgb(0.07, 0.08, 0.1)),
    )
}

pub(in crate::app) fn settings_ui_text(
    text: impl Into<String>,
    font_size: f32,
    color: Color,
    role: SettingsUiTextColorRole,
) -> impl Bundle {
    (
        Text::new(text),
        pixel_text_font(font_size),
        TextColor(color),
        SettingsUiTextColor { role },
    )
}

pub(in crate::app) fn settings_ui_surface_style(
    style: UiStyleChoice,
    theme: &LoadedTheme,
    role: SettingsUiSurfaceRole,
) -> (Color, BorderColor) {
    match style {
        UiStyleChoice::Original => match role {
            SettingsUiSurfaceRole::Background => (
                settings_ui_page_background(style, theme),
                BorderColor::DEFAULT,
            ),
            SettingsUiSurfaceRole::Panel => (
                motif_text_panel_color(),
                settings_ui_motif_border(MotifBevel::Inset),
            ),
            SettingsUiSurfaceRole::Dropdown => (
                motif_page_color(),
                settings_ui_motif_border(MotifBevel::Inset),
            ),
        },
        UiStyleChoice::Modern => match role {
            SettingsUiSurfaceRole::Background => (
                settings_ui_page_background(style, theme),
                BorderColor::DEFAULT,
            ),
            SettingsUiSurfaceRole::Panel => (Color::NONE, BorderColor::DEFAULT),
            SettingsUiSurfaceRole::Dropdown => (
                Color::srgb(0.04, 0.05, 0.07),
                BorderColor::all(Color::srgb(0.52, 0.58, 0.66)),
            ),
        },
    }
}

pub(in crate::app) fn settings_ui_page_background(
    style: UiStyleChoice,
    theme: &LoadedTheme,
) -> Color {
    match style {
        UiStyleChoice::Original => motif_page_color(),
        UiStyleChoice::Modern => theme.screen.background,
    }
}

pub(in crate::app) fn settings_ui_text_color(
    style: UiStyleChoice,
    theme: &LoadedTheme,
    role: SettingsUiTextColorRole,
) -> Color {
    match style {
        UiStyleChoice::Original => match role {
            SettingsUiTextColorRole::Title
            | SettingsUiTextColorRole::Section
            | SettingsUiTextColorRole::Value
            | SettingsUiTextColorRole::Button => motif_blue_color(),
            SettingsUiTextColorRole::Label | SettingsUiTextColorRole::Hint => Color::BLACK,
            SettingsUiTextColorRole::Status => motif_message_green_color(),
        },
        UiStyleChoice::Modern => match role {
            SettingsUiTextColorRole::Title => theme.screen.title_text,
            SettingsUiTextColorRole::Section => Color::srgb(0.82, 0.9, 1.0),
            SettingsUiTextColorRole::Label => Color::srgb(0.78, 0.82, 0.88),
            SettingsUiTextColorRole::Value => Color::srgb(0.94, 0.96, 1.0),
            SettingsUiTextColorRole::Hint => Color::srgb(0.72, 0.76, 0.82),
            SettingsUiTextColorRole::Status => Color::srgb(0.58, 0.66, 0.76),
            SettingsUiTextColorRole::Button => theme.button.text,
        },
    }
}

pub(in crate::app) fn settings_ui_control_background(
    style: UiStyleChoice,
    action: SettingsUiAction,
    interaction: Interaction,
    focused: bool,
    enabled: bool,
) -> Color {
    if !enabled {
        return Color::NONE;
    }

    match style {
        UiStyleChoice::Original => {
            if interaction == Interaction::Pressed {
                motif_button_pressed_color()
            } else if settings_ui_control_is_field_like(action) {
                motif_text_panel_color()
            } else if interaction == Interaction::Hovered || focused {
                motif_button_hover_color()
            } else {
                motif_button_face_color()
            }
        }
        UiStyleChoice::Modern => {
            if interaction == Interaction::Pressed {
                Color::srgb(0.18, 0.28, 0.42)
            } else if focused {
                Color::srgb(0.11, 0.16, 0.24)
            } else if interaction == Interaction::Hovered {
                Color::srgb(0.13, 0.15, 0.18)
            } else {
                Color::srgb(0.07, 0.08, 0.1)
            }
        }
    }
}

pub(in crate::app) fn settings_ui_control_border(
    style: UiStyleChoice,
    action: SettingsUiAction,
    interaction: Interaction,
    focused: bool,
    enabled: bool,
) -> BorderColor {
    if !enabled {
        return BorderColor::DEFAULT;
    }

    match style {
        UiStyleChoice::Original => {
            let bevel = if interaction == Interaction::Pressed
                || settings_ui_control_is_field_like(action)
            {
                MotifBevel::Inset
            } else {
                MotifBevel::Raised
            };
            settings_ui_motif_border(bevel)
        }
        UiStyleChoice::Modern => BorderColor::all(if focused {
            Color::srgb(0.96, 0.78, 0.28)
        } else if interaction == Interaction::Hovered {
            Color::srgb(0.82, 0.88, 0.96)
        } else {
            Color::srgb(0.52, 0.58, 0.66)
        }),
    }
}

pub(in crate::app) fn settings_ui_back_button_background(
    style: UiStyleChoice,
    theme: &LoadedTheme,
    interaction: Interaction,
    enabled: bool,
) -> Color {
    if !enabled {
        return Color::NONE;
    }

    match style {
        UiStyleChoice::Original => {
            if interaction == Interaction::Pressed {
                motif_button_pressed_color()
            } else if interaction == Interaction::Hovered {
                motif_button_hover_color()
            } else {
                motif_button_face_color()
            }
        }
        UiStyleChoice::Modern => {
            if interaction == Interaction::Pressed {
                theme.button.pressed
            } else if interaction == Interaction::Hovered {
                theme.button.hover
            } else {
                theme.button.normal
            }
        }
    }
}

pub(in crate::app) fn settings_ui_back_button_border(
    style: UiStyleChoice,
    interaction: Interaction,
    enabled: bool,
) -> BorderColor {
    if !enabled {
        return BorderColor::DEFAULT;
    }

    match style {
        UiStyleChoice::Original => {
            settings_ui_motif_border(if interaction == Interaction::Pressed {
                MotifBevel::Inset
            } else {
                MotifBevel::Raised
            })
        }
        UiStyleChoice::Modern => BorderColor::all(if interaction == Interaction::Hovered {
            Color::srgb(0.82, 0.88, 0.96)
        } else {
            Color::srgb(0.52, 0.58, 0.66)
        }),
    }
}

pub(in crate::app) fn settings_ui_control_is_field_like(action: SettingsUiAction) -> bool {
    matches!(
        action,
        SettingsUiAction::Focus | SettingsUiAction::Activate | SettingsUiAction::ToggleDropdown
    )
}

pub(in crate::app) fn settings_ui_motif_border(bevel: MotifBevel) -> BorderColor {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    BorderColor {
        top: top_left,
        left: top_left,
        bottom: bottom_right,
        right: bottom_right,
    }
}

pub(in crate::app) fn spawn_challenge_shell(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
) {
    let text_assets = ThemeTextAssets {
        theme,
        asset_server,
    };
    spawn_challenge_rect(
        commands,
        challenge_rect(0.0, 0.0, 800.0, 700.0),
        motif_page_color(),
        -3.0,
    );
    spawn_challenge_panel(
        commands,
        challenge_rect(20.0, 20.0, 400.0, 500.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_challenge_panel(
        commands,
        challenge_rect(440.0, 320.0, 780.0, 680.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_challenge_computer_frame(commands, text_assets);

    let logo_top_left = challenge_point(540.0, 30.0);
    let logo_size = Vec2::new(105.0, 105.0);
    let logo_center = Vec2::new(
        logo_top_left.x + logo_size.x / 2.0 - 320.0,
        300.0 - (logo_top_left.y + logo_size.y / 2.0),
    );
    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.biff.clone())),
        Transform::from_xyz(logo_center.x, logo_center.y, 1.0),
        Visibility::Hidden,
        ChallengeLogo,
        ChallengeShell,
        ScreenShell,
    ));

    spawn_challenge_scrollbar(commands, 380.0, 20.0, 400.0, 480.0, true);
    spawn_challenge_scrollbar(commands, 20.0, 480.0, 380.0, 500.0, false);
    spawn_challenge_scrollbar(commands, 760.0, 320.0, 780.0, 680.0, true);

    spawn_challenge_text(
        commands,
        text_assets,
        ChallengeTextRole::UserList,
        challenge_rect_center(38.0, 44.0, 382.0, 470.0),
        12.0,
        motif_red3_color(),
    );
    spawn_challenge_text(
        commands,
        text_assets,
        ChallengeTextRole::UserInfo,
        challenge_rect_center(458.0, 340.0, 762.0, 660.0),
        12.0,
        motif_red3_color(),
    );
    spawn_challenge_text(
        commands,
        text_assets,
        ChallengeTextRole::ComputerStatus,
        challenge_world(210.0, 625.0),
        11.0,
        Color::BLACK,
    );
    spawn_static_challenge_text(
        commands,
        text_assets,
        "Available for challenges",
        challenge_world(155.0, 653.0),
        11.0,
        Color::BLACK,
    );
    spawn_challenge_checkbox(commands, challenge_rect(40.0, 648.0, 52.0, 660.0), 2.0);
    spawn_challenge_slider(commands);
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) enum MotifBevel {
    Raised,
    Inset,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) enum MotifArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

pub(in crate::app) const LEGACY_SCROLLBAR_INSET: f32 = 2.0;

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct LegacyScrollbarParts {
    pub(in crate::app) thumb_center: Vec2,
    pub(in crate::app) thumb_size: Vec2,
    pub(in crate::app) leading_arrow_center: Vec2,
    pub(in crate::app) trailing_arrow_center: Vec2,
    pub(in crate::app) arrow_size: Vec2,
}

pub(in crate::app) fn legacy_scrollbar_parts(
    center: Vec2,
    size: Vec2,
    vertical: bool,
) -> LegacyScrollbarParts {
    let thickness = if vertical { size.x } else { size.y };
    let inset = LEGACY_SCROLLBAR_INSET.min(thickness / 3.0).max(0.0);
    let cap_extent = (thickness - inset * 2.0).max(1.0);
    let track_length = if vertical { size.y } else { size.x };
    let thumb_extent = (track_length - inset * 4.0 - cap_extent * 2.0).max(1.0);

    if vertical {
        let arrow_offset = size.y / 2.0 - inset - cap_extent / 2.0;
        LegacyScrollbarParts {
            thumb_center: center,
            thumb_size: Vec2::new(cap_extent, thumb_extent),
            leading_arrow_center: Vec2::new(center.x, center.y + arrow_offset),
            trailing_arrow_center: Vec2::new(center.x, center.y - arrow_offset),
            arrow_size: Vec2::new(cap_extent, cap_extent),
        }
    } else {
        let arrow_offset = size.x / 2.0 - inset - cap_extent / 2.0;
        LegacyScrollbarParts {
            thumb_center: center,
            thumb_size: Vec2::new(thumb_extent, cap_extent),
            leading_arrow_center: Vec2::new(center.x - arrow_offset, center.y),
            trailing_arrow_center: Vec2::new(center.x + arrow_offset, center.y),
            arrow_size: Vec2::new(cap_extent, cap_extent),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct ChallengeScreenRect {
    pub(in crate::app) x1: f32,
    pub(in crate::app) y1: f32,
    pub(in crate::app) x2: f32,
    pub(in crate::app) y2: f32,
}

impl ChallengeScreenRect {
    pub(in crate::app) const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

pub(in crate::app) fn motif_page_color() -> Color {
    Color::srgba_u8(0xbf, 0xbf, 0xbf, 0xff)
}

pub(in crate::app) fn motif_text_panel_color() -> Color {
    Color::srgba_u8(0xa8, 0xa8, 0xa8, 0xff)
}

pub(in crate::app) fn motif_button_face_color() -> Color {
    Color::srgba_u8(0xbe, 0xbe, 0xbe, 0xff)
}

pub(in crate::app) fn motif_button_hover_color() -> Color {
    Color::srgba_u8(0xd6, 0xd6, 0xd6, 0xff)
}

pub(in crate::app) fn motif_button_pressed_color() -> Color {
    Color::srgba_u8(0xa8, 0xa8, 0xa8, 0xff)
}

pub(in crate::app) fn motif_highlight_color() -> Color {
    Color::srgba_u8(0xe4, 0xe4, 0xe4, 0xff)
}

pub(in crate::app) fn motif_shadow_color() -> Color {
    Color::srgba_u8(0x67, 0x67, 0x67, 0xff)
}

pub(in crate::app) fn motif_red3_color() -> Color {
    Color::srgba_u8(0xcd, 0x00, 0x00, 0xff)
}

pub(in crate::app) fn motif_blue_color() -> Color {
    Color::srgba_u8(0x00, 0x00, 0xcc, 0xff)
}

pub(in crate::app) fn motif_dim_text_color() -> Color {
    Color::srgba_u8(0xc0, 0xc0, 0xc0, 0xff)
}

pub(in crate::app) fn motif_message_green_color() -> Color {
    Color::srgba_u8(0x33, 0x66, 0x00, 0xff)
}

pub(in crate::app) fn spawn_roster_shell(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
) {
    let text_assets = ThemeTextAssets {
        theme,
        asset_server,
    };
    spawn_roster_rect(
        commands,
        roster_rect(0.0, 0.0, LEGACY_ROSTER_WIDTH, LEGACY_ROSTER_HEIGHT),
        motif_page_color(),
        -3.0,
    );

    let logo_center = roster_world(
        75.0 + LEGACY_ROSTER_BIFF_WIDTH / 2.0,
        3.0 + LEGACY_ROSTER_BIFF_HEIGHT / 2.0,
    );
    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.biff.clone())),
        Transform::from_xyz(logo_center.x, logo_center.y, 1.0),
        Visibility::Hidden,
        ThemedSprite {
            role: ThemedSpriteRole::Biff,
        },
        RosterShell,
        ScreenShell,
    ));

    spawn_roster_panel(
        commands,
        roster_rect(15.0, 123.0, 225.0, 547.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_roster_panel(
        commands,
        roster_rect(255.0, 123.0, 585.0, 330.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_roster_panel(
        commands,
        roster_rect(255.0, 341.0, 585.0, 547.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_roster_scrollbar(commands, 15.0, 123.0, 225.0, 547.0);
    spawn_roster_scrollbar(commands, 255.0, 123.0, 585.0, 330.0);
    spawn_roster_scrollbar(commands, 255.0, 341.0, 585.0, 547.0);

    spawn_roster_static_label(
        commands,
        text_assets,
        "Head\nto\nHead",
        roster_rect(255.0, 15.0, 322.0, 120.0),
        14.0,
    );
    spawn_roster_dynamic_label(
        commands,
        text_assets,
        RosterTextRole::Player1Name,
        roster_rect(322.0, 15.0, 453.0, 67.0),
        12.0,
    );
    spawn_roster_dynamic_label(
        commands,
        text_assets,
        RosterTextRole::Player2Name,
        roster_rect(453.0, 15.0, 585.0, 67.0),
        12.0,
    );
    spawn_roster_dynamic_label(
        commands,
        text_assets,
        RosterTextRole::Player1Score,
        roster_rect(322.0, 67.0, 453.0, 120.0),
        14.0,
    );
    spawn_roster_dynamic_label(
        commands,
        text_assets,
        RosterTextRole::Player2Score,
        roster_rect(453.0, 67.0, 585.0, 120.0),
        14.0,
    );

    spawn_roster_dynamic_text(
        commands,
        text_assets,
        RosterTextRole::UserList,
        roster_world(26.0, 139.0),
        15.0,
        motif_red3_color(),
    );
    spawn_roster_dynamic_text(
        commands,
        text_assets,
        RosterTextRole::UserInfo1,
        roster_world(270.0, 139.0),
        11.0,
        Color::BLACK,
    );
    spawn_roster_dynamic_text(
        commands,
        text_assets,
        RosterTextRole::UserInfo2,
        roster_world(270.0, 357.0),
        11.0,
        Color::BLACK,
    );

    spawn_roster_static_button(
        commands,
        text_assets,
        "By Name",
        roster_rect(22.0, 555.0, 112.0, 585.0),
    );
    spawn_roster_static_button(
        commands,
        text_assets,
        "By Rank",
        roster_rect(127.0, 555.0, 217.0, 585.0),
    );
}

pub(in crate::app) fn spawn_roster_static_button(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    label: &'static str,
    rect: (Vec2, Vec2),
) {
    spawn_roster_panel(
        commands,
        rect,
        motif_button_face_color(),
        1.0,
        MotifBevel::Raised,
    );
    let (center, _) = rect;
    commands.spawn((
        Text2d::new(label),
        text_assets.font(ThemedTextFontRole::Button, 12.0),
        TextColor(motif_blue_color()),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y, 4.0),
        Visibility::Hidden,
        RosterShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn spawn_roster_static_label(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    label: &'static str,
    rect: (Vec2, Vec2),
    font_size: f32,
) {
    spawn_roster_panel(
        commands,
        rect,
        motif_text_panel_color(),
        1.0,
        MotifBevel::Raised,
    );
    let (center, _) = rect;
    commands.spawn((
        Text2d::new(label),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(motif_blue_color()),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y - 4.0, 4.0),
        Visibility::Hidden,
        RosterShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn spawn_roster_dynamic_label(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    role: RosterTextRole,
    rect: (Vec2, Vec2),
    font_size: f32,
) {
    spawn_roster_panel(
        commands,
        rect,
        motif_text_panel_color(),
        1.0,
        MotifBevel::Raised,
    );
    let (center, _) = rect;
    commands.spawn((
        Text2d::new(""),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(motif_blue_color()),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y - 4.0, 4.0),
        Visibility::Hidden,
        RosterText { role },
        RosterShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn spawn_roster_dynamic_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    role: RosterTextRole,
    position: Vec2,
    font_size: f32,
    color: Color,
) {
    commands.spawn((
        Text2d::new(""),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextMetrics,
        Anchor::TOP_LEFT,
        Transform::from_xyz(position.x, position.y, 4.0),
        Visibility::Hidden,
        RosterText { role },
        RosterShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn spawn_roster_panel(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
    bevel: MotifBevel,
) {
    spawn_roster_rect(commands, (center, size), color, z);
    spawn_roster_bevel(commands, center, size, z + 0.1, bevel);
}

pub(in crate::app) fn spawn_roster_scrollbar(
    commands: &mut Commands,
    _x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
) {
    let bar_x1 = x2 - 20.0;
    let (center, size) = roster_rect(bar_x1, y1, x2, y2);
    spawn_roster_panel(
        commands,
        (center, size),
        motif_page_color(),
        2.0,
        MotifBevel::Inset,
    );
    let parts = legacy_scrollbar_parts(center, size, true);
    spawn_roster_panel(
        commands,
        (parts.thumb_center, parts.thumb_size),
        motif_button_face_color(),
        2.2,
        MotifBevel::Inset,
    );
    spawn_roster_arrow_button(
        commands,
        parts.leading_arrow_center,
        parts.arrow_size,
        MotifArrowDirection::Up,
    );
    spawn_roster_arrow_button(
        commands,
        parts.trailing_arrow_center,
        parts.arrow_size,
        MotifArrowDirection::Down,
    );
}

pub(in crate::app) fn spawn_roster_arrow_button(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    direction: MotifArrowDirection,
) {
    spawn_roster_panel(
        commands,
        (center, size),
        motif_button_face_color(),
        2.4,
        MotifBevel::Inset,
    );
    spawn_roster_arrow_glyph(commands, center, direction, 3.0);
}

pub(in crate::app) fn spawn_roster_arrow_glyph(
    commands: &mut Commands,
    center: Vec2,
    direction: MotifArrowDirection,
    z: f32,
) {
    for index in 0..3 {
        let spread = 1.0 + index as f32 * 2.0;
        let step = index as f32 * 1.6;
        let offset = match direction {
            MotifArrowDirection::Up => Vec2::new(0.0, 2.4 - step),
            MotifArrowDirection::Down => Vec2::new(0.0, -2.4 + step),
            MotifArrowDirection::Left | MotifArrowDirection::Right => Vec2::ZERO,
        };
        spawn_roster_rect(
            commands,
            (
                Vec2::new(center.x + offset.x, center.y + offset.y),
                Vec2::new(spread, 1.0),
            ),
            Color::BLACK,
            z,
        );
    }
}

pub(in crate::app) fn spawn_roster_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_roster_rect(
            commands,
            (
                Vec2::new(center.x + offset.x, center.y + offset.y),
                bevel_size,
            ),
            bevel_color,
            z,
        );
    }
}

pub(in crate::app) fn spawn_roster_rect(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        RosterShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn roster_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> (Vec2, Vec2) {
    let center = Vec2::new(
        (x1 + x2) / 2.0 - LEGACY_ROSTER_WIDTH / 2.0,
        LEGACY_ROSTER_HEIGHT / 2.0 - (y1 + y2) / 2.0,
    );
    let size = Vec2::new(x2 - x1, y2 - y1);
    (center, size)
}

pub(in crate::app) fn roster_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(
        x - LEGACY_ROSTER_WIDTH / 2.0,
        LEGACY_ROSTER_HEIGHT / 2.0 - y,
    )
}

pub(in crate::app) fn spawn_challenge_rect(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        ChallengeShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn spawn_challenge_panel(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
    bevel: MotifBevel,
) {
    spawn_challenge_rect(commands, (center, size), color, z);
    spawn_challenge_bevel(commands, center, size, z + 0.1, bevel);
}

pub(in crate::app) fn spawn_challenge_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_challenge_rect(
            commands,
            (
                Vec2::new(center.x + offset.x, center.y + offset.y),
                bevel_size,
            ),
            bevel_color,
            z,
        );
    }
}

pub(in crate::app) fn spawn_challenge_computer_frame(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
) {
    spawn_challenge_rect(
        commands,
        challenge_rect(20.0, 520.0, 400.0, 680.0),
        motif_page_color(),
        0.0,
    );
    spawn_challenge_etched_frame_screen(
        commands,
        ChallengeScreenRect::new(16.0, 454.0, 320.0, 583.0),
        (23.0, 137.0),
        0.2,
    );
    spawn_static_challenge_text(
        commands,
        text_assets,
        "Play Computer",
        challenge_screen_world(76.0, 450.0),
        12.0,
        Color::BLACK,
    );
}

pub(in crate::app) fn spawn_challenge_etched_frame_screen(
    commands: &mut Commands,
    rect: ChallengeScreenRect,
    title_gap: (f32, f32),
    z: f32,
) {
    let gap = Some(title_gap);
    spawn_challenge_horizontal_segments(
        commands,
        ChallengeScreenRect::new(rect.x1, rect.y1, rect.x2, rect.y1 + 1.0),
        motif_shadow_color(),
        z,
        gap,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x1,
        rect.y1,
        rect.x1 + 1.0,
        rect.y2,
        motif_shadow_color(),
        z,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x1,
        rect.y2 - 1.0,
        rect.x2,
        rect.y2,
        motif_highlight_color(),
        z,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x2 - 1.0,
        rect.y1,
        rect.x2,
        rect.y2,
        motif_highlight_color(),
        z,
    );

    spawn_challenge_horizontal_segments(
        commands,
        ChallengeScreenRect::new(rect.x1 + 1.0, rect.y1 + 1.0, rect.x2 - 1.0, rect.y1 + 2.0),
        motif_highlight_color(),
        z + 0.1,
        gap,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x1 + 1.0,
        rect.y1 + 1.0,
        rect.x1 + 2.0,
        rect.y2 - 1.0,
        motif_highlight_color(),
        z + 0.1,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x1 + 1.0,
        rect.y2 - 2.0,
        rect.x2 - 1.0,
        rect.y2 - 1.0,
        motif_shadow_color(),
        z + 0.1,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x2 - 2.0,
        rect.y1 + 1.0,
        rect.x2 - 1.0,
        rect.y2 - 1.0,
        motif_shadow_color(),
        z + 0.1,
    );
}

pub(in crate::app) fn spawn_challenge_horizontal_segments(
    commands: &mut Commands,
    rect: ChallengeScreenRect,
    color: Color,
    z: f32,
    gap: Option<(f32, f32)>,
) {
    if let Some((gap_x1, gap_x2)) = gap {
        if gap_x1 > rect.x1 {
            spawn_challenge_screen_rect(commands, rect.x1, rect.y1, gap_x1, rect.y2, color, z);
        }
        if gap_x2 < rect.x2 {
            spawn_challenge_screen_rect(commands, gap_x2, rect.y1, rect.x2, rect.y2, color, z);
        }
    } else {
        spawn_challenge_screen_rect(commands, rect.x1, rect.y1, rect.x2, rect.y2, color, z);
    }
}

pub(in crate::app) fn spawn_challenge_scrollbar(
    commands: &mut Commands,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    vertical: bool,
) {
    let (center, size) = challenge_rect(x1, y1, x2, y2);
    spawn_challenge_panel(
        commands,
        (center, size),
        motif_page_color(),
        2.0,
        MotifBevel::Inset,
    );

    let parts = legacy_scrollbar_parts(center, size, vertical);
    spawn_challenge_panel(
        commands,
        (parts.thumb_center, parts.thumb_size),
        motif_button_face_color(),
        2.2,
        MotifBevel::Inset,
    );

    if vertical {
        spawn_challenge_arrow_button(
            commands,
            parts.leading_arrow_center,
            parts.arrow_size,
            MotifArrowDirection::Up,
            2.4,
        );
        spawn_challenge_arrow_button(
            commands,
            parts.trailing_arrow_center,
            parts.arrow_size,
            MotifArrowDirection::Down,
            2.4,
        );
    } else {
        spawn_challenge_arrow_button(
            commands,
            parts.leading_arrow_center,
            parts.arrow_size,
            MotifArrowDirection::Left,
            2.4,
        );
        spawn_challenge_arrow_button(
            commands,
            parts.trailing_arrow_center,
            parts.arrow_size,
            MotifArrowDirection::Right,
            2.4,
        );
    }
}

pub(in crate::app) fn spawn_challenge_arrow_button(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    direction: MotifArrowDirection,
    z: f32,
) {
    spawn_challenge_panel(
        commands,
        (center, size),
        motif_button_face_color(),
        z,
        MotifBevel::Inset,
    );
    spawn_challenge_arrow_glyph(commands, center, direction, z + 0.5);
}

pub(in crate::app) fn spawn_challenge_arrow_glyph(
    commands: &mut Commands,
    center: Vec2,
    direction: MotifArrowDirection,
    z: f32,
) {
    for index in 0..3 {
        let spread = 1.0 + index as f32 * 2.0;
        let step = index as f32 * 1.6;
        let (offset, size) = match direction {
            MotifArrowDirection::Up => (Vec2::new(0.0, 2.4 - step), Vec2::new(spread, 1.0)),
            MotifArrowDirection::Down => (Vec2::new(0.0, -2.4 + step), Vec2::new(spread, 1.0)),
            MotifArrowDirection::Left => (Vec2::new(-2.4 + step, 0.0), Vec2::new(1.0, spread)),
            MotifArrowDirection::Right => (Vec2::new(2.4 - step, 0.0), Vec2::new(1.0, spread)),
        };
        spawn_challenge_rect(
            commands,
            (Vec2::new(center.x + offset.x, center.y + offset.y), size),
            Color::BLACK,
            z,
        );
    }
}

pub(in crate::app) fn spawn_challenge_checkbox(
    commands: &mut Commands,
    rect: (Vec2, Vec2),
    z: f32,
) {
    let (center, size) = rect;
    spawn_challenge_rect(commands, (center, size), motif_page_color(), z);
    spawn_challenge_bevel(commands, center, size, z + 0.1, MotifBevel::Inset);
}

pub(in crate::app) fn spawn_challenge_slider(commands: &mut Commands) {
    spawn_challenge_panel(
        commands,
        challenge_screen_rect(30.0, 502.0, 306.0, 516.0),
        motif_page_color(),
        1.0,
        MotifBevel::Inset,
    );
    spawn_challenge_slider_knob(
        commands,
        challenge_screen_world(46.0, 509.0),
        Vec2::new(28.0, 10.0),
    );
}

pub(in crate::app) fn spawn_challenge_slider_knob(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
) {
    spawn_challenge_slider_knob_rect(
        commands,
        center,
        size,
        motif_button_face_color(),
        2.0,
        center.x,
    );
    let (top_left, bottom_right) = (motif_highlight_color(), motif_shadow_color());
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 1.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(1.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 1.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(1.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_challenge_slider_knob_rect(
            commands,
            Vec2::new(center.x + offset.x, center.y + offset.y),
            bevel_size,
            bevel_color,
            2.1,
            center.x,
        );
    }
}

pub(in crate::app) fn spawn_challenge_slider_knob_rect(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    color: Color,
    z: f32,
    base_x: f32,
) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        ChallengeSliderKnob {
            x_offset: center.x - base_x,
        },
        ChallengeShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn spawn_challenge_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    role: ChallengeTextRole,
    center: Vec2,
    font_size: f32,
    color: Color,
) {
    commands.spawn((
        Text2d::new(""),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y, 4.0),
        Visibility::Hidden,
        ChallengeText { role },
        ChallengeShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn spawn_static_challenge_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    text: &'static str,
    center: Vec2,
    font_size: f32,
    color: Color,
) {
    commands.spawn((
        Text2d::new(text),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y, 4.0),
        Visibility::Hidden,
        ChallengeShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn challenge_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> (Vec2, Vec2) {
    let top_left = challenge_point(x1, y1);
    let bottom_right = challenge_point(x2, y2);
    let center = Vec2::new(
        (top_left.x + bottom_right.x) / 2.0 - 320.0,
        300.0 - (top_left.y + bottom_right.y) / 2.0,
    );
    let size = Vec2::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y);
    (center, size)
}

pub(in crate::app) fn challenge_rect_center(x1: f32, y1: f32, x2: f32, y2: f32) -> Vec2 {
    challenge_rect(x1, y1, x2, y2).0
}

pub(in crate::app) fn spawn_challenge_screen_rect(
    commands: &mut Commands,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color,
    z: f32,
) {
    spawn_challenge_rect(commands, challenge_screen_rect(x1, y1, x2, y2), color, z);
}

pub(in crate::app) fn challenge_screen_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> (Vec2, Vec2) {
    let center = Vec2::new((x1 + x2) / 2.0 - 320.0, 300.0 - (y1 + y2) / 2.0);
    let size = Vec2::new(x2 - x1, y2 - y1);
    (center, size)
}

pub(in crate::app) fn challenge_screen_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x - 320.0, 300.0 - y)
}

pub(in crate::app) fn challenge_world(x: f32, y: f32) -> Vec2 {
    let point = challenge_point(x, y);
    Vec2::new(point.x - 320.0, 300.0 - point.y)
}

pub(in crate::app) fn challenge_point(x: f32, y: f32) -> Vec2 {
    Vec2::new(x * 0.8, y * 6.0 / 7.0)
}

pub(in crate::app) fn spawn_about_shell(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
) {
    let text_assets = ThemeTextAssets {
        theme,
        asset_server,
    };
    commands.spawn((
        Sprite::from_color(theme.about.background, Vec2::new(640.0, 600.0)),
        Transform::from_xyz(0.0, 34.0, 0.0),
        Visibility::Hidden,
        ThemedColorSprite {
            role: ThemedColorSpriteRole::AboutBackground,
        },
        AboutShell,
        ScreenShell,
    ));

    for x in [113.0, 527.0] {
        commands.spawn((
            Sprite::from_image(asset_server.load(theme.sprites.crest.clone())),
            about_transform(x, 181.0, 1.0),
            Visibility::Hidden,
            ThemedSprite {
                role: ThemedSpriteRole::AboutIcon,
            },
            AboutShell,
            ScreenShell,
        ));
    }

    spawn_about_text(
        commands,
        text_assets,
        "BattleTris",
        Vec2::new(320.0, 60.0),
        12.0,
        theme.about.title_text,
        ThemedTextColorRole::AboutTitle,
    );
    spawn_about_text(
        commands,
        text_assets,
        "Version 1.0",
        Vec2::new(320.0, 124.0),
        11.0,
        theme.about.title_text,
        ThemedTextColorRole::AboutTitle,
    );
    spawn_about_text(
        commands,
        text_assets,
        "Bryan Cantrill",
        Vec2::new(320.0, 156.0),
        11.0,
        theme.about.name_text,
        ThemedTextColorRole::AboutName,
    );
    spawn_about_text(
        commands,
        text_assets,
        "Charlie Hoecker",
        Vec2::new(320.0, 190.0),
        11.0,
        theme.about.name_text,
        ThemedTextColorRole::AboutName,
    );
    spawn_about_text(
        commands,
        text_assets,
        "Mike Shapiro",
        Vec2::new(320.0, 225.0),
        11.0,
        theme.about.name_text,
        ThemedTextColorRole::AboutName,
    );
    spawn_about_text(
        commands,
        text_assets,
        "battletris@cs.brown.edu",
        Vec2::new(320.0, 261.0),
        11.0,
        theme.about.name_text,
        ThemedTextColorRole::AboutName,
    );

    for (text, y) in [
        (
            "BattleTris Copyright (c) 1993-1997 Bryan Cantrill, Charles Hoecker, Michael Shapiro.",
            306.0,
        ),
        ("Special thanks to:", 328.0),
        (
            "Libby \"Hoss the Camel\" Cantrill, for many ideas and extensive play-testing",
            351.0,
        ),
        ("Drew Davis, for great advice early on", 374.0),
        (
            "Tony, for cleaning up our empty Mountain Dew bottles",
            397.0,
        ),
        (
            "botrytis, pebbles and barney for many long and passionate nights",
            420.0,
        ),
        (
            "The original BT beta testers:  Ben, Caffer, Masi, Dave, Scott and Todd",
            443.0,
        ),
        ("and of course", 466.0),
        ("Kevin \"shouldn't there be a paren there?\" Regan", 489.0),
    ] {
        spawn_about_text(
            commands,
            text_assets,
            text,
            Vec2::new(320.0, y),
            10.0,
            theme.about.credit_text,
            ThemedTextColorRole::AboutCredit,
        );
    }

    spawn_about_button_bevel(commands, theme);
}

pub(in crate::app) fn spawn_about_button_bevel(commands: &mut Commands, theme: &LoadedTheme) {
    let button = theme.layout.rects.about_ok;
    let center = button.center();
    let half = button.size() / 2.0;

    for (offset, size, color) in [
        (
            Vec2::new(0.0, half.y),
            Vec2::new(button.width, 2.0),
            theme.about.button_highlight,
        ),
        (
            Vec2::new(-half.x, 0.0),
            Vec2::new(2.0, button.height),
            theme.about.button_highlight,
        ),
        (
            Vec2::new(0.0, -half.y),
            Vec2::new(button.width, 2.0),
            theme.about.button_shadow,
        ),
        (
            Vec2::new(half.x, 0.0),
            Vec2::new(2.0, button.height),
            theme.about.button_shadow,
        ),
    ] {
        commands.spawn((
            Sprite::from_color(color, size),
            Transform::from_xyz(center.x + offset.x, center.y + offset.y, 3.5),
            Visibility::Hidden,
            ThemedColorSprite {
                role: if offset.x < 0.0 || offset.y > 0.0 {
                    ThemedColorSpriteRole::ButtonHighlight
                } else {
                    ThemedColorSpriteRole::ButtonShadow
                },
            },
            AboutShell,
            ScreenShell,
        ));
    }
}

pub(in crate::app) fn spawn_about_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    text: &'static str,
    position: Vec2,
    font_size: f32,
    color: Color,
    color_role: ThemedTextColorRole,
) {
    commands.spawn((
        Text2d::new(text),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextColor { role: color_role },
        ThemedTextMetrics,
        about_transform(position.x, position.y, 5.0),
        Visibility::Hidden,
        AboutShell,
        ScreenShell,
    ));
}

pub(in crate::app) fn about_transform(x: f32, y: f32, z: f32) -> Transform {
    Transform::from_xyz(x - 320.0, 334.0 - y, z)
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct MenuButtonSpec {
    pub(in crate::app) screen: ClientScreen,
    pub(in crate::app) label: &'static str,
    pub(in crate::app) center: Vec2,
    pub(in crate::app) size: Vec2,
    pub(in crate::app) action: MenuAction,
}

pub(in crate::app) fn spawn_menu_button(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
    spec: MenuButtonSpec,
) {
    let button_color = if spec.screen == ClientScreen::Startup {
        motif_button_face_color()
    } else if spec.screen == ClientScreen::About {
        theme.about.button_face
    } else if spec.screen == ClientScreen::Challenge || spec.screen == ClientScreen::Roster {
        motif_button_face_color()
    } else {
        theme.button.normal
    };
    let text_color = if spec.screen == ClientScreen::Startup {
        motif_blue_color()
    } else if spec.screen == ClientScreen::About {
        theme.about.button_text
    } else if spec.screen == ClientScreen::Challenge || spec.screen == ClientScreen::Roster {
        motif_blue_color()
    } else {
        theme.button.text
    };
    commands.spawn((
        Sprite::from_color(button_color, spec.size),
        Transform::from_xyz(spec.center.x, spec.center.y, 3.0),
        ButtonFace,
        MenuButton {
            screen: spec.screen,
            rect: Rect::from_center_size(spec.center, spec.size),
            action: spec.action,
        },
        ScreenShell,
    ));
    if spec.screen == ClientScreen::Startup {
        spawn_startup_button_bevel(commands, spec.center, spec.size);
        if matches!(spec.action, MenuAction::GoTo(ClientScreen::Challenge)) {
            spawn_startup_focus_outline(commands, spec.center, spec.size);
        }
    } else if spec.screen == ClientScreen::Challenge {
        spawn_challenge_button_bevel(commands, spec.center, spec.size);
    } else if spec.screen == ClientScreen::Roster {
        spawn_roster_bevel(commands, spec.center, spec.size, 3.5, MotifBevel::Raised);
    }
    let text_font = if spec.screen == ClientScreen::Startup
        || spec.screen == ClientScreen::Challenge
        || spec.screen == ClientScreen::Roster
    {
        themed_text_font_at_size(theme, ThemedTextFontRole::Button, 12.0, asset_server)
    } else {
        themed_text_font(theme, ThemedTextFontRole::Button, asset_server)
    };
    let mut text_entity = commands.spawn((
        Text2d::new(spec.label),
        text_font,
        TextColor(text_color),
        ThemedTextColor {
            role: if spec.screen == ClientScreen::Startup {
                ThemedTextColorRole::ScreenBody
            } else if spec.screen == ClientScreen::About {
                ThemedTextColorRole::AboutButton
            } else if spec.screen == ClientScreen::Challenge || spec.screen == ClientScreen::Roster
            {
                ThemedTextColorRole::ScreenBody
            } else {
                ThemedTextColorRole::Button
            },
        },
        ThemedTextMetrics,
        Transform::from_xyz(spec.center.x, spec.center.y, 4.0),
        MenuButton {
            screen: spec.screen,
            rect: spec.rect(),
            action: spec.action,
        },
        ScreenShell,
    ));
    if spec.screen != ClientScreen::Startup
        && spec.screen != ClientScreen::Challenge
        && spec.screen != ClientScreen::Roster
    {
        text_entity.insert(ThemedTextFont {
            role: ThemedTextFontRole::Button,
        });
    }
}

pub(in crate::app) fn spawn_startup_button_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
) {
    spawn_startup_bevel(commands, center, size, 3.5, MotifBevel::Raised);
}

pub(in crate::app) fn spawn_startup_focus_outline(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
) {
    let outline_size = size + Vec2::splat(2.0);
    for (offset, segment_size) in [
        (
            Vec2::new(0.0, outline_size.y / 2.0),
            Vec2::new(outline_size.x, 2.0),
        ),
        (
            Vec2::new(-outline_size.x / 2.0, 0.0),
            Vec2::new(2.0, outline_size.y),
        ),
        (
            Vec2::new(0.0, -outline_size.y / 2.0),
            Vec2::new(outline_size.x, 2.0),
        ),
        (
            Vec2::new(outline_size.x / 2.0, 0.0),
            Vec2::new(2.0, outline_size.y),
        ),
    ] {
        commands.spawn((
            Sprite::from_color(motif_blue_color(), segment_size),
            Transform::from_xyz(center.x + offset.x, center.y + offset.y, 3.6),
            StartupOnlyShell,
            ScreenShell,
        ));
    }
}

pub(in crate::app) fn spawn_startup_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        commands.spawn((
            Sprite::from_color(bevel_color, bevel_size),
            Transform::from_xyz(center.x + offset.x, center.y + offset.y, z),
            StartupOnlyShell,
            ScreenShell,
        ));
    }
}

impl MenuButtonSpec {
    pub(in crate::app) fn rect(self) -> Rect {
        Rect::from_center_size(self.center, self.size)
    }
}

pub(in crate::app) fn spawn_challenge_button_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
) {
    spawn_challenge_bevel(commands, center, size, 3.5, MotifBevel::Raised);
}

pub(in crate::app) fn startup_buttons(theme: &LoadedTheme) -> [MenuButtonSpec; 6] {
    let rects = theme.layout.rects;
    let gear_size = Vec2::splat(rects.startup_about.height);
    let gear_center = rects.startup_about.center()
        - Vec2::new(
            rects.startup_about.width / 2.0 + 8.0 + gear_size.x / 2.0,
            0.0,
        );
    [
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Challenge",
            center: rects.startup_challenge.center(),
            size: rects.startup_challenge.size(),
            action: MenuAction::GoTo(ClientScreen::Challenge),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Sleep",
            center: rects.startup_sleep.center(),
            size: rects.startup_sleep.size(),
            action: MenuAction::GoTo(ClientScreen::Sleep),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "*",
            center: gear_center,
            size: gear_size,
            action: MenuAction::GoTo(ClientScreen::Settings),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "About",
            center: rects.startup_about.center(),
            size: rects.startup_about.size(),
            action: MenuAction::GoTo(ClientScreen::About),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Roster",
            center: rects.startup_roster.center(),
            size: rects.startup_roster.size(),
            action: MenuAction::GoTo(ClientScreen::Roster),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Quit",
            center: rects.startup_quit.center(),
            size: rects.startup_quit.size(),
            action: MenuAction::Quit,
        },
    ]
}

pub(in crate::app) fn secondary_screen_buttons(theme: &LoadedTheme) -> [MenuButtonSpec; 8] {
    let rects = theme.layout.rects;
    [
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Challenge",
            center: rects.challenge_level_down.center(),
            size: rects.challenge_level_down.size(),
            action: MenuAction::StartSelectedChallenge,
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Update",
            center: rects.challenge_level_up.center(),
            size: rects.challenge_level_up.size(),
            action: MenuAction::GoTo(ClientScreen::Challenge),
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Play Ernie",
            center: rects.challenge_play_ernie.center(),
            size: rects.challenge_play_ernie.size(),
            action: MenuAction::StartHumanVsComputer,
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Cancel",
            center: rects.challenge_back.center(),
            size: rects.challenge_back.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Sleep,
            label: "Wake",
            center: rects.sleep_wake.center(),
            size: rects.sleep_wake.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::About,
            label: "OK",
            center: rects.about_ok.center(),
            size: rects.about_ok.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Roster,
            label: "Done",
            center: rects.roster_back.center(),
            size: rects.roster_back.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Settings,
            label: "Back",
            center: rects.settings_back.center(),
            size: rects.settings_back.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
    ]
}

pub(in crate::app) fn spawn_player_view(
    commands: &mut Commands,
    theme: &LoadedTheme,
    atlas: &ThemeAtlasImageHandle,
    player: PlayerId,
    left: f32,
    label: &str,
) {
    let width = BOARD_WIDTH as f32 * theme.cell.size;
    let height = BOARD_HEIGHT as f32 * theme.cell.size;
    let center_x = left + width / 2.0;
    let center_y = theme.layout.board.top - height / 2.0;

    commands.spawn((
        Sprite::from_color(theme.palette.board_background, Vec2::new(width, height)),
        Transform::from_xyz(center_x, center_y, -1.0),
        PlayerViewEntity { player },
        GameEntity,
    ));

    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let cell_sprite = empty_cell_sprite(theme);
            commands.spawn((
                board_cell_sprite(theme, atlas, cell_sprite),
                Transform::from_xyz(cell_x(theme, left, x), cell_y(theme, y), 0.0),
                BoardCell { player, x, y },
                PlayerViewEntity { player },
                GameEntity,
            ));
        }
    }

    let _ = (center_x, height, label);
}
