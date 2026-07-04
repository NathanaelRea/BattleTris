//! Client settings persistence and sanitization.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ControlScheme {
    Original,
}

#[derive(Resource, Debug, Clone)]
pub(super) struct ClientSettings {
    pub(super) screen: ClientScreen,
    pub(super) content_mode: ContentMode,
    pub(super) ui_style: UiStyleChoice,
    pub(super) theme: ThemeChoice,
    pub(super) sound_pack: SoundPackChoice,
    pub(super) controls: ControlScheme,
    pub(super) pixel_scale: f32,
    pub(super) ernie_level: usize,
    pub(super) challenge_style: ChallengeStyle,
    pub(super) challenge_mode: ChallengeMode,
    pub(super) display_name: String,
    pub(super) community_label: String,
    pub(super) direct_listen_addr: String,
    pub(super) direct_share_addr: String,
    pub(super) direct_join_addr: String,
    pub(super) lobby_addr: String,
    pub(super) lobby_enabled: bool,
    pub(super) hosted_ranked: bool,
    pub(super) settings_path: Option<PathBuf>,
    pub(super) assets_dir: PathBuf,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            screen: ClientScreen::Startup,
            content_mode: ContentMode::Normal,
            ui_style: UiStyleChoice::Original,
            theme: ThemeChoice::Original,
            sound_pack: SoundPackChoice::GeneratedDefault,
            controls: ControlScheme::Original,
            pixel_scale: 1.0,
            ernie_level: DEFAULT_ERNIE_LEVEL,
            challenge_style: ChallengeStyle::Legacy,
            challenge_mode: ChallengeMode::ComputerOpponent,
            display_name: default_display_name(),
            community_label: CommunityLabel::local().as_str().to_string(),
            direct_listen_addr: "0.0.0.0:4405".to_string(),
            direct_share_addr: suggested_share_addr_for("0.0.0.0:4405"),
            direct_join_addr: "127.0.0.1:4405".to_string(),
            lobby_addr: DEFAULT_LOBBY_ADDR.to_string(),
            lobby_enabled: true,
            hosted_ranked: true,
            settings_path: settings_path(),
            assets_dir: assets_dir(),
        }
    }
}

impl ClientSettings {
    pub(super) fn load_or_default() -> Self {
        let mut settings = Self::default();
        let Some(path) = &settings.settings_path else {
            return settings;
        };

        let Ok(contents) = fs::read_to_string(path) else {
            return settings;
        };

        match toml::from_str::<PersistedClientSettings>(&contents) {
            Ok(persisted) => settings.apply_persisted(persisted),
            Err(error) => warn!(
                "BattleTris settings file {} could not be parsed: {error}",
                path.display()
            ),
        }
        settings
    }

    pub(super) fn save(&self) {
        let Some(path) = &self.settings_path else {
            return;
        };

        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                warn!(
                    "BattleTris settings directory {} could not be created: {error}",
                    parent.display()
                );
                return;
            }
        }

        match toml::to_string_pretty(&self.persisted()) {
            Ok(contents) => {
                if let Err(error) = fs::write(path, contents) {
                    warn!(
                        "BattleTris settings file {} could not be written: {error}",
                        path.display()
                    );
                }
            }
            Err(error) => warn!("BattleTris settings could not be serialized: {error}"),
        }
    }

    pub(super) fn persisted(&self) -> PersistedClientSettings {
        PersistedClientSettings {
            ui_style: self.ui_style,
            theme: self.theme,
            sound_pack: self.sound_pack,
            controls: self.controls,
            pixel_scale: self.pixel_scale,
            ernie_level: self.ernie_level,
            challenge_style: self.challenge_style,
            display_name: self.display_name.clone(),
            community_label: self.community_label.clone(),
            direct_listen_addr: self.direct_listen_addr.clone(),
            direct_share_addr: self.direct_share_addr.clone(),
            direct_join_addr: self.direct_join_addr.clone(),
            lobby_addr: self.lobby_addr.clone(),
            hosted_ranked: self.hosted_ranked,
        }
    }

    pub(super) fn apply_persisted(&mut self, persisted: PersistedClientSettings) {
        self.ui_style = persisted.ui_style;
        self.theme = persisted.theme;
        self.sound_pack = persisted.sound_pack;
        self.controls = persisted.controls;
        self.pixel_scale = sanitize_pixel_scale(persisted.pixel_scale);
        self.ernie_level = sanitize_ernie_level(persisted.ernie_level);
        self.challenge_style = persisted.challenge_style;
        self.display_name =
            sanitize_nonempty_setting(persisted.display_name, default_display_name());
        self.community_label =
            sanitize_nonempty_setting(persisted.community_label, "local".to_string());
        self.direct_listen_addr =
            sanitize_socket_setting(persisted.direct_listen_addr, "0.0.0.0:4405");
        self.direct_share_addr =
            sanitize_share_addr_setting(persisted.direct_share_addr, &self.direct_listen_addr);
        self.direct_join_addr =
            sanitize_socket_setting(persisted.direct_join_addr, "127.0.0.1:4405");
        self.lobby_addr = sanitize_socket_setting(persisted.lobby_addr, DEFAULT_LOBBY_ADDR);
        self.hosted_ranked = persisted.hosted_ranked;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub(super) struct PersistedClientSettings {
    pub(super) ui_style: UiStyleChoice,
    pub(super) theme: ThemeChoice,
    pub(super) sound_pack: SoundPackChoice,
    pub(super) controls: ControlScheme,
    pub(super) pixel_scale: f32,
    pub(super) ernie_level: usize,
    pub(super) challenge_style: ChallengeStyle,
    pub(super) display_name: String,
    pub(super) community_label: String,
    pub(super) direct_listen_addr: String,
    pub(super) direct_share_addr: String,
    pub(super) direct_join_addr: String,
    pub(super) lobby_addr: String,
    pub(super) hosted_ranked: bool,
}

impl Default for PersistedClientSettings {
    fn default() -> Self {
        Self {
            ui_style: UiStyleChoice::Original,
            theme: ThemeChoice::Original,
            sound_pack: SoundPackChoice::GeneratedDefault,
            controls: ControlScheme::Original,
            pixel_scale: 1.0,
            ernie_level: DEFAULT_ERNIE_LEVEL,
            challenge_style: ChallengeStyle::Legacy,
            display_name: default_display_name(),
            community_label: "local".to_string(),
            direct_listen_addr: "0.0.0.0:4405".to_string(),
            direct_share_addr: suggested_share_addr_for("0.0.0.0:4405"),
            direct_join_addr: "127.0.0.1:4405".to_string(),
            lobby_addr: DEFAULT_LOBBY_ADDR.to_string(),
            hosted_ranked: true,
        }
    }
}

pub(super) fn log_content_mode(settings: Res<ClientSettings>, themes: Res<ThemePacks>) {
    let theme = themes.get(settings.theme);
    info!(
        "BattleTris content mode: {}; Gimp sprite: {}",
        settings.content_mode.id(),
        theme.sprites.gimp_for(settings.content_mode)
    );
}

pub(super) fn sanitize_pixel_scale(pixel_scale: f32) -> f32 {
    if pixel_scale.is_finite() {
        pixel_scale.clamp(0.75, 2.0)
    } else {
        1.0
    }
}

pub(super) fn sanitize_nonempty_setting(value: String, fallback: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed.to_string()
    }
}

pub(super) fn sanitize_socket_setting(value: String, fallback: &str) -> String {
    let trimmed = sanitize_nonempty_setting(value, fallback.to_string());
    if trimmed.parse::<SocketAddr>().is_ok() {
        trimmed
    } else {
        fallback.to_string()
    }
}

pub(super) fn sanitize_share_addr_setting(value: String, bind_addr: &str) -> String {
    let fallback = suggested_share_addr_for(bind_addr);
    let sanitized = sanitize_socket_setting(value, &fallback);
    if socket_addr_is_unspecified(&sanitized) {
        fallback
    } else {
        sanitized
    }
}

pub(super) fn socket_addr_is_unspecified(value: &str) -> bool {
    value
        .parse::<SocketAddr>()
        .map(|addr| addr.ip().is_unspecified())
        .unwrap_or(false)
}

pub(super) fn suggested_share_addr_for(bind_addr: &str) -> String {
    let bind = bind_addr
        .parse::<SocketAddr>()
        .unwrap_or_else(|_| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4405));
    if !bind.ip().is_unspecified() {
        return bind.to_string();
    }
    SocketAddr::new(suggest_lan_ip(), bind.port()).to_string()
}

pub(super) fn suggest_lan_ip() -> IpAddr {
    UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
        .and_then(|socket| {
            let _ = socket.connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 80));
            socket.local_addr()
        })
        .map(|addr| addr.ip())
        .ok()
        .filter(|ip| !ip.is_unspecified())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

pub(super) fn settings_field_value(settings: &ClientSettings, field: SettingsField) -> &str {
    match field {
        SettingsField::DisplayName => &settings.display_name,
        SettingsField::CommunityLabel => &settings.community_label,
        SettingsField::HostBindAddress => &settings.direct_listen_addr,
        SettingsField::ShareAddress => &settings.direct_share_addr,
        SettingsField::JoinAddress => &settings.direct_join_addr,
        SettingsField::LobbyAddress => &settings.lobby_addr,
    }
}

pub(super) fn settings_field_value_mut(
    settings: &mut ClientSettings,
    field: SettingsField,
) -> &mut String {
    match field {
        SettingsField::DisplayName => &mut settings.display_name,
        SettingsField::CommunityLabel => &mut settings.community_label,
        SettingsField::HostBindAddress => &mut settings.direct_listen_addr,
        SettingsField::ShareAddress => &mut settings.direct_share_addr,
        SettingsField::JoinAddress => &mut settings.direct_join_addr,
        SettingsField::LobbyAddress => &mut settings.lobby_addr,
    }
}

pub(super) fn sanitize_settings_after_edit(settings: &mut ClientSettings, field: SettingsField) {
    match field {
        SettingsField::DisplayName => {
            settings.display_name = sanitize_nonempty_setting(
                std::mem::take(&mut settings.display_name),
                default_display_name(),
            );
        }
        SettingsField::CommunityLabel => {
            settings.community_label = sanitize_nonempty_setting(
                std::mem::take(&mut settings.community_label),
                "local".to_string(),
            );
        }
        SettingsField::HostBindAddress => {
            settings.direct_listen_addr = sanitize_socket_setting(
                std::mem::take(&mut settings.direct_listen_addr),
                "0.0.0.0:4405",
            );
            if socket_addr_is_unspecified(&settings.direct_share_addr) {
                settings.direct_share_addr = suggested_share_addr_for(&settings.direct_listen_addr);
            }
        }
        SettingsField::ShareAddress => {
            settings.direct_share_addr = sanitize_share_addr_setting(
                std::mem::take(&mut settings.direct_share_addr),
                &settings.direct_listen_addr,
            );
        }
        SettingsField::JoinAddress => {
            settings.direct_join_addr = sanitize_socket_setting(
                std::mem::take(&mut settings.direct_join_addr),
                "127.0.0.1:4405",
            );
        }
        SettingsField::LobbyAddress => {
            settings.lobby_addr = sanitize_socket_setting(
                std::mem::take(&mut settings.lobby_addr),
                DEFAULT_LOBBY_ADDR,
            );
        }
    }
}

pub(super) fn sanitize_ernie_level(level: usize) -> usize {
    level.min(COMPUTER_DIFFICULTIES.len() - 1)
}

pub(super) fn adjust_ernie_level(settings: &mut ClientSettings, step: isize) {
    let max = COMPUTER_DIFFICULTIES.len() as isize - 1;
    settings.ernie_level = (settings.ernie_level as isize + step).clamp(0, max) as usize;
    settings.save();
}

pub(super) fn toggle_theme(settings: &mut ClientSettings) {
    settings.theme = match settings.theme {
        ThemeChoice::Original => ThemeChoice::HighContrast,
        ThemeChoice::HighContrast => ThemeChoice::Original,
    };
}

pub(super) fn selected_ernie_difficulty(
    settings: &ClientSettings,
) -> battletris_core::ai::ComputerDifficulty {
    computer_difficulty(settings.ernie_level).expect("sanitized legacy AI difficulty exists")
}

pub(super) fn default_display_name() -> String {
    std::env::var("BATTLETRIS_DISPLAY_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("USER").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Local Player".to_string())
}

pub(super) fn lobby_registration_preview(settings: &ClientSettings) -> LobbyRegister {
    LobbyRegister {
        player: HostedPlayer {
            player_id: player_id_from_display_name(&settings.display_name),
            display_name: settings.display_name.clone(),
        },
        direct_addr: settings.direct_share_addr.clone(),
        ranked: settings.hosted_ranked,
    }
}

pub(super) fn hosted_player(settings: &ClientSettings) -> HostedPlayer {
    lobby_registration_preview(settings).player
}

pub(super) fn hosted_player_id(settings: &ClientSettings) -> String {
    player_id_from_display_name(&settings.display_name)
}

pub(super) fn player_id_from_display_name(display_name: &str) -> String {
    let mut id = String::new();
    for ch in display_name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
        } else if (ch.is_ascii_whitespace() || ch == '-' || ch == '_') && !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_matches('-');
    if id.is_empty() {
        "local-player".to_string()
    } else {
        id.to_string()
    }
}

pub(super) fn settings_path() -> Option<PathBuf> {
    select_settings_path(settings_file_candidates(), project_settings_path())
}

pub(super) fn select_settings_path(
    local_candidates: Vec<PathBuf>,
    project_path: Option<PathBuf>,
) -> Option<PathBuf> {
    local_candidates
        .into_iter()
        .find(|path| path.is_file())
        .or(project_path)
}

pub(super) fn settings_file_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        push_settings_candidate(&mut candidates, current_dir.join(SETTINGS_FILE_NAME));
    }
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    push_settings_candidate(&mut candidates, crate_dir.join(SETTINGS_FILE_NAME));
    push_settings_candidate(
        &mut candidates,
        crate_dir.join("../..").join(SETTINGS_FILE_NAME),
    );
    candidates
}

pub(super) fn push_settings_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    if !candidates.iter().any(|candidate| candidate == &path) {
        candidates.push(path);
    }
}

pub(super) fn project_settings_path() -> Option<PathBuf> {
    ProjectDirs::from("org", "BattleTris", "BattleTris")
        .map(|dirs| dirs.config_dir().join(SETTINGS_FILE_NAME))
}

pub(super) fn assets_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("BATTLETRIS_ASSETS_DIR") {
        return PathBuf::from(path);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(package_root) = exe_path.parent().and_then(|bin_dir| bin_dir.parent()) {
            let packaged_assets = package_root.join("assets");
            if packaged_assets.join("manifest.toml").is_file() {
                return packaged_assets;
            }
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets")
}
