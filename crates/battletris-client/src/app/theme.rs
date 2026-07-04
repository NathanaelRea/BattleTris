//! Theme manifest loading, atlas handles, and themed entity updates.

use super::*;

#[derive(Resource, Debug, Clone)]
pub(super) struct ThemePacks {
    pub(super) original: LoadedTheme,
    pub(super) high_contrast: LoadedTheme,
}

impl ThemePacks {
    pub(super) fn load(assets_dir: &std::path::Path) -> Self {
        Self {
            original: LoadedTheme::load(assets_dir, ThemeChoice::Original),
            high_contrast: LoadedTheme::load(assets_dir, ThemeChoice::HighContrast),
        }
    }

    pub(super) const fn get(&self, choice: ThemeChoice) -> &LoadedTheme {
        match choice {
            ThemeChoice::Original => &self.original,
            ThemeChoice::HighContrast => &self.high_contrast,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LoadedTheme {
    pub(super) sprites: LoadedThemeSprites,
    pub(super) fonts: LoadedThemeFonts,
    pub(super) cell: ThemeCell,
    pub(super) cell_atlas: ThemeCellAtlas,
    pub(super) layout: ThemeLayout,
    pub(super) palette: ThemePalette,
    pub(super) screen: ThemeScreenStyle,
    pub(super) button: ThemeButtonStyle,
    pub(super) about: ThemeAboutStyle,
}

impl LoadedTheme {
    pub(super) fn load(assets_dir: &std::path::Path, choice: ThemeChoice) -> Self {
        let theme_dir = assets_dir.join("themes").join(choice.directory());
        let manifest_path = theme_dir.join("theme.toml");
        let contents = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
            panic!(
                "BattleTris theme manifest {} could not be read: {error}",
                manifest_path.display()
            )
        });
        let raw: RawTheme = toml::from_str(&contents).unwrap_or_else(|error| {
            panic!(
                "BattleTris theme manifest {} could not be parsed: {error}",
                manifest_path.display()
            )
        });
        raw.validate(&theme_dir, &manifest_path);
        Self {
            sprites: raw.sprites.loaded(choice),
            fonts: raw.fonts.loaded(choice),
            cell: raw.cell,
            cell_atlas: raw.sprites.cell_atlas,
            layout: raw.layout,
            palette: raw.semantic.palette(&manifest_path),
            screen: raw.screen.into_style(&manifest_path),
            button: raw.semantic.button(&manifest_path),
            about: raw.about.into_style(&manifest_path),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RawTheme {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) format_version: u32,
    pub(super) sprites: ThemeSprites,
    pub(super) fonts: ThemeFonts,
    pub(super) cell: ThemeCell,
    pub(super) layout: ThemeLayout,
    pub(super) semantic: RawThemeSemantic,
    pub(super) screen: RawThemeScreenStyle,
    pub(super) about: RawThemeAboutStyle,
    pub(super) description: String,
    pub(super) author: String,
    pub(super) license: String,
    pub(super) default_scale: f32,
    pub(super) pixel_filtering: String,
    pub(super) supports_high_contrast: bool,
    pub(super) provenance: ThemeProvenance,
}

impl RawTheme {
    pub(super) fn validate(&self, theme_dir: &std::path::Path, manifest_path: &std::path::Path) {
        let _accessibility_flag = self.supports_high_contrast;
        if self.kind != "theme" || self.format_version != 1 {
            panic!(
                "BattleTris theme manifest {} has unsupported kind/version: kind={} format_version={}",
                manifest_path.display(),
                self.kind,
                self.format_version
            );
        }
        if self.id.trim().is_empty()
            || self.name.trim().is_empty()
            || self.description.trim().is_empty()
            || self.author.trim().is_empty()
            || self.license.trim().is_empty()
            || self.default_scale <= 0.0
            || !matches!(self.pixel_filtering.as_str(), "nearest" | "linear")
            || self.cell.size <= 0.0
            || self.cell.gap < 0.0
            || self.cell.shadow < 0.0
            || self.layout.board.spacing <= 0.0
            || self.screen.title_font_size <= 0.0
            || self.screen.body_font_size <= 0.0
            || self.screen.button_font_size <= 0.0
            || self.fonts.line_height <= 0.0
        {
            panic!(
                "BattleTris theme manifest {} has invalid metadata or layout values",
                manifest_path.display()
            );
        }
        self.layout.validate(manifest_path);
        self.sprites.cell_atlas.validate(manifest_path);
        self.semantic.validate(manifest_path);
        if self.provenance.notes.trim().is_empty() || self.provenance.sources.is_empty() {
            panic!(
                "BattleTris theme manifest {} requires provenance notes and at least one source",
                manifest_path.display()
            );
        }
        for relative in [
            &self.sprites.atlas,
            &self.sprites.startup,
            &self.sprites.bazaar,
            &self.sprites.biff,
            &self.sprites.gimp,
            &self.sprites.crest,
        ] {
            let path = theme_dir.join(relative);
            if !path.is_file() {
                panic!(
                    "BattleTris theme manifest {} requires missing asset {}",
                    manifest_path.display(),
                    path.display()
                );
            }
        }
        if let Some(rated) = &self.sprites.rated {
            for relative in [&rated.atlas, &rated.gimp] {
                let path = theme_dir.join(relative);
                if !path.is_file() {
                    panic!(
                        "BattleTris theme manifest {} requires missing rated asset {}",
                        manifest_path.display(),
                        path.display()
                    );
                }
            }
        }
        self.sprites
            .cell_atlas
            .validate_image(theme_dir, &self.sprites.atlas, manifest_path);
        if let Some(rated) = &self.sprites.rated {
            self.sprites
                .cell_atlas
                .validate_image(theme_dir, &rated.atlas, manifest_path);
        }
        for font in [&self.fonts.ui, &self.fonts.title, &self.fonts.mono] {
            if !font.is_empty() {
                let path = theme_dir.join(font);
                if !path.is_file() {
                    panic!(
                        "BattleTris theme manifest {} requires missing font {}",
                        manifest_path.display(),
                        path.display()
                    );
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct ThemeProvenance {
    pub(super) notes: String,
    pub(super) sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ThemeSprites {
    pub(super) atlas: String,
    pub(super) startup: String,
    pub(super) bazaar: String,
    pub(super) biff: String,
    pub(super) gimp: String,
    pub(super) crest: String,
    pub(super) rated: Option<ThemeRatedSprites>,
    pub(super) cell_atlas: ThemeCellAtlas,
}

impl ThemeSprites {
    pub(super) fn loaded(&self, choice: ThemeChoice) -> LoadedThemeSprites {
        let prefix = format!("themes/{}/", choice.directory());
        LoadedThemeSprites {
            atlas: format!("{prefix}{}", self.atlas),
            startup: format!("{prefix}{}", self.startup),
            bazaar: format!("{prefix}{}", self.bazaar),
            biff: format!("{prefix}{}", self.biff),
            gimp: format!("{prefix}{}", self.gimp),
            crest: format!("{prefix}{}", self.crest),
            rated: self.rated.as_ref().map(|rated| LoadedThemeRatedSprites {
                atlas: format!("{prefix}{}", rated.atlas),
                gimp: format!("{prefix}{}", rated.gimp),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct ThemeRatedSprites {
    pub(super) atlas: String,
    pub(super) gimp: String,
}

#[derive(Debug, Clone)]
pub(super) struct LoadedThemeSprites {
    pub(super) atlas: String,
    pub(super) startup: String,
    pub(super) bazaar: String,
    pub(super) biff: String,
    pub(super) gimp: String,
    pub(super) crest: String,
    pub(super) rated: Option<LoadedThemeRatedSprites>,
}

impl LoadedThemeSprites {
    pub(super) fn atlas_for(&self, content_mode: ContentMode) -> &str {
        match (content_mode, &self.rated) {
            (ContentMode::Rated, Some(rated)) => &rated.atlas,
            _ => &self.atlas,
        }
    }

    pub(super) fn gimp_for(&self, content_mode: ContentMode) -> &str {
        match (content_mode, &self.rated) {
            (ContentMode::Rated, Some(rated)) => &rated.gimp,
            _ => &self.gimp,
        }
    }

    pub(super) const fn supports_rated(&self) -> bool {
        self.rated.is_some()
    }
}

#[derive(Debug, Clone)]
pub(super) struct LoadedThemeRatedSprites {
    pub(super) atlas: String,
    pub(super) gimp: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ThemeFonts {
    pub(super) ui: String,
    pub(super) title: String,
    pub(super) mono: String,
    pub(super) line_height: f32,
    pub(super) tracking: f32,
}

impl ThemeFonts {
    pub(super) fn loaded(&self, choice: ThemeChoice) -> LoadedThemeFonts {
        LoadedThemeFonts {
            ui: theme_asset_path(choice, &self.ui),
            title: theme_asset_path(choice, &self.title),
            mono: theme_asset_path(choice, &self.mono),
            line_height: self.line_height,
            tracking: self.tracking,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LoadedThemeFonts {
    pub(super) ui: Option<String>,
    pub(super) title: Option<String>,
    pub(super) mono: Option<String>,
    pub(super) line_height: f32,
    pub(super) tracking: f32,
}

impl LoadedThemeFonts {
    pub(super) fn path_for(&self, role: ThemedTextFontRole) -> Option<&str> {
        match role {
            ThemedTextFontRole::Title => self.title.as_deref().or(self.ui.as_deref()),
            ThemedTextFontRole::Body | ThemedTextFontRole::Button => {
                self.ui.as_deref().or(self.mono.as_deref())
            }
            ThemedTextFontRole::Mono => self.mono.as_deref().or(self.ui.as_deref()),
        }
    }
}

pub(super) fn theme_asset_path(choice: ThemeChoice, relative: &str) -> Option<String> {
    if relative.is_empty() {
        None
    } else {
        Some(format!("themes/{}/{}", choice.directory(), relative))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeCell {
    pub(super) size: f32,
    pub(super) gap: f32,
    pub(super) shadow: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeCellAtlas {
    pub(super) tile_width: u32,
    pub(super) tile_height: u32,
    pub(super) columns: u32,
    pub(super) rows: u32,
    pub(super) padding_x: u32,
    pub(super) padding_y: u32,
    pub(super) offset_x: u32,
    pub(super) offset_y: u32,
    pub(super) cells: ThemeCellAtlasCells,
}

impl ThemeCellAtlas {
    pub(super) fn texture_count(self) -> usize {
        self.columns as usize * self.rows as usize
    }

    pub(super) fn tile_size(self) -> UVec2 {
        UVec2::new(self.tile_width, self.tile_height)
    }

    pub(super) fn padding(self) -> Option<UVec2> {
        Some(UVec2::new(self.padding_x, self.padding_y))
    }

    pub(super) fn offset(self) -> Option<UVec2> {
        Some(UVec2::new(self.offset_x, self.offset_y))
    }

    pub(super) fn validate(self, manifest_path: &std::path::Path) {
        if self.tile_width == 0 || self.tile_height == 0 || self.columns == 0 || self.rows == 0 {
            panic!(
                "BattleTris theme manifest {} has invalid cell atlas dimensions",
                manifest_path.display()
            );
        }
        if self.cells.visible_colors.len() != 19 || self.cells.die.len() != 6 {
            panic!(
                "BattleTris theme manifest {} must map 19 visible colors and 6 die faces",
                manifest_path.display()
            );
        }
        let texture_count = self.texture_count();
        let mut indices = Vec::new();
        indices.push(self.cells.empty);
        indices.extend(self.cells.visible_colors);
        indices.push(self.cells.structure);
        indices.push(self.cells.happy);
        indices.push(self.cells.frown);
        indices.push(self.cells.gimp);
        indices.extend(self.cells.die);
        indices.push(self.cells.invisible);
        indices.push(self.cells.hidden);
        let unique = indices.iter().copied().collect::<HashSet<_>>();
        if unique.len() != indices.len() || indices.iter().any(|index| *index >= texture_count) {
            panic!(
                "BattleTris theme manifest {} has duplicate or out-of-range cell atlas indices",
                manifest_path.display()
            );
        }
    }

    pub(super) fn validate_image(
        self,
        theme_dir: &std::path::Path,
        atlas: &str,
        manifest_path: &std::path::Path,
    ) {
        let path = theme_dir.join(atlas);
        let (width, height) = image::image_dimensions(&path).unwrap_or_else(|error| {
            panic!(
                "BattleTris theme manifest {} requires decodable atlas {}: {error}",
                manifest_path.display(),
                path.display()
            )
        });
        let expected_width = self.offset_x
            + self.columns * self.tile_width
            + self.columns.saturating_sub(1) * self.padding_x;
        let expected_height = self.offset_y
            + self.rows * self.tile_height
            + self.rows.saturating_sub(1) * self.padding_y;
        if width < expected_width || height < expected_height {
            panic!(
                "BattleTris theme manifest {} atlas {} is {}x{}, expected at least {}x{}",
                manifest_path.display(),
                path.display(),
                width,
                height,
                expected_width,
                expected_height
            );
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeCellAtlasCells {
    pub(super) empty: usize,
    pub(super) visible_colors: [usize; 19],
    pub(super) structure: usize,
    pub(super) happy: usize,
    pub(super) frown: usize,
    pub(super) gimp: usize,
    pub(super) die: [usize; 6],
    pub(super) invisible: usize,
    pub(super) hidden: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeLayout {
    pub(super) board: ThemeBoardLayout,
    pub(super) screens: ThemeScreenLayouts,
    pub(super) rects: ThemeLayoutRects,
}

impl ThemeLayout {
    pub(super) const fn screen(&self, screen: ClientScreen) -> ThemeWindowLayout {
        match screen {
            ClientScreen::Startup => self.screens.startup,
            ClientScreen::Game => self.screens.game,
            ClientScreen::Challenge => self.screens.challenge,
            ClientScreen::Sleep => self.screens.sleep,
            ClientScreen::About => self.screens.about,
            ClientScreen::Roster => self.screens.roster,
            ClientScreen::Settings => self.screens.settings,
        }
    }

    pub(super) const fn fixture(&self, fixture: VisualFixture) -> ThemeWindowLayout {
        match fixture {
            VisualFixture::Startup => self.screens.startup,
            VisualFixture::Challenge => self.screens.challenge,
            VisualFixture::Sleep => self.screens.sleep,
            VisualFixture::About => self.screens.about,
            VisualFixture::Roster => self.screens.roster,
            VisualFixture::Settings => self.screens.settings,
            VisualFixture::GamePlaying | VisualFixture::GameOver | VisualFixture::BoardCells => {
                self.screens.game
            }
            VisualFixture::GameBazaar => self.screens.bazaar,
            VisualFixture::GameRecon => self.screens.game_recon,
        }
    }

    pub(super) fn validate(&self, manifest_path: &std::path::Path) {
        for (name, window) in self.screens.named() {
            if window.width <= 0.0 || window.height <= 0.0 {
                panic!(
                    "BattleTris theme manifest {} has invalid {name} screen size",
                    manifest_path.display()
                );
            }
        }
        self.rects.validate(manifest_path);
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeScreenLayouts {
    pub(super) startup: ThemeWindowLayout,
    pub(super) challenge: ThemeWindowLayout,
    pub(super) sleep: ThemeWindowLayout,
    pub(super) about: ThemeWindowLayout,
    pub(super) roster: ThemeWindowLayout,
    pub(super) settings: ThemeWindowLayout,
    pub(super) game: ThemeWindowLayout,
    pub(super) game_recon: ThemeWindowLayout,
    pub(super) bazaar: ThemeWindowLayout,
}

impl ThemeScreenLayouts {
    pub(super) const fn named(self) -> [(&'static str, ThemeWindowLayout); 9] {
        [
            ("startup", self.startup),
            ("challenge", self.challenge),
            ("sleep", self.sleep),
            ("about", self.about),
            ("roster", self.roster),
            ("settings", self.settings),
            ("game", self.game),
            ("game_recon", self.game_recon),
            ("bazaar", self.bazaar),
        ]
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeWindowLayout {
    pub(super) width: f32,
    pub(super) height: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeBoardLayout {
    pub(super) top: f32,
    pub(super) player_one_left: f32,
    pub(super) player_two_left: f32,
    pub(super) spacing: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeLayoutRects {
    pub(super) startup_challenge: ThemeRect,
    pub(super) startup_sleep: ThemeRect,
    pub(super) startup_about: ThemeRect,
    pub(super) startup_roster: ThemeRect,
    pub(super) startup_quit: ThemeRect,
    pub(super) startup_local_game: ThemeRect,
    pub(super) startup_play_ernie: ThemeRect,
    pub(super) startup_theme: ThemeRect,
    pub(super) challenge_level_down: ThemeRect,
    pub(super) challenge_level_up: ThemeRect,
    pub(super) challenge_play_ernie: ThemeRect,
    pub(super) challenge_back: ThemeRect,
    pub(super) sleep_wake: ThemeRect,
    pub(super) about_ok: ThemeRect,
    pub(super) roster_back: ThemeRect,
    pub(super) settings_back: ThemeRect,
    pub(super) bazaar_catalog: ThemeRect,
    pub(super) bazaar_arsenal: ThemeRect,
    pub(super) bazaar_add: ThemeRect,
    pub(super) bazaar_remove: ThemeRect,
    pub(super) bazaar_done: ThemeRect,
}

impl ThemeLayoutRects {
    pub(super) fn validate(self, manifest_path: &std::path::Path) {
        for (name, rect) in self.named() {
            if rect.width <= 0.0 || rect.height <= 0.0 {
                panic!(
                    "BattleTris theme manifest {} has invalid rect {name}",
                    manifest_path.display()
                );
            }
        }
    }

    pub(super) const fn named(self) -> [(&'static str, ThemeRect); 21] {
        [
            ("startup_challenge", self.startup_challenge),
            ("startup_sleep", self.startup_sleep),
            ("startup_about", self.startup_about),
            ("startup_roster", self.startup_roster),
            ("startup_quit", self.startup_quit),
            ("startup_local_game", self.startup_local_game),
            ("startup_play_ernie", self.startup_play_ernie),
            ("startup_theme", self.startup_theme),
            ("challenge_level_down", self.challenge_level_down),
            ("challenge_level_up", self.challenge_level_up),
            ("challenge_play_ernie", self.challenge_play_ernie),
            ("challenge_back", self.challenge_back),
            ("sleep_wake", self.sleep_wake),
            ("about_ok", self.about_ok),
            ("roster_back", self.roster_back),
            ("settings_back", self.settings_back),
            ("bazaar_catalog", self.bazaar_catalog),
            ("bazaar_arsenal", self.bazaar_arsenal),
            ("bazaar_add", self.bazaar_add),
            ("bazaar_remove", self.bazaar_remove),
            ("bazaar_done", self.bazaar_done),
        ]
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ThemeRect {
    pub(super) center_x: f32,
    pub(super) center_y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

impl ThemeRect {
    pub(super) fn center(self) -> Vec2 {
        Vec2::new(self.center_x, self.center_y)
    }

    pub(super) fn size(self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }

    pub(super) fn rect(self) -> Rect {
        Rect::from_center_size(self.center(), self.size())
    }
}

#[derive(Debug, Clone)]
pub(super) struct ThemePalette {
    pub(super) board_background: Color,
    pub(super) empty: Color,
    pub(super) structure: Color,
    pub(super) happy: Color,
    pub(super) frown: Color,
    pub(super) gimp: Color,
    pub(super) die: Color,
    pub(super) invisible: Color,
    pub(super) hidden: Color,
    pub(super) text_secondary: Color,
    pub(super) text_accent: Color,
    pub(super) visible_colors: Vec<Color>,
}

#[derive(Debug, Clone)]
pub(super) struct ThemeButtonStyle {
    pub(super) normal: Color,
    pub(super) hover: Color,
    pub(super) pressed: Color,
    pub(super) text: Color,
}

#[derive(Debug, Clone)]
pub(super) struct ThemeScreenStyle {
    pub(super) background: Color,
    pub(super) title_text: Color,
    pub(super) body_text: Color,
    pub(super) title_font_size: f32,
    pub(super) body_font_size: f32,
    pub(super) button_font_size: f32,
}

#[derive(Debug, Clone)]
pub(super) struct ThemeAboutStyle {
    pub(super) background: Color,
    pub(super) title_text: Color,
    pub(super) name_text: Color,
    pub(super) credit_text: Color,
    pub(super) button_face: Color,
    pub(super) button_highlight: Color,
    pub(super) button_shadow: Color,
    pub(super) button_text: Color,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawThemeScreenStyle {
    pub(super) background: String,
    pub(super) title_text: String,
    pub(super) body_text: String,
    pub(super) title_font_size: f32,
    pub(super) body_font_size: f32,
    pub(super) button_font_size: f32,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawThemeAboutStyle {
    pub(super) background: String,
    pub(super) title_text: String,
    pub(super) name_text: String,
    pub(super) credit_text: String,
    pub(super) button_face: String,
    pub(super) button_highlight: String,
    pub(super) button_shadow: String,
    pub(super) button_text: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawThemeSemantic {
    pub(super) text: RawThemeSemanticText,
    pub(super) board: RawThemeSemanticBoard,
    pub(super) button: RawThemeSemanticButton,
    pub(super) bazaar: RawThemeSemanticBazaar,
    pub(super) weapon: RawThemeSemanticWeapon,
}

impl RawThemeSemantic {
    pub(super) fn validate(&self, manifest_path: &std::path::Path) {
        for color in [
            &self.text.primary,
            &self.text.secondary,
            &self.text.accent,
            &self.text.warning,
            &self.board.background,
            &self.board.empty,
            &self.board.structure,
            &self.board.happy,
            &self.board.frown,
            &self.board.gimp,
            &self.board.die,
            &self.board.invisible,
            &self.board.hidden,
            &self.button.normal,
            &self.button.hover,
            &self.button.pressed,
            &self.button.text,
            &self.bazaar.affordable,
            &self.bazaar.unaffordable,
            &self.bazaar.selected,
            &self.weapon.active,
            &self.weapon.expired,
        ] {
            let _ = parse_hex_color(color, manifest_path);
        }
        if self.board.visible_colors.len() != 19 {
            panic!(
                "BattleTris theme manifest {} must define 19 semantic visible cell colors",
                manifest_path.display()
            );
        }
        for color in &self.board.visible_colors {
            let _ = parse_hex_color(color, manifest_path);
        }
    }

    pub(super) fn palette(&self, manifest_path: &std::path::Path) -> ThemePalette {
        ThemePalette {
            board_background: parse_hex_color(&self.board.background, manifest_path),
            empty: parse_hex_color(&self.board.empty, manifest_path),
            structure: parse_hex_color(&self.board.structure, manifest_path),
            happy: parse_hex_color(&self.board.happy, manifest_path),
            frown: parse_hex_color(&self.board.frown, manifest_path),
            gimp: parse_hex_color(&self.board.gimp, manifest_path),
            die: parse_hex_color(&self.board.die, manifest_path),
            invisible: parse_hex_color(&self.board.invisible, manifest_path),
            hidden: parse_hex_color(&self.board.hidden, manifest_path),
            text_secondary: parse_hex_color(&self.text.secondary, manifest_path),
            text_accent: parse_hex_color(&self.text.accent, manifest_path),
            visible_colors: self
                .board
                .visible_colors
                .iter()
                .map(|color| parse_hex_color(color, manifest_path))
                .collect(),
        }
    }

    pub(super) fn button(&self, manifest_path: &std::path::Path) -> ThemeButtonStyle {
        ThemeButtonStyle {
            normal: parse_hex_color(&self.button.normal, manifest_path),
            hover: parse_hex_color(&self.button.hover, manifest_path),
            pressed: parse_hex_color(&self.button.pressed, manifest_path),
            text: parse_hex_color(&self.button.text, manifest_path),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RawThemeSemanticText {
    pub(super) primary: String,
    pub(super) secondary: String,
    pub(super) accent: String,
    pub(super) warning: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawThemeSemanticBoard {
    pub(super) background: String,
    pub(super) empty: String,
    pub(super) structure: String,
    pub(super) happy: String,
    pub(super) frown: String,
    pub(super) gimp: String,
    pub(super) die: String,
    pub(super) invisible: String,
    pub(super) hidden: String,
    pub(super) visible_colors: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawThemeSemanticButton {
    pub(super) normal: String,
    pub(super) hover: String,
    pub(super) pressed: String,
    pub(super) text: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawThemeSemanticBazaar {
    pub(super) affordable: String,
    pub(super) unaffordable: String,
    pub(super) selected: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawThemeSemanticWeapon {
    pub(super) active: String,
    pub(super) expired: String,
}

impl RawThemeScreenStyle {
    pub(super) fn into_style(self, manifest_path: &std::path::Path) -> ThemeScreenStyle {
        ThemeScreenStyle {
            background: parse_hex_color(&self.background, manifest_path),
            title_text: parse_hex_color(&self.title_text, manifest_path),
            body_text: parse_hex_color(&self.body_text, manifest_path),
            title_font_size: self.title_font_size,
            body_font_size: self.body_font_size,
            button_font_size: self.button_font_size,
        }
    }
}

impl RawThemeAboutStyle {
    pub(super) fn into_style(self, manifest_path: &std::path::Path) -> ThemeAboutStyle {
        ThemeAboutStyle {
            background: parse_hex_color(&self.background, manifest_path),
            title_text: parse_hex_color(&self.title_text, manifest_path),
            name_text: parse_hex_color(&self.name_text, manifest_path),
            credit_text: parse_hex_color(&self.credit_text, manifest_path),
            button_face: parse_hex_color(&self.button_face, manifest_path),
            button_highlight: parse_hex_color(&self.button_highlight, manifest_path),
            button_shadow: parse_hex_color(&self.button_shadow, manifest_path),
            button_text: parse_hex_color(&self.button_text, manifest_path),
        }
    }
}

pub(super) fn parse_hex_color(value: &str, manifest_path: &std::path::Path) -> Color {
    let Some(hex) = value.strip_prefix('#') else {
        panic!(
            "BattleTris theme manifest {} has non-hex color {value}",
            manifest_path.display()
        );
    };
    let (rgb, alpha) = match hex.len() {
        6 => (hex, "ff"),
        8 => hex.split_at(6),
        _ => panic!(
            "BattleTris theme manifest {} has invalid color {value}",
            manifest_path.display()
        ),
    };
    let red = u8::from_str_radix(&rgb[0..2], 16).expect("validated hex red");
    let green = u8::from_str_radix(&rgb[2..4], 16).expect("validated hex green");
    let blue = u8::from_str_radix(&rgb[4..6], 16).expect("validated hex blue");
    let alpha = u8::from_str_radix(alpha, 16).expect("validated hex alpha");
    Color::srgba_u8(red, green, blue, alpha)
}

#[derive(Resource, Debug, Clone)]
pub(super) struct ThemeAtlasHandles {
    pub(super) original: ThemeAtlasHandle,
    pub(super) high_contrast: ThemeAtlasHandle,
}

impl ThemeAtlasHandles {
    pub(super) fn get(
        &self,
        choice: ThemeChoice,
        content_mode: ContentMode,
        themes: &ThemePacks,
    ) -> &ThemeAtlasImageHandle {
        let theme = themes.get(choice);
        let handles = match choice {
            ThemeChoice::Original => &self.original,
            ThemeChoice::HighContrast => &self.high_contrast,
        };
        if content_mode == ContentMode::Rated {
            if let Some(rated) = &handles.rated {
                return rated;
            }
            warn!(
                "BattleTris rated content mode requested, but theme {:?} has no rated assets; using normal sprites",
                choice
            );
            debug_assert!(!theme.sprites.supports_rated());
        }
        &handles.normal
    }
}

#[derive(Debug, Clone)]
pub(super) struct ThemeAtlasHandle {
    pub(super) normal: ThemeAtlasImageHandle,
    pub(super) rated: Option<ThemeAtlasImageHandle>,
}

#[derive(Debug, Clone)]
pub(super) struct ThemeAtlasImageHandle {
    pub(super) image: Handle<Image>,
    pub(super) layout: Handle<TextureAtlasLayout>,
}

pub(super) fn load_theme_atlases(
    mut commands: Commands,
    themes: Res<ThemePacks>,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.insert_resource(ThemeAtlasHandles {
        original: theme_atlas_handle(
            themes.get(ThemeChoice::Original),
            &asset_server,
            &mut atlas_layouts,
        ),
        high_contrast: theme_atlas_handle(
            themes.get(ThemeChoice::HighContrast),
            &asset_server,
            &mut atlas_layouts,
        ),
    });
}

pub(super) fn theme_atlas_handle(
    theme: &LoadedTheme,
    asset_server: &AssetServer,
    atlas_layouts: &mut Assets<TextureAtlasLayout>,
) -> ThemeAtlasHandle {
    let layout = TextureAtlasLayout::from_grid(
        theme.cell_atlas.tile_size(),
        theme.cell_atlas.columns,
        theme.cell_atlas.rows,
        theme.cell_atlas.padding(),
        theme.cell_atlas.offset(),
    );
    let layout = atlas_layouts.add(layout);
    ThemeAtlasHandle {
        normal: ThemeAtlasImageHandle {
            image: asset_server.load(theme.sprites.atlas_for(ContentMode::Normal).to_string()),
            layout: layout.clone(),
        },
        rated: theme
            .sprites
            .rated
            .as_ref()
            .map(|rated| ThemeAtlasImageHandle {
                image: asset_server.load(rated.atlas.clone()),
                layout,
            }),
    }
}

#[derive(SystemParam)]
pub(super) struct ThemeEntityQueries<'w, 's> {
    pub(super) sprites: Query<'w, 's, (&'static ThemedSprite, &'static mut Sprite)>,
    pub(super) color_sprites:
        Query<'w, 's, (&'static ThemedColorSprite, &'static mut Sprite), Without<ThemedSprite>>,
    pub(super) text_colors: Query<'w, 's, (&'static ThemedTextColor, &'static mut TextColor)>,
    pub(super) text_fonts: Query<'w, 's, (&'static ThemedTextFont, &'static mut TextFont)>,
}

pub(super) fn update_theme_entities(
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    asset_server: Res<AssetServer>,
    mut active_theme: Local<Option<(ThemeChoice, ContentMode)>>,
    mut themed: ThemeEntityQueries,
) {
    let active_key = (settings.theme, settings.content_mode);
    if *active_theme == Some(active_key) {
        return;
    }
    *active_theme = Some(active_key);

    let theme = themes.get(settings.theme);
    for (sprite_theme, mut sprite) in &mut themed.sprites {
        sprite.image = asset_server.load(themed_sprite_path(
            theme,
            sprite_theme.role,
            settings.content_mode,
        ));
    }
    for (sprite_theme, mut sprite) in &mut themed.color_sprites {
        sprite.color = themed_sprite_color(theme, sprite_theme.role);
    }
    for (text_theme, mut text_color) in &mut themed.text_colors {
        text_color.0 = themed_text_color(theme, text_theme.role);
    }
    for (font_theme, mut text_font) in &mut themed.text_fonts {
        *text_font = themed_text_font(theme, font_theme.role, &asset_server);
    }
}

pub(super) fn update_challenge_logo_texture(
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<ChallengeLogoTextureCache>,
    mut logos: Query<&mut Sprite, With<ChallengeLogo>>,
) {
    if logos.is_empty() {
        return;
    }

    let logo = if let Some(handle) = cache.get(settings.theme) {
        handle
    } else {
        let raw_handle: Handle<Image> =
            asset_server.load(themes.get(settings.theme).sprites.biff.clone());
        let processed = images.get(&raw_handle).map(|source| {
            let mut image = source.clone();
            quantize_motif_ppm_image(&mut image);
            image
        });
        if let Some(image) = processed {
            let handle = images.add(image);
            cache.set(settings.theme, handle.clone());
            handle
        } else {
            raw_handle
        }
    };

    for mut sprite in &mut logos {
        sprite.image = logo.clone();
    }
}

pub(super) fn quantize_motif_ppm_image(image: &mut Image) {
    let Some(data) = image.data.as_mut() else {
        return;
    };
    match image.texture_descriptor.format {
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb => {
            for pixel in data.chunks_exact_mut(4) {
                pixel[0] = quantize_motif_ppm_component(pixel[0]);
                pixel[1] = quantize_motif_ppm_component(pixel[1]);
                pixel[2] = quantize_motif_ppm_component(pixel[2]);
            }
            image.sampler = ImageSampler::nearest();
        }
        _ => {}
    }
}

pub(super) fn quantize_motif_ppm_component(value: u8) -> u8 {
    let max = u8::MAX as u16;
    let bucket = value as u16 * 4 / max;
    (bucket * max / 4) as u8
}

pub(super) fn themed_sprite_path(
    theme: &LoadedTheme,
    role: ThemedSpriteRole,
    _content_mode: ContentMode,
) -> String {
    match role {
        ThemedSpriteRole::Startup => theme.sprites.startup.clone(),
        ThemedSpriteRole::Bazaar => theme.sprites.bazaar.clone(),
        ThemedSpriteRole::Biff => theme.sprites.biff.clone(),
        ThemedSpriteRole::AboutIcon => theme.sprites.crest.clone(),
    }
}

pub(super) fn themed_sprite_color(theme: &LoadedTheme, role: ThemedColorSpriteRole) -> Color {
    match role {
        ThemedColorSpriteRole::ScreenBackground => theme.screen.background,
        ThemedColorSpriteRole::AboutBackground => theme.about.background,
        ThemedColorSpriteRole::ButtonHighlight => theme.about.button_highlight,
        ThemedColorSpriteRole::ButtonShadow => theme.about.button_shadow,
    }
}

pub(super) fn themed_text_color(theme: &LoadedTheme, role: ThemedTextColorRole) -> Color {
    match role {
        ThemedTextColorRole::Secondary => theme.palette.text_secondary,
        ThemedTextColorRole::ScreenTitle => theme.screen.title_text,
        ThemedTextColorRole::ScreenBody => theme.screen.body_text,
        ThemedTextColorRole::Button => theme.button.text,
        ThemedTextColorRole::AboutTitle => theme.about.title_text,
        ThemedTextColorRole::AboutName => theme.about.name_text,
        ThemedTextColorRole::AboutCredit => theme.about.credit_text,
        ThemedTextColorRole::AboutButton => theme.about.button_text,
    }
}

pub(super) fn themed_text_font_size(theme: &LoadedTheme, role: ThemedTextFontRole) -> f32 {
    match role {
        ThemedTextFontRole::Title => theme.screen.title_font_size,
        ThemedTextFontRole::Body => theme.screen.body_font_size,
        ThemedTextFontRole::Button => theme.screen.button_font_size,
        ThemedTextFontRole::Mono => theme.screen.body_font_size,
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ThemeTextAssets<'a> {
    pub(super) theme: &'a LoadedTheme,
    pub(super) asset_server: &'a AssetServer,
}

impl ThemeTextAssets<'_> {
    pub(super) fn font(self, role: ThemedTextFontRole, font_size: f32) -> TextFont {
        themed_text_font_at_size(self.theme, role, font_size, self.asset_server)
    }
}

pub(super) fn themed_text_font(
    theme: &LoadedTheme,
    role: ThemedTextFontRole,
    asset_server: &AssetServer,
) -> TextFont {
    themed_text_font_at_size(
        theme,
        role,
        themed_text_font_size(theme, role),
        asset_server,
    )
}

pub(super) fn themed_text_font_at_size(
    theme: &LoadedTheme,
    role: ThemedTextFontRole,
    font_size: f32,
    asset_server: &AssetServer,
) -> TextFont {
    let font = pixel_text_font(font_size);
    if let Some(path) = theme.fonts.path_for(role) {
        font.with_font(asset_server.load(path.to_string()))
    } else {
        font
    }
}

pub(super) fn pixel_text_font(font_size: f32) -> TextFont {
    TextFont::from_font_size(font_size)
        .with_font_smoothing(FontSmoothing::None)
        .with_font_weight(FontWeight::BOLD)
}
