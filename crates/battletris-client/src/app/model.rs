//! Shared client app choices and UI edit state.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClientScreen {
    Startup,
    Game,
    Challenge,
    Sleep,
    About,
    Roster,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ThemeChoice {
    Original,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SoundPackChoice {
    GeneratedDefault,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContentMode {
    Normal,
    Rated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChallengeMode {
    ComputerOpponent,
    HostDirect,
    JoinDirect,
    HostViaLobby,
    BrowseLobby,
    BrowseLan,
}

impl ChallengeMode {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::ComputerOpponent => "Computer Opponent",
            Self::HostDirect => "Host Direct",
            Self::JoinDirect => "Join Direct",
            Self::HostViaLobby => "Host Via Lobby",
            Self::BrowseLobby => "Browse Lobby",
            Self::BrowseLan => "Browse LAN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ChallengeStyle {
    Legacy,
    Modern,
}

impl ChallengeStyle {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Legacy => "Legacy",
            Self::Modern => "Modern",
        }
    }

    pub(super) const fn toggled(self) -> Self {
        match self {
            Self::Legacy => Self::Modern,
            Self::Modern => Self::Legacy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UiStyleChoice {
    Original,
    Modern,
}

impl UiStyleChoice {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Original => "Original",
            Self::Modern => "Modern",
        }
    }

    pub(super) const fn toggled(self) -> Self {
        match self {
            Self::Original => Self::Modern,
            Self::Modern => Self::Original,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsField {
    DisplayName,
    CommunityLabel,
    HostBindAddress,
    ShareAddress,
    JoinAddress,
    ModernServerAddress,
    LegacyServerAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsControl {
    UiStyle,
    Theme,
    SoundPack,
    ChallengeStyle,
    HostedRanked,
    PixelScale,
    Text(SettingsField),
}

impl SettingsControl {
    pub(super) const ALL: [Self; 13] = [
        Self::UiStyle,
        Self::Theme,
        Self::SoundPack,
        Self::ChallengeStyle,
        Self::HostedRanked,
        Self::PixelScale,
        Self::Text(SettingsField::DisplayName),
        Self::Text(SettingsField::CommunityLabel),
        Self::Text(SettingsField::HostBindAddress),
        Self::Text(SettingsField::ShareAddress),
        Self::Text(SettingsField::JoinAddress),
        Self::Text(SettingsField::ModernServerAddress),
        Self::Text(SettingsField::LegacyServerAddress),
    ];

    pub(super) const fn text_field(self) -> Option<SettingsField> {
        match self {
            Self::Text(field) => Some(field),
            Self::UiStyle
            | Self::Theme
            | Self::SoundPack
            | Self::ChallengeStyle
            | Self::HostedRanked
            | Self::PixelScale => None,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct SettingsEditState {
    pub(super) control: SettingsControl,
    pub(super) field: SettingsField,
    pub(super) open_dropdown: Option<SettingsControl>,
}

impl Default for SettingsEditState {
    fn default() -> Self {
        Self {
            control: SettingsControl::UiStyle,
            field: SettingsField::DisplayName,
            open_dropdown: None,
        }
    }
}

impl SettingsEditState {
    pub(super) fn set_focused_control(&mut self, control: SettingsControl) {
        self.control = control;
        if let Some(field) = control.text_field() {
            self.field = field;
        }
    }

    pub(super) fn focus(&mut self, control: SettingsControl) {
        self.set_focused_control(control);
        self.open_dropdown = None;
    }

    pub(super) fn toggle_dropdown(&mut self, control: SettingsControl) {
        self.set_focused_control(control);
        self.open_dropdown = (self.open_dropdown != Some(control)).then_some(control);
    }

    pub(super) fn close_dropdown(&mut self) {
        self.open_dropdown = None;
    }
}

impl ContentMode {
    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Rated => "rated",
        }
    }
}

impl SoundPackChoice {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::GeneratedDefault => "Generated Default",
            Self::Muted => "Muted",
        }
    }

    pub(super) const fn toggled(self) -> Self {
        match self {
            Self::GeneratedDefault => Self::Muted,
            Self::Muted => Self::GeneratedDefault,
        }
    }

    pub(super) const fn directory(self) -> &'static str {
        match self {
            Self::GeneratedDefault => "generated-default",
            Self::Muted => "muted",
        }
    }
}

impl ThemeChoice {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Original => "Original",
            Self::HighContrast => "High Contrast",
        }
    }

    pub(super) const fn directory(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::HighContrast => "high-contrast",
        }
    }

    pub(super) fn from_id(value: &str) -> Option<Self> {
        [Self::Original, Self::HighContrast]
            .into_iter()
            .find(|choice| choice.directory() == value)
    }
}
