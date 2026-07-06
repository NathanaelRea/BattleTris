//! UI component markers and shared UI-only types.

use super::*;

#[derive(Component)]
pub(in crate::app) struct BoardCell {
    pub(in crate::app) player: PlayerId,
    pub(in crate::app) x: usize,
    pub(in crate::app) y: usize,
}

#[derive(Component)]
pub(in crate::app) struct HudText {
    pub(in crate::app) player: PlayerId,
}

#[derive(Component)]
pub(in crate::app) struct PhaseText;

#[derive(Component)]
pub(in crate::app) struct PlayingGameEntity;

#[derive(Component)]
pub(in crate::app) struct LegacyGameText {
    pub(in crate::app) role: LegacyGameTextRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum LegacyGameTextRole {
    Score,
    ArsenalSlot(usize),
    Message,
}

#[derive(Component)]
pub(in crate::app) struct MenuText;

#[derive(Component)]
pub(in crate::app) struct GameEntity;

#[derive(Component)]
pub(in crate::app) struct BazaarEntity;

#[derive(Component)]
pub(in crate::app) struct BazaarText {
    pub(in crate::app) role: BazaarTextRole,
}

#[derive(Component)]
pub(in crate::app) struct BazaarSelectionMarker {
    pub(in crate::app) role: BazaarSelectionMarkerRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum BazaarSelectionMarkerRole {
    Background,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum BazaarTextRole {
    Catalog,
    SelectedCatalogRow,
    Funds,
    ArsenalSlot(usize),
    Message,
    Description,
}

#[derive(Component)]
pub(in crate::app) struct PlayerViewEntity {
    pub(in crate::app) player: PlayerId,
}

#[derive(Component)]
pub(in crate::app) struct ScreenShell;

#[derive(Component)]
pub(in crate::app) struct ScreenText;

#[derive(Component)]
pub(in crate::app) struct GenericScreenShell;

#[derive(Component)]
pub(in crate::app) struct SettingsUiRoot;

#[derive(Component)]
pub(in crate::app) struct SettingsUiBackground;

#[derive(Component)]
pub(in crate::app) struct SettingsUiBiffImage;

#[derive(Component)]
pub(in crate::app) struct SettingsUiTitleText;

#[derive(Component)]
pub(in crate::app) struct SettingsUiBackButton;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct SettingsUiControlButton {
    pub(in crate::app) control: SettingsControl,
    pub(in crate::app) action: SettingsUiAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum SettingsUiAction {
    Focus,
    Activate,
    ToggleDropdown,
    Select(SettingsSelectOption),
    Decrement,
    Increment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum SettingsSelectOption {
    UiStyle(UiStyleChoice),
    Theme(ThemeChoice),
    SoundPack(SoundPackChoice),
    ChallengeStyle(ChallengeStyle),
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct SettingsUiDropdownMenu {
    pub(in crate::app) control: SettingsControl,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct SettingsUiSurface {
    pub(in crate::app) role: SettingsUiSurfaceRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum SettingsUiSurfaceRole {
    Background,
    Panel,
    Dropdown,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct SettingsUiTextColor {
    pub(in crate::app) role: SettingsUiTextColorRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum SettingsUiTextColorRole {
    Title,
    Section,
    Label,
    Value,
    Hint,
    Status,
    Button,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct SettingsUiValueText {
    pub(in crate::app) value: SettingsUiValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum SettingsUiValue {
    UiStyle,
    Theme,
    SoundPack,
    ChallengeStyle,
    HostedRanked,
    PixelScale,
    Field(SettingsField),
}

#[derive(Component)]
pub(in crate::app) struct SettingsUiStatusText;

pub(in crate::app) type SettingsUiBackButtonVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (With<SettingsUiBackButton>, Without<SettingsUiControlButton>),
>;

#[derive(Component)]
pub(in crate::app) struct StartupOnlyShell;

#[derive(Component)]
pub(in crate::app) struct AboutShell;

#[derive(Component)]
pub(in crate::app) struct ChallengeShell;

#[derive(Component)]
pub(in crate::app) struct RosterShell;

#[derive(Component)]
pub(in crate::app) struct ChallengeLogo;

#[derive(Component)]
pub(in crate::app) struct ChallengeSliderKnob {
    pub(in crate::app) x_offset: f32,
}

#[derive(Default)]
pub(in crate::app) struct ChallengeLogoTextureCache {
    pub(in crate::app) original: Option<Handle<Image>>,
    pub(in crate::app) high_contrast: Option<Handle<Image>>,
}

impl ChallengeLogoTextureCache {
    pub(in crate::app) fn get(&self, theme: ThemeChoice) -> Option<Handle<Image>> {
        match theme {
            ThemeChoice::Original => self.original.clone(),
            ThemeChoice::HighContrast => self.high_contrast.clone(),
        }
    }

    pub(in crate::app) fn set(&mut self, theme: ThemeChoice, handle: Handle<Image>) {
        match theme {
            ThemeChoice::Original => self.original = Some(handle),
            ThemeChoice::HighContrast => self.high_contrast = Some(handle),
        }
    }
}

#[derive(Component)]
pub(in crate::app) struct ChallengeText {
    pub(in crate::app) role: ChallengeTextRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ChallengeTextRole {
    UserList,
    UserInfo,
    ComputerStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum BazaarWaitingText {
    LocalWaiting,
    LocalRepeated,
    PlayerWaiting(PlayerId),
    PlayerRepeated(PlayerId),
}

pub(in crate::app) struct UiTextTone;

impl UiTextTone {
    pub(in crate::app) fn challenge_copy(content_mode: ContentMode) -> &'static str {
        match content_mode {
            ContentMode::Normal => "",
            ContentMode::Rated => "wants a piece of yo' ass.",
        }
    }

    pub(in crate::app) fn bazaar_waiting_copy(
        content_mode: ContentMode,
        text: BazaarWaitingText,
    ) -> String {
        match (content_mode, text) {
            (ContentMode::Rated, BazaarWaitingText::LocalWaiting)
            | (ContentMode::Rated, BazaarWaitingText::PlayerWaiting(_)) => {
                "Waiting for fat slut...".to_string()
            }
            (ContentMode::Rated, BazaarWaitingText::LocalRepeated)
            | (ContentMode::Rated, BazaarWaitingText::PlayerRepeated(_)) => {
                "Fuckface is getting angsty.".to_string()
            }
            (ContentMode::Normal, BazaarWaitingText::LocalWaiting) => {
                "Done. Waiting for opponent.".to_string()
            }
            (ContentMode::Normal, BazaarWaitingText::LocalRepeated) => {
                "Already waiting for opponent.".to_string()
            }
            (ContentMode::Normal, BazaarWaitingText::PlayerWaiting(player)) => {
                format!("{} done. Waiting for opponent.", player_label(player))
            }
            (ContentMode::Normal, BazaarWaitingText::PlayerRepeated(player)) => {
                format!("{} is already waiting.", player_label(player))
            }
        }
    }

    pub(in crate::app) fn bazaar_done_overlay_copy(content_mode: ContentMode) -> &'static str {
        match content_mode {
            ContentMode::Normal => {
                "Done selected. Waiting for opponent; shopping controls are dimmed."
            }
            ContentMode::Rated => "Waiting for fat slut...",
        }
    }

    pub(in crate::app) fn bazaar_instructions_copy(content_mode: ContentMode) -> &'static str {
        match content_mode {
            ContentMode::Normal => "Click a row to inspect. Click Add/Remove/DONE. Number slots launch in game, remove staged here.",
            ContentMode::Rated => "Click a row to inspect. Click Add/Remove/DONE. Number slots launch in game, remove staged here.",
        }
    }

    pub(in crate::app) fn game_result_copy(
        content_mode: ContentMode,
        local_won: Option<bool>,
    ) -> &'static str {
        match (content_mode, local_won) {
            (ContentMode::Rated, Some(false)) => "Nice loss, shithead.",
            (ContentMode::Rated, Some(true)) => "Yer the shit!",
            (ContentMode::Normal, _) | (ContentMode::Rated, None) => "Game over",
        }
    }
}

#[derive(Component)]
pub(in crate::app) struct RosterText {
    pub(in crate::app) role: RosterTextRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum RosterTextRole {
    UserList,
    UserInfo1,
    UserInfo2,
    Player1Name,
    Player2Name,
    Player1Score,
    Player2Score,
}

#[derive(Component)]
pub(in crate::app) struct ButtonFace;

#[derive(Component)]
pub(in crate::app) struct ThemedSprite {
    pub(in crate::app) role: ThemedSpriteRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ThemedSpriteRole {
    Startup,
    Bazaar,
    Biff,
    AboutIcon,
}

#[derive(Component)]
pub(in crate::app) struct ThemedTextColor {
    pub(in crate::app) role: ThemedTextColorRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ThemedTextColorRole {
    Secondary,
    ScreenTitle,
    ScreenBody,
    Button,
    AboutTitle,
    AboutName,
    AboutCredit,
    AboutButton,
}

#[derive(Component)]
pub(in crate::app) struct ThemedTextFont {
    pub(in crate::app) role: ThemedTextFontRole,
}

#[derive(Component)]
pub(in crate::app) struct ThemedTextMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ThemedTextFontRole {
    Title,
    Body,
    Button,
    Mono,
}

#[derive(Component)]
pub(in crate::app) struct ThemedColorSprite {
    pub(in crate::app) role: ThemedColorSpriteRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ThemedColorSpriteRole {
    ScreenBackground,
    AboutBackground,
    ButtonHighlight,
    ButtonShadow,
}

#[derive(Component)]
pub(in crate::app) struct MenuButton {
    pub(in crate::app) screen: ClientScreen,
    pub(in crate::app) rect: Rect,
    pub(in crate::app) action: MenuAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum MenuAction {
    StartSelectedChallenge,
    UpdateChallenge,
    StartHumanVsComputer,
    GoTo(ClientScreen),
    Quit,
}
