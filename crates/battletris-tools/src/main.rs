//! Developer and content tool entry point.
//!
//! This crate will collect asset conversion, generated audio, replay inspection,
//! protocol fixture utilities, legacy data extraction, and future admin tools.

use image::{imageops::FilterType, Rgba, RgbaImage};
use std::{env, fs, io::Write, path::Path};
use toml::Value;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("generate-theme-assets") => {
            let assets_dir = args.next().unwrap_or_else(|| "assets".to_string());
            generate_theme_assets(Path::new(&assets_dir));
        }
        Some("generate-sound-pack") => {
            let assets_dir = args.next().unwrap_or_else(|| "assets".to_string());
            generate_sound_pack(Path::new(&assets_dir));
        }
        Some("validate-theme") => {
            let theme_dir = args.next().unwrap_or_else(|| {
                eprintln!("validate-theme requires a theme directory");
                std::process::exit(2);
            });
            validate_theme(Path::new(&theme_dir));
        }
        Some("validate-sound-pack") => {
            let sound_dir = args.next().unwrap_or_else(|| {
                eprintln!("validate-sound-pack requires a sound-pack directory");
                std::process::exit(2);
            });
            let sound_dir = Path::new(&sound_dir);
            validate_sound_pack(sound_dir, !is_overlay_sound_pack(sound_dir));
        }
        Some("generate-assets") => {
            let assets_dir = args.next().unwrap_or_else(|| "assets".to_string());
            let assets_dir = Path::new(&assets_dir);
            generate_theme_assets(assets_dir);
            generate_sound_pack(assets_dir);
        }
        _ => {
            eprintln!(
                "usage: tools generate-assets [assets-dir]\n       tools generate-theme-assets [assets-dir]\n       tools generate-sound-pack [assets-dir]\n       tools validate-theme <theme-dir>\n       tools validate-sound-pack <sound-pack-dir>"
            );
        }
    }
}

fn validate_sound_pack(sound_dir: &Path, require_all_events: bool) {
    let manifest_path = sound_dir.join("sound-pack.toml");
    let contents = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
        panic!(
            "read sound-pack manifest {} failed: {error}",
            manifest_path.display()
        )
    });
    let manifest: Value = toml::from_str(&contents).unwrap_or_else(|error| {
        panic!(
            "parse sound-pack manifest {} failed: {error}",
            manifest_path.display()
        )
    });
    if required_str(&manifest, "kind") != "sound-pack"
        || required_i64(&manifest, "format_version") != 1
        || required_str(&manifest, "name").trim().is_empty()
        || required_str(&manifest, "description").trim().is_empty()
    {
        panic!(
            "sound-pack manifest {} has invalid metadata",
            manifest_path.display()
        );
    }

    let events = required_array(&manifest, "event");
    if require_all_events {
        for expected in SOUND_EVENT_IDS {
            if !events.iter().any(|event| {
                event
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == *expected)
            }) {
                panic!(
                    "sound-pack manifest {} is missing event {}",
                    manifest_path.display(),
                    expected
                );
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for event in events {
        let id = event
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("sound-pack manifest event id must be a string"));
        if !SOUND_EVENT_IDS.contains(&id) || !seen.insert(id) {
            panic!(
                "sound-pack manifest {} has unknown or duplicate event {}",
                manifest_path.display(),
                id
            );
        }
        let files = event
            .get("files")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("sound-pack manifest event {id} needs files"));
        if files.is_empty()
            || event
                .get("volume")
                .and_then(Value::as_float)
                .is_none_or(|volume| !volume.is_finite() || volume < 0.0)
        {
            panic!(
                "sound-pack manifest {} has invalid event {}",
                manifest_path.display(),
                id
            );
        }
        for file in files {
            let relative = file
                .as_str()
                .unwrap_or_else(|| panic!("sound-pack manifest event {id} file must be a string"));
            validate_wav(&sound_dir.join(relative), &manifest_path);
        }
    }
    println!("validated sound pack: {}", sound_dir.display());
}

fn is_overlay_sound_pack(sound_dir: &Path) -> bool {
    sound_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "generated-rated")
}

fn validate_theme(theme_dir: &Path) {
    let manifest_path = theme_dir.join("theme.toml");
    let contents = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
        panic!(
            "read theme manifest {} failed: {error}",
            manifest_path.display()
        )
    });
    let manifest: Value = toml::from_str(&contents).unwrap_or_else(|error| {
        panic!(
            "parse theme manifest {} failed: {error}",
            manifest_path.display()
        )
    });

    let id = required_str(&manifest, "id");
    if id.trim().is_empty()
        || required_str(&manifest, "name").trim().is_empty()
        || required_str(&manifest, "description").trim().is_empty()
        || required_str(&manifest, "author").trim().is_empty()
        || required_str(&manifest, "license").trim().is_empty()
        || required_f64(&manifest, "default_scale") <= 0.0
        || required_str(&manifest, "kind") != "theme"
        || required_i64(&manifest, "format_version") != 1
    {
        panic!(
            "theme manifest {} has invalid metadata",
            manifest_path.display()
        );
    }
    let pixel_filtering = required_str(&manifest, "pixel_filtering");
    if pixel_filtering != "nearest" && pixel_filtering != "linear" {
        panic!(
            "theme manifest {} has unsupported pixel_filtering={pixel_filtering}",
            manifest_path.display()
        );
    }
    let _ = required_bool(&manifest, "supports_high_contrast");
    if required_str(&manifest, "provenance.notes")
        .trim()
        .is_empty()
        || required_array(&manifest, "provenance.sources").is_empty()
    {
        panic!(
            "theme manifest {} needs provenance notes and sources",
            manifest_path.display()
        );
    }

    for path_key in [
        "sprites.atlas",
        "sprites.startup",
        "sprites.bazaar",
        "sprites.biff",
        "sprites.gimp",
        "sprites.crest",
    ] {
        let relative = required_str(&manifest, path_key);
        decode_png(&theme_dir.join(relative), &manifest_path);
    }
    let rated_atlas = optional_str(&manifest, "sprites.rated.atlas");
    let rated_gimp = optional_str(&manifest, "sprites.rated.gimp");
    match (rated_atlas, rated_gimp) {
        (Some(atlas), Some(gimp)) => {
            decode_png(&theme_dir.join(atlas), &manifest_path);
            decode_png(&theme_dir.join(gimp), &manifest_path);
        }
        (None, None) => {}
        _ => panic!(
            "theme manifest {} must declare both sprites.rated.atlas and sprites.rated.gimp or neither",
            manifest_path.display()
        ),
    }

    validate_theme_fonts(theme_dir, &manifest, &manifest_path);
    validate_theme_atlas(theme_dir, &manifest, &manifest_path);
    validate_theme_colors(&manifest, &manifest_path);
    validate_theme_layout(&manifest, &manifest_path);

    println!("validated theme {id}: {}", theme_dir.display());
}

fn validate_theme_fonts(theme_dir: &Path, manifest: &Value, manifest_path: &Path) {
    for key in ["fonts.ui", "fonts.title", "fonts.mono"] {
        let relative = required_str(manifest, key);
        if !relative.is_empty() && !theme_dir.join(relative).is_file() {
            panic!(
                "theme manifest {} references missing font {}",
                manifest_path.display(),
                theme_dir.join(relative).display()
            );
        }
    }
    if required_f64(manifest, "fonts.line_height") <= 0.0 {
        panic!(
            "theme manifest {} has invalid fonts.line_height",
            manifest_path.display()
        );
    }
    let _ = required_f64(manifest, "fonts.tracking");
}

fn validate_theme_atlas(theme_dir: &Path, manifest: &Value, manifest_path: &Path) {
    let tile_width = required_u32(manifest, "sprites.cell_atlas.tile_width");
    let tile_height = required_u32(manifest, "sprites.cell_atlas.tile_height");
    let columns = required_u32(manifest, "sprites.cell_atlas.columns");
    let rows = required_u32(manifest, "sprites.cell_atlas.rows");
    let padding_x = required_u32(manifest, "sprites.cell_atlas.padding_x");
    let padding_y = required_u32(manifest, "sprites.cell_atlas.padding_y");
    let offset_x = required_u32(manifest, "sprites.cell_atlas.offset_x");
    let offset_y = required_u32(manifest, "sprites.cell_atlas.offset_y");
    if tile_width == 0 || tile_height == 0 || columns == 0 || rows == 0 {
        panic!(
            "theme manifest {} has invalid cell atlas dimensions",
            manifest_path.display()
        );
    }

    let texture_count = columns as usize * rows as usize;
    let mut indices = Vec::new();
    indices.push(required_usize(manifest, "sprites.cell_atlas.cells.empty"));
    indices.extend(required_usize_array(
        manifest,
        "sprites.cell_atlas.cells.visible_colors",
        19,
    ));
    indices.push(required_usize(
        manifest,
        "sprites.cell_atlas.cells.structure",
    ));
    indices.push(required_usize(manifest, "sprites.cell_atlas.cells.happy"));
    indices.push(required_usize(manifest, "sprites.cell_atlas.cells.frown"));
    indices.push(required_usize(manifest, "sprites.cell_atlas.cells.gimp"));
    indices.extend(required_usize_array(
        manifest,
        "sprites.cell_atlas.cells.die",
        6,
    ));
    indices.push(required_usize(
        manifest,
        "sprites.cell_atlas.cells.invisible",
    ));
    indices.push(required_usize(manifest, "sprites.cell_atlas.cells.hidden"));

    let unique = indices
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if unique.len() != indices.len() || indices.iter().any(|index| *index >= texture_count) {
        panic!(
            "theme manifest {} has duplicate or out-of-range atlas cell indices",
            manifest_path.display()
        );
    }

    let expected_width = offset_x + columns * tile_width + columns.saturating_sub(1) * padding_x;
    let expected_height = offset_y + rows * tile_height + rows.saturating_sub(1) * padding_y;
    validate_atlas_image(
        &theme_dir.join(required_str(manifest, "sprites.atlas")),
        expected_width,
        expected_height,
    );
    if let Some(relative) = optional_str(manifest, "sprites.rated.atlas") {
        validate_atlas_image(&theme_dir.join(relative), expected_width, expected_height);
    }
}

fn validate_atlas_image(atlas_path: &Path, expected_width: u32, expected_height: u32) {
    let image = image::open(atlas_path)
        .unwrap_or_else(|error| panic!("decode atlas {} failed: {error}", atlas_path.display()));
    if image.width() < expected_width || image.height() < expected_height {
        panic!(
            "atlas {} is {}x{}, expected at least {}x{}",
            atlas_path.display(),
            image.width(),
            image.height(),
            expected_width,
            expected_height
        );
    }
}

fn validate_theme_colors(manifest: &Value, manifest_path: &Path) {
    for key in [
        "semantic.text.primary",
        "semantic.text.secondary",
        "semantic.text.accent",
        "semantic.text.warning",
        "semantic.board.background",
        "semantic.board.empty",
        "semantic.board.structure",
        "semantic.board.happy",
        "semantic.board.frown",
        "semantic.board.gimp",
        "semantic.board.die",
        "semantic.board.invisible",
        "semantic.board.hidden",
        "semantic.button.normal",
        "semantic.button.hover",
        "semantic.button.pressed",
        "semantic.button.text",
        "semantic.bazaar.affordable",
        "semantic.bazaar.unaffordable",
        "semantic.bazaar.selected",
        "semantic.weapon.active",
        "semantic.weapon.expired",
        "screen.background",
        "screen.title_text",
        "screen.body_text",
        "about.background",
        "about.title_text",
        "about.name_text",
        "about.credit_text",
        "about.button_face",
        "about.button_highlight",
        "about.button_shadow",
        "about.button_text",
    ] {
        validate_hex_color(required_str(manifest, key), manifest_path);
    }
    for color in required_str_array(manifest, "semantic.board.visible_colors", 19) {
        validate_hex_color(color, manifest_path);
    }
}

fn validate_theme_layout(manifest: &Value, manifest_path: &Path) {
    for key in [
        "layout.screens.startup",
        "layout.screens.challenge",
        "layout.screens.sleep",
        "layout.screens.about",
        "layout.screens.roster",
        "layout.screens.settings",
        "layout.screens.game",
        "layout.screens.game_recon",
        "layout.screens.bazaar",
    ] {
        if required_f64(manifest, &format!("{key}.width")) <= 0.0
            || required_f64(manifest, &format!("{key}.height")) <= 0.0
        {
            panic!(
                "theme manifest {} has invalid screen layout",
                manifest_path.display()
            );
        }
    }
    if required_f64(manifest, "layout.board.spacing") <= 0.0
        || required_f64(manifest, "cell.size") <= 0.0
        || required_f64(manifest, "cell.gap") < 0.0
        || required_f64(manifest, "cell.shadow") < 0.0
        || required_f64(manifest, "screen.title_font_size") <= 0.0
        || required_f64(manifest, "screen.body_font_size") <= 0.0
        || required_f64(manifest, "screen.button_font_size") <= 0.0
    {
        panic!(
            "theme manifest {} has invalid sizing",
            manifest_path.display()
        );
    }

    for key in [
        "startup_challenge",
        "startup_sleep",
        "startup_about",
        "startup_roster",
        "startup_quit",
        "startup_local_game",
        "startup_play_ernie",
        "startup_theme",
        "challenge_level_down",
        "challenge_level_up",
        "challenge_play_ernie",
        "challenge_back",
        "sleep_wake",
        "about_ok",
        "roster_back",
        "settings_back",
        "bazaar_catalog",
        "bazaar_arsenal",
        "bazaar_add",
        "bazaar_remove",
        "bazaar_done",
    ] {
        if required_f64(manifest, &format!("layout.rects.{key}.width")) <= 0.0
            || required_f64(manifest, &format!("layout.rects.{key}.height")) <= 0.0
        {
            panic!(
                "theme manifest {} has invalid rect {key}",
                manifest_path.display()
            );
        }
    }
}

fn decode_png(path: &Path, manifest_path: &Path) {
    image::open(path).unwrap_or_else(|error| {
        panic!(
            "theme manifest {} references undecodable PNG {}: {error}",
            manifest_path.display(),
            path.display()
        )
    });
}

fn validate_hex_color(value: &str, manifest_path: &Path) {
    let Some(hex) = value.strip_prefix('#') else {
        panic!(
            "theme manifest {} has non-hex color {value}",
            manifest_path.display()
        );
    };
    if hex.len() != 6 && hex.len() != 8 || !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        panic!(
            "theme manifest {} has invalid color {value}",
            manifest_path.display()
        );
    }
}

fn required_value<'a>(manifest: &'a Value, path: &str) -> &'a Value {
    let mut current = manifest;
    for part in path.split('.') {
        current = current
            .get(part)
            .unwrap_or_else(|| panic!("theme manifest missing {path}"));
    }
    current
}

fn optional_value<'a>(manifest: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = manifest;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn required_str<'a>(manifest: &'a Value, path: &str) -> &'a str {
    required_value(manifest, path)
        .as_str()
        .unwrap_or_else(|| panic!("theme manifest {path} must be a string"))
}

fn optional_str<'a>(manifest: &'a Value, path: &str) -> Option<&'a str> {
    optional_value(manifest, path).map(|value| {
        value
            .as_str()
            .unwrap_or_else(|| panic!("theme manifest {path} must be a string"))
    })
}

fn required_array<'a>(manifest: &'a Value, path: &str) -> &'a [Value] {
    required_value(manifest, path)
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("theme manifest {path} must be an array"))
}

fn required_str_array<'a>(manifest: &'a Value, path: &str, len: usize) -> Vec<&'a str> {
    let values = required_array(manifest, path);
    if values.len() != len {
        panic!("theme manifest {path} must have {len} entries");
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("theme manifest {path} entries must be strings"))
        })
        .collect()
}

fn required_i64(manifest: &Value, path: &str) -> i64 {
    required_value(manifest, path)
        .as_integer()
        .unwrap_or_else(|| panic!("theme manifest {path} must be an integer"))
}

fn required_u32(manifest: &Value, path: &str) -> u32 {
    required_i64(manifest, path)
        .try_into()
        .unwrap_or_else(|_| panic!("theme manifest {path} must fit u32"))
}

fn required_usize(manifest: &Value, path: &str) -> usize {
    required_i64(manifest, path)
        .try_into()
        .unwrap_or_else(|_| panic!("theme manifest {path} must fit usize"))
}

fn required_usize_array(manifest: &Value, path: &str, len: usize) -> Vec<usize> {
    let values = required_array(manifest, path);
    if values.len() != len {
        panic!("theme manifest {path} must have {len} entries");
    }
    values
        .iter()
        .map(|value| {
            value
                .as_integer()
                .unwrap_or_else(|| panic!("theme manifest {path} entries must be integers"))
                .try_into()
                .unwrap_or_else(|_| panic!("theme manifest {path} entries must fit usize"))
        })
        .collect()
}

fn required_f64(manifest: &Value, path: &str) -> f64 {
    required_value(manifest, path)
        .as_float()
        .or_else(|| {
            required_value(manifest, path)
                .as_integer()
                .map(|value| value as f64)
        })
        .unwrap_or_else(|| panic!("theme manifest {path} must be a number"))
}

fn required_bool(manifest: &Value, path: &str) -> bool {
    required_value(manifest, path)
        .as_bool()
        .unwrap_or_else(|| panic!("theme manifest {path} must be a boolean"))
}

fn generate_theme_assets(assets_dir: &Path) {
    let original = assets_dir.join("themes/original/images");
    let high_contrast = assets_dir.join("themes/high-contrast/images");
    let legacy_art = assets_dir.join("../usr/src/art");
    fs::create_dir_all(&original).expect("create original image directory");
    fs::create_dir_all(&high_contrast).expect("create high-contrast image directory");

    write_legacy_original_block_atlas(
        &original.join("blocks.png"),
        &legacy_art.join("btgimp2.ppm"),
    );
    write_legacy_original_block_atlas(
        &original.join("blocks-rated.png"),
        &legacy_art.join("btgimp.ppm"),
    );
    write_block_atlas(
        &high_contrast.join("blocks.png"),
        &[
            [0xff, 0x39, 0x39, 0xff],
            [0x30, 0xb7, 0xff, 0xff],
            [0xff, 0xf2, 0x33, 0xff],
            [0x47, 0xff, 0x6d, 0xff],
            [0xff, 0x55, 0xff, 0xff],
            [0xff, 0xff, 0xff, 0xff],
            [0xff, 0xc4, 0x00, 0xff],
            [0xff, 0x7a, 0x7a, 0xff],
            [0x6f, 0xe0, 0xff, 0xff],
            [0xd8, 0xff, 0x4f, 0xff],
            [0xff, 0x8c, 0xff, 0xff],
            [0xff, 0xa3, 0x47, 0xff],
            [0x7a, 0xff, 0xd9, 0xff],
            [0xb7, 0xff, 0x30, 0xff],
            [0xff, 0xd0, 0x00, 0xff],
            [0xff, 0x4f, 0xa3, 0xff],
            [0xb8, 0xc7, 0xff, 0xff],
            [0xff, 0xe0, 0xa3, 0xff],
            [0xcf, 0xcf, 0xcf, 0xff],
        ],
        AtlasSemanticColors {
            empty: [0x04, 0x05, 0x07, 0xff],
            structure: [0xcc, 0xd1, 0xdb, 0xff],
            happy: [0xff, 0xea, 0x00, 0xff],
            frown: [0xd1, 0x78, 0x29, 0xff],
            gimp: [0xff, 0x1a, 0xe8, 0xff],
            die: [0xc7, 0xd6, 0xff, 0xff],
            invisible: [0x33, 0x38, 0x42, 0x60],
            hidden: [0x00, 0x00, 0x00, 0xff],
        },
    );
    convert_ppm(
        &legacy_art.join("btstartup2.ppm"),
        &original.join("startup.png"),
        Some((640, 600)),
    );
    convert_ppm(
        &legacy_art.join("btbazaar.ppm"),
        &original.join("bazaar.png"),
        Some((800, 800)),
    );
    convert_ppm(
        &legacy_art.join("btbiff4.ppm"),
        &original.join("biff.png"),
        None,
    );
    convert_ppm(
        &legacy_art.join("btbiff4.ppm"),
        &original.join("crest.png"),
        None,
    );
    convert_ppm(
        &legacy_art.join("btgimp2.ppm"),
        &original.join("gimp.png"),
        None,
    );
    convert_ppm(
        &legacy_art.join("btgimp.ppm"),
        &original.join("gimp-rated.png"),
        None,
    );
    write_panel(
        &high_contrast.join("startup.png"),
        [0x10, 0x10, 0x10, 0xff],
        [0xff, 0xff, 0x00, 0xff],
    );
    write_panel(
        &high_contrast.join("bazaar.png"),
        [0x10, 0x10, 0x10, 0xff],
        [0x00, 0xff, 0xff, 0xff],
    );
    write_panel(
        &high_contrast.join("biff.png"),
        [0x10, 0x10, 0x10, 0xff],
        [0xff, 0x55, 0xff, 0xff],
    );
    convert_ppm(
        &legacy_art.join("btbiff1.ppm"),
        &high_contrast.join("crest.png"),
        None,
    );
    write_panel(
        &high_contrast.join("gimp.png"),
        [0x10, 0x10, 0x10, 0xff],
        [0xff, 0x55, 0xff, 0xff],
    );
}

#[derive(Debug, Clone, Copy)]
struct AtlasSemanticColors {
    empty: [u8; 4],
    structure: [u8; 4],
    happy: [u8; 4],
    frown: [u8; 4],
    gimp: [u8; 4],
    die: [u8; 4],
    invisible: [u8; 4],
    hidden: [u8; 4],
}

const LEGACY_CELL: u32 = 23;
const LEGACY_BOX_BORDER: u32 = 3;
const LEGACY_DIE_RAD: u32 = 5;
const LEGACY_DIE_X1: u32 = 1;
const LEGACY_DIE_X2: u32 = 7;
const LEGACY_DIE_X3: u32 = 13;
const LEGACY_DIE_Y1: u32 = 1;
const LEGACY_DIE_Y2: u32 = 7;
const LEGACY_DIE_Y3: u32 = 13;
const LEGACY_HAP_X1: u32 = 2;
const LEGACY_HAP_X2: u32 = 3;
const LEGACY_HAP_X3: u32 = 11;
const LEGACY_HAP_Y1: u32 = 1;
const LEGACY_HAP_Y2: u32 = 8;
const LEGACY_HAP_Y3: u32 = 13;
const LEGACY_HAP_XRAD: u32 = 4;
const LEGACY_HAP_YRAD: u32 = 7;
const LEGACY_HAP_XRAD2: u32 = 11;
const LEGACY_HAP_YRAD2: u32 = 5;

const BT_BLACK: usize = 0;
const BT_IVORY: usize = 1;
const BT_YELLOW: usize = 2;
const BT_BLUE: usize = 4;
const BT_NEUTRAL: usize = 9;
const BT_GRAY: usize = 10;
const BT_DYELLOW: usize = 11;
const BT_MAX_DIF_COLORS: usize = 9;

const LEGACY_PALETTE: [[u8; 4]; 18] = [
    legacy_rgb(0x000000),
    legacy_rgb(0xeeeee0),
    legacy_rgb(0xeeee00),
    legacy_rgb(0xee0000),
    legacy_rgb(0x0000cd),
    legacy_rgb(0xee9a00),
    legacy_rgb(0x32cd32),
    legacy_rgb(0x009acd),
    legacy_rgb(0xa020f0),
    legacy_rgb(0xbfbfbf),
    legacy_rgb(0xa8a8a8),
    legacy_rgb(0xdaa520),
    legacy_rgb(0x8b0000),
    legacy_rgb(0x00008b),
    legacy_rgb(0xda7600),
    legacy_rgb(0x228b22),
    legacy_rgb(0x436eee),
    legacy_rgb(0x68228b),
];

#[derive(Debug, Clone, Copy)]
struct LegacyRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl LegacyRect {
    const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LegacyArc {
    start_degrees: f32,
    extent_degrees: f32,
}

impl LegacyArc {
    const fn new(start_degrees: f32, extent_degrees: f32) -> Self {
        Self {
            start_degrees,
            extent_degrees,
        }
    }
}

const fn legacy_rgb(rgb: u32) -> [u8; 4] {
    [
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
        0xff,
    ]
}

fn write_legacy_original_block_atlas(path: &Path, gimp_path: &Path) {
    let mut image = RgbaImage::new(LEGACY_CELL * 32, LEGACY_CELL);

    for atlas_index in 1..=19 {
        let legacy_color = if atlas_index <= 8 {
            atlas_index
        } else {
            ((atlas_index - 1) % 8) + 1
        };
        draw_legacy_box(&mut image, atlas_index as u32 * LEGACY_CELL, legacy_color);
    }
    draw_legacy_structure(&mut image, 20 * LEGACY_CELL);
    draw_legacy_happy(&mut image, 21 * LEGACY_CELL, true);
    draw_legacy_happy(&mut image, 22 * LEGACY_CELL, false);
    draw_legacy_gimp(&mut image, 23 * LEGACY_CELL, gimp_path);
    for die in 1..=6 {
        draw_legacy_die(&mut image, (23 + die) * LEGACY_CELL, die);
    }
    fill_legacy_rect(
        &mut image,
        31 * LEGACY_CELL,
        LegacyRect::new(0, 0, LEGACY_CELL, LEGACY_CELL),
        LEGACY_PALETTE[BT_BLACK],
    );

    image.save(path).expect("write legacy block atlas png");
}

fn draw_legacy_box(image: &mut RgbaImage, x0: u32, color_index: usize) {
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(0, 0, LEGACY_CELL, LEGACY_CELL),
        LEGACY_PALETTE[color_index + BT_MAX_DIF_COLORS],
    );
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(
            0,
            0,
            LEGACY_CELL - LEGACY_BOX_BORDER,
            LEGACY_CELL - LEGACY_BOX_BORDER,
        ),
        LEGACY_PALETTE[color_index],
    );
}

fn draw_legacy_structure(image: &mut RgbaImage, x0: u32) {
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(0, 0, LEGACY_CELL, LEGACY_CELL),
        LEGACY_PALETTE[BT_NEUTRAL],
    );
}

fn draw_legacy_die(image: &mut RgbaImage, x0: u32, face: u32) {
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(0, 0, LEGACY_CELL, LEGACY_CELL),
        LEGACY_PALETTE[BT_GRAY],
    );
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(
            0,
            0,
            LEGACY_CELL - LEGACY_BOX_BORDER,
            LEGACY_CELL - LEGACY_BOX_BORDER,
        ),
        LEGACY_PALETTE[BT_IVORY],
    );

    if face > 1 {
        draw_legacy_pip(image, x0, LEGACY_DIE_X1, LEGACY_DIE_Y1);
        draw_legacy_pip(image, x0, LEGACY_DIE_X3, LEGACY_DIE_Y3);
    }
    if face > 3 {
        draw_legacy_pip(image, x0, LEGACY_DIE_X3, LEGACY_DIE_Y1);
        draw_legacy_pip(image, x0, LEGACY_DIE_X1, LEGACY_DIE_Y3);
    }
    if face % 2 == 1 {
        draw_legacy_pip(image, x0, LEGACY_DIE_X2, LEGACY_DIE_Y2);
    }
    if face == 6 {
        draw_legacy_pip(image, x0, LEGACY_DIE_X1, LEGACY_DIE_Y2);
        draw_legacy_pip(image, x0, LEGACY_DIE_X3, LEGACY_DIE_Y2);
    }
}

fn draw_legacy_pip(image: &mut RgbaImage, x0: u32, x: u32, y: u32) {
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(x, y, LEGACY_DIE_RAD, LEGACY_DIE_RAD),
        LEGACY_PALETTE[BT_GRAY],
    );
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(x + 1, y + 1, LEGACY_DIE_RAD - 2, LEGACY_DIE_RAD - 2),
        LEGACY_PALETTE[BT_BLACK],
    );
}

fn draw_legacy_happy(image: &mut RgbaImage, x0: u32, happy: bool) {
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(0, 0, LEGACY_CELL, LEGACY_CELL),
        LEGACY_PALETTE[BT_DYELLOW],
    );
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(
            0,
            0,
            LEGACY_CELL - LEGACY_BOX_BORDER,
            LEGACY_CELL - LEGACY_BOX_BORDER,
        ),
        LEGACY_PALETTE[BT_YELLOW],
    );
    draw_legacy_filled_ellipse(
        image,
        x0,
        LegacyRect::new(
            LEGACY_HAP_X1,
            LEGACY_HAP_Y1,
            LEGACY_HAP_XRAD,
            LEGACY_HAP_YRAD,
        ),
        LEGACY_PALETTE[BT_BLACK],
    );
    draw_legacy_filled_ellipse(
        image,
        x0,
        LegacyRect::new(
            LEGACY_HAP_X3,
            LEGACY_HAP_Y1,
            LEGACY_HAP_XRAD,
            LEGACY_HAP_YRAD,
        ),
        LEGACY_PALETTE[BT_BLACK],
    );

    if happy {
        draw_legacy_arc(
            image,
            x0,
            LegacyRect::new(
                LEGACY_HAP_X2,
                LEGACY_HAP_Y2,
                LEGACY_HAP_XRAD2,
                LEGACY_HAP_YRAD2,
            ),
            LegacyArc::new(180.0, 180.0),
            LEGACY_PALETTE[BT_BLACK],
        );
    } else {
        put_legacy_pixel(
            image,
            x0 + LEGACY_HAP_X3 + 1,
            LEGACY_HAP_Y1 + 7,
            LEGACY_PALETTE[BT_BLUE],
        );
        put_legacy_pixel(
            image,
            x0 + LEGACY_HAP_X3 + 1,
            LEGACY_HAP_Y1 + 8,
            LEGACY_PALETTE[BT_BLUE],
        );
        put_legacy_pixel(
            image,
            x0 + LEGACY_HAP_X3 + 2,
            LEGACY_HAP_Y1 + 8,
            LEGACY_PALETTE[BT_BLUE],
        );
        draw_legacy_filled_ellipse(
            image,
            x0,
            LegacyRect::new(LEGACY_HAP_X3, LEGACY_HAP_Y1 + 8, 3, 3),
            LEGACY_PALETTE[BT_BLUE],
        );
        draw_legacy_arc(
            image,
            x0,
            LegacyRect::new(
                LEGACY_HAP_X2,
                LEGACY_HAP_Y3,
                LEGACY_HAP_XRAD2,
                LEGACY_HAP_YRAD2,
            ),
            LegacyArc::new(0.0, 180.0),
            LEGACY_PALETTE[BT_BLACK],
        );
    }
}

fn draw_legacy_gimp(image: &mut RgbaImage, x0: u32, gimp_path: &Path) {
    fill_legacy_rect(
        image,
        x0,
        LegacyRect::new(0, 0, LEGACY_CELL, LEGACY_CELL),
        LEGACY_PALETTE[BT_BLACK],
    );
    let gimp = read_ppm(gimp_path);
    for y in 0..gimp.height().min(LEGACY_CELL) {
        for x in 0..gimp.width().min(LEGACY_CELL) {
            let pixel = quantize_legacy_ppm_pixel(*gimp.get_pixel(x, y));
            image.put_pixel(x0 + x, y, pixel);
        }
    }
}

fn quantize_legacy_ppm_pixel(pixel: Rgba<u8>) -> Rgba<u8> {
    Rgba([
        quantize_legacy_ppm_component(pixel[0]),
        quantize_legacy_ppm_component(pixel[1]),
        quantize_legacy_ppm_component(pixel[2]),
        pixel[3],
    ])
}

fn quantize_legacy_ppm_component(value: u8) -> u8 {
    let bucket = value as u16 * 4 / u8::MAX as u16;
    (bucket * u8::MAX as u16 / 4) as u8
}

fn fill_legacy_rect(image: &mut RgbaImage, x0: u32, rect: LegacyRect, color: [u8; 4]) {
    for py in rect.y..rect.y + rect.height {
        for px in rect.x..rect.x + rect.width {
            put_legacy_pixel(image, x0 + px, py, color);
        }
    }
}

fn draw_legacy_filled_ellipse(image: &mut RgbaImage, x0: u32, rect: LegacyRect, color: [u8; 4]) {
    let rx = rect.width as f32 / 2.0;
    let ry = rect.height as f32 / 2.0;
    let cx = rect.x as f32 + rx;
    let cy = rect.y as f32 + ry;
    for py in rect.y..rect.y + rect.height {
        for px in rect.x..rect.x + rect.width {
            let dx = (px as f32 + 0.5 - cx) / rx;
            let dy = (py as f32 + 0.5 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                put_legacy_pixel(image, x0 + px, py, color);
            }
        }
    }
}

fn draw_legacy_arc(
    image: &mut RgbaImage,
    x0: u32,
    rect: LegacyRect,
    arc: LegacyArc,
    color: [u8; 4],
) {
    let rx = rect.width as f32 / 2.0;
    let ry = rect.height as f32 / 2.0;
    let cx = x0 as f32 + rect.x as f32 + rx;
    let cy = rect.y as f32 + ry;
    let steps = (arc.extent_degrees.abs() * 4.0).ceil().max(1.0) as u32;
    for step in 0..=steps {
        let degrees = arc.start_degrees + arc.extent_degrees * step as f32 / steps as f32;
        let radians = degrees.to_radians();
        let px = (cx + rx * radians.cos()).round() as i32;
        let py = (cy - ry * radians.sin()).round() as i32;
        if px >= 0 && py >= 0 {
            put_legacy_pixel(image, px as u32, py as u32, color);
        }
    }
}

fn put_legacy_pixel(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4]) {
    if x < image.width() && y < image.height() {
        image.put_pixel(x, y, Rgba(color));
    }
}

fn write_block_atlas(path: &Path, visible_colors: &[[u8; 4]], semantic: AtlasSemanticColors) {
    let cell = 23;
    assert_eq!(
        visible_colors.len(),
        19,
        "cell atlas needs 19 visible colors"
    );
    let mut image = RgbaImage::new(cell * 32, cell);
    draw_shaded_block(&mut image, 0, semantic.empty);
    for (index, color) in visible_colors.iter().enumerate() {
        let x0 = (index as u32 + 1) * cell;
        draw_shaded_block(&mut image, x0, *color);
    }
    draw_shaded_block(&mut image, cell * 20, semantic.structure);
    draw_face(&mut image, cell * 21, true, semantic.happy);
    draw_face(&mut image, cell * 22, false, semantic.frown);
    draw_shaded_block(&mut image, cell * 23, semantic.gimp);
    draw_die_faces(&mut image, cell * 24, semantic.die);
    draw_shaded_block(&mut image, cell * 30, semantic.invisible);
    draw_shaded_block(&mut image, cell * 31, semantic.hidden);
    image.save(path).expect("write block atlas png");
}

fn draw_shaded_block(image: &mut RgbaImage, x0: u32, color: [u8; 4]) {
    let cell = 23;
    for y in 0..cell {
        for x in 0..cell {
            let shadow = x >= cell - 3 || y >= cell - 3;
            let highlight = x < 2 || y < 2;
            let shade = if shadow { 64 } else { 0 };
            let light = if highlight { 32 } else { 0 };
            image.put_pixel(
                x0 + x,
                y,
                Rgba([
                    color[0].saturating_add(light).saturating_sub(shade),
                    color[1].saturating_add(light).saturating_sub(shade),
                    color[2].saturating_add(light).saturating_sub(shade),
                    color[3],
                ]),
            );
        }
    }
}

fn draw_die_faces(image: &mut RgbaImage, x0: u32, color: [u8; 4]) {
    for face in 0..6 {
        let cell_x = x0 + face * 23;
        draw_shaded_block(image, cell_x, color);
        for &(px, py) in pip_positions(face + 1) {
            draw_disc(image, cell_x + px, py, 2, [0x08, 0x08, 0x08, 0xff]);
        }
    }
}

fn pip_positions(face: u32) -> &'static [(u32, u32)] {
    match face {
        1 => &[(11, 11)],
        2 => &[(7, 7), (15, 15)],
        3 => &[(7, 7), (11, 11), (15, 15)],
        4 => &[(7, 7), (15, 7), (7, 15), (15, 15)],
        5 => &[(7, 7), (15, 7), (11, 11), (7, 15), (15, 15)],
        _ => &[(7, 7), (15, 7), (7, 11), (15, 11), (7, 15), (15, 15)],
    }
}

fn draw_face(image: &mut RgbaImage, x0: u32, happy: bool, color: [u8; 4]) {
    draw_shaded_block(image, x0, color);
    draw_disc(image, x0 + 8, 8, 2, [0x20, 0x20, 0x20, 0xff]);
    draw_disc(image, x0 + 15, 8, 2, [0x20, 0x20, 0x20, 0xff]);
    if happy {
        for x in 7..=15 {
            let y = 13 + ((x as i32 - 11).abs() / 2) as u32;
            image.put_pixel(x0 + x, y, Rgba([0x20, 0x20, 0x20, 0xff]));
        }
    } else {
        for x in 7..=15 {
            let y = 16 - ((x as i32 - 11).abs() / 2) as u32;
            image.put_pixel(x0 + x, y, Rgba([0x20, 0x20, 0x20, 0xff]));
        }
        draw_disc(image, x0 + 16, 12, 1, [0x58, 0xa6, 0xff, 0xff]);
    }
}

fn draw_disc(image: &mut RgbaImage, cx: u32, cy: u32, radius: u32, color: [u8; 4]) {
    let radius_sq = (radius * radius) as i32;
    for y in cy.saturating_sub(radius)..=cy + radius {
        for x in cx.saturating_sub(radius)..=cx + radius {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            if dx * dx + dy * dy <= radius_sq && x < image.width() && y < image.height() {
                image.put_pixel(x, y, Rgba(color));
            }
        }
    }
}

fn convert_ppm(source: &Path, destination: &Path, resize: Option<(u32, u32)>) {
    let image = read_ppm(source);
    let image = if let Some((width, height)) = resize {
        image::imageops::resize(&image, width, height, FilterType::Nearest)
    } else {
        image
    };
    image.save(destination).unwrap_or_else(|error| {
        panic!(
            "convert legacy art {} to {} failed: {error}",
            source.display(),
            destination.display()
        )
    });
}

fn read_ppm(path: &Path) -> RgbaImage {
    let bytes = fs::read(path)
        .unwrap_or_else(|error| panic!("read PPM {} failed: {error}", path.display()));
    let mut cursor = 0;
    let magic = next_ppm_token(&bytes, &mut cursor);
    assert_eq!(magic, "P6", "{} is not a binary P6 PPM", path.display());
    let width = next_ppm_token(&bytes, &mut cursor)
        .parse::<u32>()
        .expect("PPM width");
    let height = next_ppm_token(&bytes, &mut cursor)
        .parse::<u32>()
        .expect("PPM height");
    let max_value = next_ppm_token(&bytes, &mut cursor)
        .parse::<u32>()
        .expect("PPM max value");
    assert_eq!(
        max_value,
        255,
        "{} has unsupported PPM max value",
        path.display()
    );
    if cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    let expected = width as usize * height as usize * 3;
    assert!(
        bytes.len().saturating_sub(cursor) >= expected,
        "{} has too few PPM pixel bytes",
        path.display()
    );
    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let index = cursor + ((y * width + x) as usize * 3);
            image.put_pixel(
                x,
                y,
                Rgba([bytes[index], bytes[index + 1], bytes[index + 2], 0xff]),
            );
        }
    }
    image
}

fn next_ppm_token(bytes: &[u8], cursor: &mut usize) -> String {
    loop {
        while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        }
        if *cursor < bytes.len() && bytes[*cursor] == b'#' {
            while *cursor < bytes.len() && bytes[*cursor] != b'\n' {
                *cursor += 1;
            }
            continue;
        }
        break;
    }
    let start = *cursor;
    while *cursor < bytes.len() && !bytes[*cursor].is_ascii_whitespace() {
        *cursor += 1;
    }
    std::str::from_utf8(&bytes[start..*cursor])
        .expect("PPM token is UTF-8")
        .to_string()
}

fn generate_sound_pack(assets_dir: &Path) {
    write_sound_pack(
        &assets_dir.join("sounds/generated-default"),
        "Generated Default",
        "Generated short WAV cues for semantic BattleTris client events.",
        SOUND_EVENTS,
        true,
    );
    write_sound_pack(
        &assets_dir.join("sounds/generated-rated"),
        "Generated Rated Overlay",
        "Generated rated-mode overlay cues for explicit legacy compatibility mode.",
        RATED_SOUND_EVENTS,
        false,
    );
}

struct SoundSpec {
    id: &'static str,
    file: &'static str,
    frequency_hz: f32,
    duration_ms: u32,
    volume: f32,
    wave: Waveform,
}

#[derive(Clone, Copy)]
enum Waveform {
    Sine,
    Square,
    Noise,
}

const SOUND_EVENT_IDS: &[&str] = &[
    "menu_action",
    "piece_locked",
    "line_clear",
    "bazaar_entered",
    "purchase",
    "weapon_launch",
    "weapon_launch_gimp",
    "challenge_incoming",
    "challenge_rejected",
    "bazaar_wait",
    "opponent_wait",
    "game_lost",
    "game_won",
    "game_dead",
    "about_easter_egg",
    "warning",
    "game_over",
];

const SOUND_EVENTS: &[SoundSpec] = &[
    SoundSpec {
        id: "menu_action",
        file: "menu-action.wav",
        frequency_hz: 440.0,
        duration_ms: 90,
        volume: 0.55,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "piece_locked",
        file: "piece-locked.wav",
        frequency_hz: 220.0,
        duration_ms: 65,
        volume: 0.45,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "line_clear",
        file: "line-clear.wav",
        frequency_hz: 660.0,
        duration_ms: 120,
        volume: 0.65,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "bazaar_entered",
        file: "bazaar-entered.wav",
        frequency_hz: 330.0,
        duration_ms: 180,
        volume: 0.65,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "purchase",
        file: "purchase.wav",
        frequency_hz: 550.0,
        duration_ms: 85,
        volume: 0.55,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "weapon_launch",
        file: "weapon-launch.wav",
        frequency_hz: 880.0,
        duration_ms: 150,
        volume: 0.75,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "weapon_launch_gimp",
        file: "weapon-launch-gimp.wav",
        frequency_hz: 740.0,
        duration_ms: 220,
        volume: 0.75,
        wave: Waveform::Square,
    },
    SoundSpec {
        id: "challenge_incoming",
        file: "challenge-incoming.wav",
        frequency_hz: 494.0,
        duration_ms: 140,
        volume: 0.6,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "challenge_rejected",
        file: "challenge-rejected.wav",
        frequency_hz: 196.0,
        duration_ms: 180,
        volume: 0.7,
        wave: Waveform::Square,
    },
    SoundSpec {
        id: "bazaar_wait",
        file: "bazaar-wait.wav",
        frequency_hz: 294.0,
        duration_ms: 160,
        volume: 0.55,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "opponent_wait",
        file: "opponent-wait.wav",
        frequency_hz: 262.0,
        duration_ms: 160,
        volume: 0.55,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "game_lost",
        file: "game-lost.wav",
        frequency_hz: 147.0,
        duration_ms: 260,
        volume: 0.75,
        wave: Waveform::Square,
    },
    SoundSpec {
        id: "game_won",
        file: "game-won.wav",
        frequency_hz: 784.0,
        duration_ms: 260,
        volume: 0.75,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "game_dead",
        file: "game-dead.wav",
        frequency_hz: 123.0,
        duration_ms: 300,
        volume: 0.75,
        wave: Waveform::Noise,
    },
    SoundSpec {
        id: "about_easter_egg",
        file: "about-easter-egg.wav",
        frequency_hz: 622.0,
        duration_ms: 240,
        volume: 0.55,
        wave: Waveform::Sine,
    },
    SoundSpec {
        id: "warning",
        file: "warning.wav",
        frequency_hz: 110.0,
        duration_ms: 200,
        volume: 0.75,
        wave: Waveform::Square,
    },
    SoundSpec {
        id: "game_over",
        file: "game-over.wav",
        frequency_hz: 165.0,
        duration_ms: 260,
        volume: 0.75,
        wave: Waveform::Square,
    },
];

const RATED_SOUND_EVENTS: &[SoundSpec] = &[
    SoundSpec {
        id: "weapon_launch_gimp",
        file: "weapon-launch-gimp-rated.wav",
        frequency_hz: 92.0,
        duration_ms: 260,
        volume: 0.8,
        wave: Waveform::Noise,
    },
    SoundSpec {
        id: "challenge_rejected",
        file: "challenge-rejected-rated.wav",
        frequency_hz: 130.0,
        duration_ms: 240,
        volume: 0.8,
        wave: Waveform::Noise,
    },
    SoundSpec {
        id: "bazaar_wait",
        file: "bazaar-wait-rated.wav",
        frequency_hz: 185.0,
        duration_ms: 180,
        volume: 0.65,
        wave: Waveform::Square,
    },
    SoundSpec {
        id: "game_lost",
        file: "game-lost-rated.wav",
        frequency_hz: 98.0,
        duration_ms: 320,
        volume: 0.85,
        wave: Waveform::Noise,
    },
    SoundSpec {
        id: "game_dead",
        file: "game-dead-rated.wav",
        frequency_hz: 82.0,
        duration_ms: 340,
        volume: 0.85,
        wave: Waveform::Noise,
    },
    SoundSpec {
        id: "warning",
        file: "warning-rated.wav",
        frequency_hz: 104.0,
        duration_ms: 240,
        volume: 0.8,
        wave: Waveform::Noise,
    },
    SoundSpec {
        id: "game_over",
        file: "game-over-rated.wav",
        frequency_hz: 110.0,
        duration_ms: 320,
        volume: 0.85,
        wave: Waveform::Noise,
    },
];

fn write_sound_pack(
    sounds: &Path,
    name: &str,
    description: &str,
    events: &[SoundSpec],
    require_all_events: bool,
) {
    fs::create_dir_all(sounds).expect("create generated sound directory");
    for event in events {
        write_wav(
            &sounds.join(event.file),
            event.frequency_hz,
            event.duration_ms,
            event.wave,
        );
    }
    write_sound_manifest(sounds, name, description, events);
    validate_sound_pack(sounds, require_all_events);
}

fn write_sound_manifest(sounds: &Path, name: &str, description: &str, events: &[SoundSpec]) {
    let mut manifest = format!(
        "name = \"{name}\"\nkind = \"sound-pack\"\nformat_version = 1\ndescription = \"{description}\"\n"
    );
    for event in events {
        manifest.push_str(&format!(
            "\n[[event]]\nid = \"{}\"\nfiles = [\"{}\"]\nvolume = {:.2}\n",
            event.id, event.file, event.volume
        ));
    }
    fs::write(sounds.join("sound-pack.toml"), manifest).expect("write sound-pack manifest");
}

fn write_wav(path: &Path, frequency_hz: f32, duration_ms: u32, waveform: Waveform) {
    let sample_rate = 44_100_u32;
    let sample_count = sample_rate * duration_ms / 1_000;
    let data_bytes = sample_count * 2;
    let mut file = fs::File::create(path).expect("create generated wav");
    file.write_all(b"RIFF").expect("write wav riff");
    file.write_all(&(36 + data_bytes).to_le_bytes())
        .expect("write wav size");
    file.write_all(b"WAVEfmt ").expect("write wav wave fmt");
    file.write_all(&16_u32.to_le_bytes())
        .expect("write wav fmt size");
    file.write_all(&1_u16.to_le_bytes()).expect("write wav pcm");
    file.write_all(&1_u16.to_le_bytes())
        .expect("write wav channels");
    file.write_all(&sample_rate.to_le_bytes())
        .expect("write wav sample rate");
    file.write_all(&(sample_rate * 2).to_le_bytes())
        .expect("write wav byte rate");
    file.write_all(&2_u16.to_le_bytes())
        .expect("write wav block align");
    file.write_all(&16_u16.to_le_bytes())
        .expect("write wav bits");
    file.write_all(b"data").expect("write wav data tag");
    file.write_all(&data_bytes.to_le_bytes())
        .expect("write wav data size");
    for sample in 0..sample_count {
        let t = sample as f32 / sample_rate as f32;
        let fade = 1.0 - (sample as f32 / sample_count as f32);
        let phase = t * frequency_hz;
        let raw = match waveform {
            Waveform::Sine => (phase * std::f32::consts::TAU).sin(),
            Waveform::Square => {
                if phase.fract() < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Noise => deterministic_noise(sample),
        };
        let value = raw * fade * 0.25;
        let pcm = (value * i16::MAX as f32) as i16;
        file.write_all(&pcm.to_le_bytes())
            .expect("write wav sample");
    }
}

fn deterministic_noise(sample: u32) -> f32 {
    let mut value = sample.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    value ^= value >> 16;
    (value as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn validate_wav(path: &Path, manifest_path: &Path) {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "sound-pack manifest {} could not read WAV {}: {error}",
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
            "sound-pack manifest {} references undecodable PCM WAV {}",
            manifest_path.display(),
            path.display()
        );
    }
}

fn write_panel(path: &Path, background: [u8; 4], foreground: [u8; 4]) {
    let mut image = RgbaImage::new(160, 120);
    for y in 0..120 {
        for x in 0..160 {
            let border = !(8..152).contains(&x) || !(8..112).contains(&y);
            image.put_pixel(x, y, Rgba(if border { foreground } else { background }));
        }
    }
    image.save(path).expect("write panel png");
}
