//! Developer and content tool entry point.
//!
//! This crate will collect asset conversion, generated audio, replay inspection,
//! protocol fixture utilities, legacy data extraction, and future admin tools.

use image::{imageops::FilterType, Rgba, RgbaImage};
use std::{env, fs, io::Write, path::Path};

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
        Some("generate-assets") => {
            let assets_dir = args.next().unwrap_or_else(|| "assets".to_string());
            let assets_dir = Path::new(&assets_dir);
            generate_theme_assets(assets_dir);
            generate_sound_pack(assets_dir);
        }
        _ => {
            eprintln!(
                "usage: tools generate-assets [assets-dir]\n       tools generate-theme-assets [assets-dir]\n       tools generate-sound-pack [assets-dir]"
            );
        }
    }
}

fn generate_theme_assets(assets_dir: &Path) {
    let original = assets_dir.join("themes/original-inspired/images");
    let high_contrast = assets_dir.join("themes/high-contrast/images");
    fs::create_dir_all(&original).expect("create original-inspired image directory");
    fs::create_dir_all(&high_contrast).expect("create high-contrast image directory");

    write_block_atlas(
        &original.join("blocks.png"),
        &[
            [0xb8, 0x4a, 0x3a, 0xff],
            [0x4f, 0x88, 0xc6, 0xff],
            [0xe1, 0xbf, 0x45, 0xff],
            [0x6f, 0xa8, 0x5d, 0xff],
            [0x9a, 0x62, 0xb5, 0xff],
            [0xd9, 0xd0, 0xb5, 0xff],
            [0xff, 0xd8, 0x4a, 0xff],
            [0x94, 0x3b, 0x91, 0xff],
        ],
    );
    write_block_atlas(
        &high_contrast.join("blocks.png"),
        &[
            [0xff, 0x39, 0x39, 0xff],
            [0x30, 0xb7, 0xff, 0xff],
            [0xff, 0xf2, 0x33, 0xff],
            [0x47, 0xff, 0x6d, 0xff],
            [0xff, 0x55, 0xff, 0xff],
            [0xf4, 0xf4, 0xf4, 0xff],
            [0xff, 0xc4, 0x00, 0xff],
            [0x00, 0x00, 0x00, 0xff],
        ],
    );
    let legacy_art = assets_dir.join("../usr/src/art");
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
        &legacy_art.join("btbiff1.ppm"),
        &original.join("biff.png"),
        None,
    );
    convert_ppm(
        &legacy_art.join("btgimp.ppm"),
        &original.join("gimp.png"),
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
    write_panel(
        &high_contrast.join("gimp.png"),
        [0x10, 0x10, 0x10, 0xff],
        [0xff, 0x55, 0xff, 0xff],
    );
}

fn write_block_atlas(path: &Path, colors: &[[u8; 4]]) {
    let cell = 23;
    let extra_cells = 10;
    let mut image = RgbaImage::new(cell * (colors.len() as u32 + extra_cells), cell);
    for (index, color) in colors.iter().enumerate() {
        let x0 = index as u32 * cell;
        draw_shaded_block(&mut image, x0, *color);
    }
    draw_die_faces(&mut image, cell * colors.len() as u32);
    draw_face(&mut image, cell * (colors.len() as u32 + 6), true);
    draw_face(&mut image, cell * (colors.len() as u32 + 7), false);
    draw_shaded_block(
        &mut image,
        cell * (colors.len() as u32 + 8),
        [0x94, 0x3b, 0x91, 0xff],
    );
    draw_shaded_block(
        &mut image,
        cell * (colors.len() as u32 + 9),
        [0x18, 0x1a, 0x1c, 0x80],
    );
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

fn draw_die_faces(image: &mut RgbaImage, x0: u32) {
    for face in 0..6 {
        let cell_x = x0 + face * 23;
        draw_shaded_block(image, cell_x, [0xd9, 0xd2, 0xba, 0xff]);
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

fn draw_face(image: &mut RgbaImage, x0: u32, happy: bool) {
    draw_shaded_block(image, x0, [0xff, 0xd8, 0x4a, 0xff]);
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
    let sounds = assets_dir.join("sounds/generated-default");
    fs::create_dir_all(&sounds).expect("create generated sound directory");
    for event in SOUND_EVENTS {
        write_wav(
            &sounds.join(event.file),
            event.frequency_hz,
            event.duration_ms,
        );
    }
}

struct SoundSpec {
    file: &'static str,
    frequency_hz: f32,
    duration_ms: u32,
}

const SOUND_EVENTS: &[SoundSpec] = &[
    SoundSpec {
        file: "menu-action.wav",
        frequency_hz: 440.0,
        duration_ms: 90,
    },
    SoundSpec {
        file: "piece-locked.wav",
        frequency_hz: 220.0,
        duration_ms: 65,
    },
    SoundSpec {
        file: "line-clear.wav",
        frequency_hz: 660.0,
        duration_ms: 120,
    },
    SoundSpec {
        file: "bazaar-entered.wav",
        frequency_hz: 330.0,
        duration_ms: 180,
    },
    SoundSpec {
        file: "purchase.wav",
        frequency_hz: 550.0,
        duration_ms: 85,
    },
    SoundSpec {
        file: "weapon-launch.wav",
        frequency_hz: 880.0,
        duration_ms: 150,
    },
    SoundSpec {
        file: "warning.wav",
        frequency_hz: 110.0,
        duration_ms: 200,
    },
    SoundSpec {
        file: "game-over.wav",
        frequency_hz: 165.0,
        duration_ms: 260,
    },
];

fn write_wav(path: &Path, frequency_hz: f32, duration_ms: u32) {
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
        let value = (t * frequency_hz * std::f32::consts::TAU).sin() * fade * 0.25;
        let pcm = (value * i16::MAX as f32) as i16;
        file.write_all(&pcm.to_le_bytes())
            .expect("write wav sample");
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
