//! Bevy UI module split by UI responsibility.

use super::*;

mod components;
mod input;
mod labels;
mod render;
mod spawn;

#[allow(unused_imports)]
pub(super) use components::{
    AboutShell, BazaarEntity, BazaarSelectionMarker, BazaarSelectionMarkerRole, BazaarText,
    BazaarTextRole, BazaarWaitingText, BoardCell, ButtonFace, ChallengeLogo,
    ChallengeLogoTextureCache, ChallengeShell, ChallengeSliderKnob, ChallengeText,
    ChallengeTextRole, GameEntity, GenericScreenShell, HudText, LegacyGameText, LegacyGameTextRole,
    MenuAction, MenuButton, MenuText, PhaseText, PlayerViewEntity, PlayingGameEntity, RosterShell,
    RosterText, RosterTextRole, ScreenShell, ScreenText, SettingsSelectOption, SettingsUiAction,
    SettingsUiBackButton, SettingsUiBackButtonVisualQuery, SettingsUiBackground,
    SettingsUiBiffImage, SettingsUiControlButton, SettingsUiDropdownMenu, SettingsUiRoot,
    SettingsUiStatusText, SettingsUiSurface, SettingsUiSurfaceRole, SettingsUiTextColor,
    SettingsUiTextColorRole, SettingsUiTitleText, SettingsUiValue, SettingsUiValueText,
    StartupOnlyShell, ThemedColorSprite, ThemedColorSpriteRole, ThemedSprite, ThemedSpriteRole,
    ThemedTextColor, ThemedTextColorRole, ThemedTextFont, ThemedTextFontRole, ThemedTextMetrics,
    UiTextTone,
};
#[allow(unused_imports)]
pub(super) use input::{
    accept_pending_direct_challenge, accept_pending_hosted_challenge, activate_settings_control,
    adjacent_catalog_token, adjust_settings_pixel_scale, apply_menu_action,
    apply_settings_select_option, arsenal_slot_label, bazaar_add_rect, bazaar_arsenal_token_at,
    bazaar_catalog_keys, bazaar_catalog_token_at, bazaar_done_rect, bazaar_remove_rect,
    browse_hosted_lobby, browse_lan, buy_bazaar_weapon, buy_selected_bazaar_weapon,
    cancel_hosted_registration, cancel_network_challenge, challenge_entry_index_at_world,
    controls_for, deny_pending_direct_challenge, direct_accept_seed, direct_identity,
    handle_bazaar_click, handle_bazaar_input, handle_challenge_input,
    handle_focused_settings_control, handle_game_input, handle_keyboard_input,
    handle_mouse_buttons, handle_screen_shortcuts, handle_settings_input, handle_settings_shortcut,
    handle_settings_ui_interactions, handle_sleep_input, handle_startup_input,
    host_direct_challenge, host_via_lobby_challenge, hosted_start_for_accept,
    join_direct_challenge, join_hosted_direct_after_start, leave_network_game,
    lobby_entry_for_session, network_operation_can_cancel, next_settings_control,
    parse_network_addr, poll_registered_hosted_status, protocol_slot_for_player, queue_sound,
    register_hosted_lobby, remove_selected_bazaar_weapon, schedule_network_input,
    select_challenge_entry, select_challenge_entry_at_world, select_lan_entry,
    select_lan_entry_at_world, select_lobby_entry, select_lobby_entry_at_world, selected_lan_entry,
    selected_lobby_entry, send_fast_drop, send_network_bazaar_buy, send_network_bazaar_done,
    send_network_bazaar_remove, send_network_fast_drop, send_network_player_controls,
    send_network_press_command, send_network_repeat_command, send_player_controls,
    send_press_command, send_repeat_command, settings_ui_status_label, settings_ui_value_label,
    slot_keys, slot_label_to_index, staged_slot_index_for_token, start_lan_advertising,
    start_legacy_challenge_mode, start_legacy_host_challenge, start_legacy_join_challenge,
    start_or_browse_hosted_lobby, start_or_browse_lan, start_selected_challenge_mode,
    start_selected_hosted_game, start_sleep_availability, text_entry_character, text_entry_keys,
    update_settings_ui_dropdown_visibility, update_settings_ui_text, update_settings_ui_theme,
    update_settings_ui_visibility, update_settings_ui_visuals, GameInputContext,
    KeyboardInputParams, MouseButtonParams, PlayerControls,
};
#[allow(unused_imports)]
pub(super) use labels::{
    active_effects_label, arsenal_label, arsenal_slots_label, bazaar_arsenal_slot_widget_label,
    bazaar_catalog_label, bazaar_catalog_widget_label, bazaar_description_widget_label,
    bazaar_message_widget_label, bazaar_selection_marker_y, bazaar_text_label,
    browse_lan_panel_label, browse_lobby_panel_label, cell_sprite, cell_x, cell_y,
    challenge_compact_status_label, challenge_ernie_slider_x, challenge_label,
    challenge_mode_panel_label, challenge_network_status_label, challenge_opponent_list_label,
    challenge_primary_button_label, challenge_screen_body_label, challenge_status_lifecycle_label,
    client_player_index, computer_challenge_panel_label, controls_label,
    effective_direct_share_addr, empty_cell_sprite, host_direct_panel_label,
    host_via_lobby_panel_label, hosted_roster_rows, hosted_roster_text_label,
    hosted_roster_user_info_label, incoming_challenge_panel_label, join_direct_panel_label,
    latest_weapon_feedback, legacy_arsenal_slot_label, legacy_challenge_info_panel_label,
    legacy_challenge_player_list_label, legacy_game_message_label, legacy_game_text_label,
    legacy_host_panel_label, legacy_join_panel_label, legacy_score_label,
    legacy_transport_status_label, lobby_status_label, local_game_result_for, menu_label,
    network_session_status_label, opponent_player, phase_label, piece_label, player_hud,
    player_label, recon_hud, roster_duration_label, roster_player_name_label, roster_text_label,
    roster_user_info_label, roster_user_list_label, screen_body_label, short_weapon_name,
    sleep_network_status_label, sorted_weapon_catalog, streak_label, truncate_label,
    wrap_bazaar_description,
};
#[allow(unused_imports)]
pub(super) use render::{
    active_window_layout, board_cell_sprite, player_view_visible, render_cell_sprite, render_game,
    report_startup_render_health, update_menu_button_visuals, update_screen_visibility,
    update_window_layout, BazaarSelectionMarkerQuery, BazaarTextQuery, ChallengeSliderKnobQuery,
    ChallengeTextQuery, GameVisibilityQuery, HudTextQuery, LegacyGameTextQuery,
    MenuButtonTextQuery, MenuTextSingle, PhaseTextSingle, RenderGameParams, RenderedCellSprite,
    RosterTextQuery, ScreenTextSingle, ShellVisibilityQuery, TextMetricsQuery,
};
#[allow(unused_imports)]
pub(super) use spawn::{
    about_transform, bazaar_rect, bazaar_text_font_role, bazaar_world, challenge_point,
    challenge_rect, challenge_rect_center, challenge_screen_rect, challenge_screen_world,
    challenge_world, game_screen_rect, game_screen_world, legacy_game_text_font_role,
    legacy_scrollbar_parts, motif_blue_color, motif_button_face_color, motif_button_hover_color,
    motif_button_pressed_color, motif_dim_text_color, motif_highlight_color,
    motif_message_green_color, motif_page_color, motif_red3_color, motif_shadow_color,
    motif_text_panel_color, roster_rect, roster_world, secondary_screen_buttons,
    settings_ui_back_button_background, settings_ui_back_button_border,
    settings_ui_back_button_node, settings_ui_button_node, settings_ui_control_background,
    settings_ui_control_border, settings_ui_control_is_field_like, settings_ui_motif_border,
    settings_ui_page_background, settings_ui_surface_style, settings_ui_text,
    settings_ui_text_color, setup, spawn_about_button_bevel, spawn_about_shell, spawn_about_text,
    spawn_bazaar_arrow_button, spawn_bazaar_arrow_glyph, spawn_bazaar_bevel,
    spawn_bazaar_dynamic_text, spawn_bazaar_legacy_scrollbar, spawn_bazaar_overlay,
    spawn_bazaar_panel, spawn_bazaar_rect, spawn_bazaar_scrollbar, spawn_bazaar_scrollbar_panel,
    spawn_bazaar_static_text, spawn_challenge_arrow_button, spawn_challenge_arrow_glyph,
    spawn_challenge_bevel, spawn_challenge_button_bevel, spawn_challenge_checkbox,
    spawn_challenge_computer_frame, spawn_challenge_etched_frame_screen,
    spawn_challenge_horizontal_segments, spawn_challenge_panel, spawn_challenge_rect,
    spawn_challenge_screen_rect, spawn_challenge_scrollbar, spawn_challenge_shell,
    spawn_challenge_slider, spawn_challenge_slider_knob, spawn_challenge_slider_knob_rect,
    spawn_challenge_text, spawn_game_bevel, spawn_game_panel, spawn_game_rect,
    spawn_legacy_game_hud, spawn_legacy_game_text, spawn_menu_button, spawn_player_view,
    spawn_roster_arrow_button, spawn_roster_arrow_glyph, spawn_roster_bevel,
    spawn_roster_dynamic_label, spawn_roster_dynamic_text, spawn_roster_panel, spawn_roster_rect,
    spawn_roster_scrollbar, spawn_roster_shell, spawn_roster_static_button,
    spawn_roster_static_label, spawn_screen_shell, spawn_settings_checkbox_ui_row,
    spawn_settings_control_row, spawn_settings_dropdown_option, spawn_settings_dropdown_options,
    spawn_settings_dropdown_ui_row, spawn_settings_scale_ui_row, spawn_settings_text_input_ui_row,
    spawn_settings_ui_shell, spawn_startup_bevel, spawn_startup_button_bevel,
    spawn_startup_focus_outline, spawn_static_challenge_text, startup_buttons, ChallengeScreenRect,
    LegacyScrollbarParts, MenuButtonSpec, MotifArrowDirection, MotifBevel, LEGACY_SCROLLBAR_INSET,
};
