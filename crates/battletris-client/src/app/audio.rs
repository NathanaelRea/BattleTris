//! Sound pack loading and semantic sound event mapping.

use super::*;

#[derive(Resource, Debug, Clone)]
pub(super) struct SoundPacks {
    pub(super) generated_default: LoadedSoundPack,
    pub(super) generated_rated: LoadedSoundPack,
}

impl SoundPacks {
    pub(super) fn load(assets_dir: &std::path::Path) -> Self {
        Self {
            generated_default: LoadedSoundPack::load(assets_dir, SoundPackChoice::GeneratedDefault),
            generated_rated: LoadedSoundPack::load_overlay(assets_dir, "generated-rated"),
        }
    }

    pub(super) fn sound_for(
        &self,
        choice: SoundPackChoice,
        content_mode: ContentMode,
        event: SoundEvent,
    ) -> Option<&LoadedSoundEvent> {
        match (choice, content_mode) {
            (SoundPackChoice::GeneratedDefault, ContentMode::Rated) => self
                .generated_rated
                .event(event)
                .or_else(|| self.generated_default.event(event)),
            (SoundPackChoice::GeneratedDefault, ContentMode::Normal) => {
                self.generated_default.event(event)
            }
            (SoundPackChoice::Muted, _) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LoadedSoundPack {
    pub(super) events: Vec<LoadedSoundEvent>,
}

impl LoadedSoundPack {
    pub(super) fn load(assets_dir: &std::path::Path, choice: SoundPackChoice) -> Self {
        Self::load_from_dir(assets_dir, choice.directory(), true)
    }

    pub(super) fn load_overlay(assets_dir: &std::path::Path, directory: &'static str) -> Self {
        Self::load_from_dir(assets_dir, directory, false)
    }

    pub(super) fn load_from_dir(
        assets_dir: &std::path::Path,
        directory: &'static str,
        require_all_events: bool,
    ) -> Self {
        let sound_dir = assets_dir.join("sounds").join(directory);
        let manifest_path = sound_dir.join("sound-pack.toml");
        let contents = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
            panic!(
                "BattleTris sound-pack manifest {} could not be read: {error}",
                manifest_path.display()
            )
        });
        let raw: RawSoundPack = toml::from_str(&contents).unwrap_or_else(|error| {
            panic!(
                "BattleTris sound-pack manifest {} could not be parsed: {error}",
                manifest_path.display()
            )
        });
        raw.validate(&sound_dir, &manifest_path, require_all_events);
        let prefix = format!("sounds/{directory}/");
        Self {
            events: raw
                .event
                .into_iter()
                .filter_map(|event| {
                    let kind = SoundEvent::from_id(&event.id)?;
                    Some(LoadedSoundEvent {
                        kind,
                        file: format!("{prefix}{}", event.files[0]),
                    })
                })
                .collect(),
        }
    }

    pub(super) fn event(&self, kind: SoundEvent) -> Option<&LoadedSoundEvent> {
        self.events.iter().find(|event| event.kind == kind)
    }
}

#[derive(Debug, Clone)]
pub(super) struct LoadedSoundEvent {
    pub(super) kind: SoundEvent,
    pub(super) file: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawSoundPack {
    pub(super) kind: String,
    pub(super) format_version: u32,
    pub(super) event: Vec<RawSoundEvent>,
}

impl RawSoundPack {
    pub(super) fn validate(
        &self,
        sound_dir: &std::path::Path,
        manifest_path: &std::path::Path,
        require_all_events: bool,
    ) {
        if self.kind != "sound-pack" || self.format_version != 1 {
            panic!(
                "BattleTris sound-pack manifest {} has unsupported kind/version: kind={} format_version={}",
                manifest_path.display(),
                self.kind,
                self.format_version
            );
        }
        if require_all_events {
            for expected in SoundEvent::ALL {
                if !self.event.iter().any(|event| event.id == expected.id()) {
                    panic!(
                        "BattleTris sound-pack manifest {} is missing event {}",
                        manifest_path.display(),
                        expected.id()
                    );
                }
            }
        }
        for event in &self.event {
            if SoundEvent::from_id(&event.id).is_none() {
                panic!(
                    "BattleTris sound-pack manifest {} has unknown event {}",
                    manifest_path.display(),
                    event.id
                );
            }
            if event.files.is_empty() || !event.volume.is_finite() || event.volume < 0.0 {
                panic!(
                    "BattleTris sound-pack manifest {} has invalid event {}",
                    manifest_path.display(),
                    event.id
                );
            }
            for relative in &event.files {
                let path = sound_dir.join(relative);
                if !path.is_file() {
                    panic!(
                        "BattleTris sound-pack manifest {} requires missing sound {}",
                        manifest_path.display(),
                        path.display()
                    );
                }
                validate_wav_file(&path, manifest_path);
            }
        }
    }
}

pub(super) fn validate_wav_file(path: &std::path::Path, manifest_path: &std::path::Path) {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "BattleTris sound-pack manifest {} could not read WAV {}: {error}",
            manifest_path.display(),
            path.display()
        )
    });
    if bytes.len() < 44
        || &bytes[0..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || u16::from_le_bytes([bytes[20], bytes[21]]) != 1
        || u16::from_le_bytes([bytes[34], bytes[35]]) != 16
        || !bytes.windows(4).any(|chunk| chunk == b"data")
    {
        panic!(
            "BattleTris sound-pack manifest {} references undecodable PCM WAV {}",
            manifest_path.display(),
            path.display()
        );
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RawSoundEvent {
    pub(super) id: String,
    pub(super) files: Vec<String>,
    pub(super) volume: f32,
}

#[derive(Resource, Debug, Default)]
pub(super) struct SoundEventState {
    pub(super) next_log_index: usize,
    pub(super) last_event: Option<SoundEvent>,
    pub(super) pending_events: Vec<SoundEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SoundEvent {
    MenuAction,
    PieceLocked,
    LineClear,
    BazaarEntered,
    Purchase,
    WeaponLaunch,
    WeaponLaunchGimp,
    ChallengeIncoming,
    ChallengeRejected,
    BazaarWait,
    OpponentWait,
    GameLost,
    GameWon,
    GameDead,
    AboutEasterEgg,
    Warning,
    GameOver,
}

impl SoundEvent {
    pub(super) const ALL: [Self; 17] = [
        Self::MenuAction,
        Self::PieceLocked,
        Self::LineClear,
        Self::BazaarEntered,
        Self::Purchase,
        Self::WeaponLaunch,
        Self::WeaponLaunchGimp,
        Self::ChallengeIncoming,
        Self::ChallengeRejected,
        Self::BazaarWait,
        Self::OpponentWait,
        Self::GameLost,
        Self::GameWon,
        Self::GameDead,
        Self::AboutEasterEgg,
        Self::Warning,
        Self::GameOver,
    ];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::MenuAction => "menu_action",
            Self::PieceLocked => "piece_locked",
            Self::LineClear => "line_clear",
            Self::BazaarEntered => "bazaar_entered",
            Self::Purchase => "purchase",
            Self::WeaponLaunch => "weapon_launch",
            Self::WeaponLaunchGimp => "weapon_launch_gimp",
            Self::ChallengeIncoming => "challenge_incoming",
            Self::ChallengeRejected => "challenge_rejected",
            Self::BazaarWait => "bazaar_wait",
            Self::OpponentWait => "opponent_wait",
            Self::GameLost => "game_lost",
            Self::GameWon => "game_won",
            Self::GameDead => "game_dead",
            Self::AboutEasterEgg => "about_easter_egg",
            Self::Warning => "warning",
            Self::GameOver => "game_over",
        }
    }

    pub(super) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.id() == id)
    }
}

pub(super) fn collect_sound_events(
    local: Res<LocalGame>,
    settings: Res<ClientSettings>,
    mut sound: ResMut<SoundEventState>,
) {
    if settings.screen != ClientScreen::Game {
        return;
    }
    if settings.sound_pack == SoundPackChoice::Muted {
        sound.next_log_index = local.game.event_log().len();
        sound.last_event = None;
        sound.pending_events.clear();
        return;
    }

    for logged in &local.game.event_log()[sound.next_log_index..] {
        if let Some(event) = sound_event_for(&logged.event) {
            queue_sound(&mut sound, event);
        }
    }
    sound.next_log_index = local.game.event_log().len();
}

pub(super) fn play_sound_events(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    settings: Res<ClientSettings>,
    sound_packs: Res<SoundPacks>,
    mut sound: ResMut<SoundEventState>,
) {
    if settings.sound_pack == SoundPackChoice::Muted {
        sound.pending_events.clear();
        return;
    }

    for event in std::mem::take(&mut sound.pending_events) {
        let Some(sound_event) =
            sound_packs.sound_for(settings.sound_pack, settings.content_mode, event)
        else {
            continue;
        };
        commands.spawn((
            AudioPlayer::new(asset_server.load(sound_event.file.clone())),
            PlaybackSettings::DESPAWN,
        ));
    }
}

pub(super) fn sound_event_for(event: &BattleEvent) -> Option<SoundEvent> {
    match event {
        BattleEvent::PlayerEvent {
            event: CoreEvent::PieceLocked { .. },
            ..
        } => Some(SoundEvent::PieceLocked),
        BattleEvent::PlayerEvent {
            event: CoreEvent::LinesCleared { .. },
            ..
        } => Some(SoundEvent::LineClear),
        BattleEvent::PlayerEvent {
            event: CoreEvent::SpawnFailed { .. } | CoreEvent::HappyMissed { .. },
            ..
        } => Some(SoundEvent::Warning),
        BattleEvent::BazaarEntered => Some(SoundEvent::BazaarEntered),
        BattleEvent::BazaarPlayerDone { .. } | BattleEvent::BazaarLeft => {
            Some(SoundEvent::Purchase)
        }
        BattleEvent::WeaponLaunched {
            token: WeaponToken::Gimp,
            ..
        } => Some(SoundEvent::WeaponLaunchGimp),
        BattleEvent::WeaponLaunched { .. }
        | BattleEvent::OneShotWeaponApplied { .. }
        | BattleEvent::TimedWeaponActivated { .. }
        | BattleEvent::WeaponReflected { .. }
        | BattleEvent::WeaponNullified { .. } => Some(SoundEvent::WeaponLaunch),
        BattleEvent::TimedWeaponExpired { .. } => Some(SoundEvent::Purchase),
        BattleEvent::PlayerDied { .. } => Some(SoundEvent::GameDead),
        BattleEvent::GameOver { .. } => Some(SoundEvent::GameOver),
        BattleEvent::Paused | BattleEvent::Resumed => Some(SoundEvent::MenuAction),
        _ => None,
    }
}
