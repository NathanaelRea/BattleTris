//! Compositor-backed smoke tests for the Bevy client window.

use std::{path::PathBuf, process::Command};

#[test]
#[ignore = "requires a running desktop compositor, such as Hyprland"]
fn startup_screenshot_is_not_black() {
    let screenshot_path = smoke_screenshot_path();
    let output = Command::new(env!("CARGO_BIN_EXE_client"))
        .arg("--smoke-screenshot")
        .arg(&screenshot_path)
        .env("RUST_LOG", "info,wgpu=warn,naga=warn")
        .output()
        .expect("client smoke run starts");

    assert!(
        output.status.success(),
        "client smoke run failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        screenshot_path.exists(),
        "client smoke run did not create {}\nstdout:\n{}\nstderr:\n{}",
        screenshot_path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let image = image::open(&screenshot_path)
        .unwrap_or_else(|error| panic!("could not open {}: {error}", screenshot_path.display()))
        .to_rgb8();
    let (width, height) = image.dimensions();
    let total_pixels = u64::from(width) * u64::from(height);
    let mut bright_pixels = 0_u64;
    let mut min_luma = u8::MAX;
    let mut max_luma = u8::MIN;

    for pixel in image.pixels() {
        let [red, green, blue] = pixel.0;
        let luma = ((u16::from(red) * 2126 + u16::from(green) * 7152 + u16::from(blue) * 722)
            / 10_000) as u8;
        if luma > 80 {
            bright_pixels += 1;
        }
        min_luma = min_luma.min(luma);
        max_luma = max_luma.max(luma);
    }

    assert!(
        bright_pixels > total_pixels / 1_000,
        "startup screenshot looks black: bright_pixels={bright_pixels} total_pixels={total_pixels} luma_range={min_luma}..{max_luma} path={}",
        screenshot_path.display()
    );
    assert!(
        max_luma.saturating_sub(min_luma) > 40,
        "startup screenshot lacks visible contrast: luma_range={min_luma}..{max_luma} path={}",
        screenshot_path.display()
    );
}

fn smoke_screenshot_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "battletris-startup-smoke-{}.png",
        std::process::id()
    ))
}
