//! Client command-line parsing and run configuration.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct ClientRunConfig {
    pub(super) capture: Option<VisualCaptureSpec>,
    pub(super) deterministic_capture: bool,
    pub(super) content_mode: ContentMode,
    pub(super) session_overrides: ClientSessionOverrides,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ClientSessionOverrides {
    pub(super) start_screen: Option<ClientScreen>,
    pub(super) mute: bool,
    pub(super) no_server: bool,
    pub(super) lobby_host: Option<String>,
    pub(super) lobby_port: Option<u16>,
}

impl ClientSessionOverrides {
    pub(super) fn apply_to(&self, settings: &mut ClientSettings) {
        if let Some(screen) = self.start_screen {
            settings.screen = screen;
        }
        if self.mute {
            settings.sound_pack = SoundPackChoice::Muted;
        }
        if self.no_server {
            settings.lobby_enabled = false;
        }
        if self.lobby_host.is_some() || self.lobby_port.is_some() {
            settings.legacy_server_addr = server_addr_with_overrides(
                &settings.legacy_server_addr,
                self.lobby_host.as_deref(),
                self.lobby_port,
            )
            .expect("legacy lobby override was validated during CLI parsing");
        }
    }
}

impl ClientRunConfig {
    pub(super) fn from_env() -> Result<Self, String> {
        let args = std::env::args_os().skip(1).collect::<Vec<_>>();
        if args.len() == 1 && is_help_arg(&args[0])
            || args
                .first()
                .is_some_and(|arg| arg == OsStr::new("headless"))
                && args.get(1).is_some_and(|arg| is_help_arg(arg))
        {
            println!("{}", client_usage());
            std::process::exit(0);
        }
        Self::parse(args, std::env::var_os("BATTLETRIS_SMOKE_SCREENSHOT"))
    }

    pub(super) fn parse(args: Vec<OsString>, smoke_env: Option<OsString>) -> Result<Self, String> {
        let (content_mode, session_overrides, args) = parse_legacy_session_args(args)?;
        if args.is_empty() {
            return Ok(Self {
                capture: smoke_env.map(|path| VisualCaptureSpec::Smoke { path: path.into() }),
                deterministic_capture: false,
                content_mode,
                session_overrides,
            });
        }

        if args
            .first()
            .is_some_and(|arg| arg == OsStr::new("headless"))
        {
            return parse_headless_args(&args[1..], content_mode, session_overrides);
        }

        if args.len() == 1 && is_help_arg(&args[0]) {
            return Err(client_usage());
        }

        let mut index = 0;
        let mut smoke_path = smoke_env.map(PathBuf::from);
        while index < args.len() {
            let arg = &args[index];
            if arg == OsStr::new("--smoke-screenshot") {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--smoke-screenshot requires a path".to_string());
                };
                smoke_path = Some(PathBuf::from(path));
            } else if let Some(path) = arg
                .to_str()
                .and_then(|arg| arg.strip_prefix("--smoke-screenshot="))
            {
                smoke_path = Some(PathBuf::from(path));
            } else {
                return Err(format!(
                    "unrecognized client argument: {}",
                    display_arg(arg)
                ));
            }
            index += 1;
        }

        Ok(Self {
            capture: smoke_path.map(|path| VisualCaptureSpec::Smoke { path }),
            deterministic_capture: false,
            content_mode,
            session_overrides,
        })
    }
}

pub(super) fn parse_legacy_session_args(
    args: Vec<OsString>,
) -> Result<(ContentMode, ClientSessionOverrides, Vec<OsString>), String> {
    let mut content_mode = ContentMode::Normal;
    let mut session_overrides = ClientSessionOverrides::default();
    let mut remaining = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == OsStr::new("--rated") || arg == OsStr::new("-r") {
            content_mode = ContentMode::Rated;
        } else if arg == OsStr::new("--sleep") || arg == OsStr::new("-s") {
            session_overrides.start_screen = Some(ClientScreen::Sleep);
        } else if arg == OsStr::new("--mute") || arg == OsStr::new("-m") {
            session_overrides.mute = true;
        } else if arg == OsStr::new("--no-server") || arg == OsStr::new("-X") {
            session_overrides.no_server = true;
        } else if arg == OsStr::new("--headphones") || arg == OsStr::new("-p") {
            // Accepted for legacy CLI compatibility; modern audio has no speakerbox route.
        } else if arg == OsStr::new("--a-team") || arg == OsStr::new("-a") {
            // Accepted for legacy CLI compatibility; the A-Team welcome sound is not shipped.
        } else if arg == OsStr::new("--server-host") || arg == OsStr::new("-S") {
            index += 1;
            session_overrides.lobby_host =
                Some(required_arg(&args, index, display_arg(arg).as_str())?.to_string());
        } else if let Some(host) = option_value(arg, "--server-host") {
            session_overrides.lobby_host = Some(host.to_string());
        } else if arg == OsStr::new("--server-port") || arg == OsStr::new("-P") {
            index += 1;
            session_overrides.lobby_port = Some(parse_lobby_port(required_arg(
                &args,
                index,
                display_arg(arg).as_str(),
            )?)?);
        } else if let Some(port) = option_value(arg, "--server-port") {
            session_overrides.lobby_port = Some(parse_lobby_port(port)?);
        } else if arg == OsStr::new("-xrm") || arg == OsStr::new("--xrm") {
            index += 1;
            parse_xrm_override(
                required_arg(&args, index, display_arg(arg).as_str())?,
                &mut content_mode,
                &mut session_overrides,
            )?;
        } else if let Some(resource) = option_value(arg, "--xrm") {
            parse_xrm_override(resource, &mut content_mode, &mut session_overrides)?;
        } else {
            remaining.push(arg.clone());
        }
        index += 1;
    }
    if session_overrides.lobby_host.is_some() || session_overrides.lobby_port.is_some() {
        server_addr_with_overrides(
            DEFAULT_LEGACY_SERVER_ADDR,
            session_overrides.lobby_host.as_deref(),
            session_overrides.lobby_port,
        )?;
    }
    Ok((content_mode, session_overrides, remaining))
}

pub(super) fn parse_lobby_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|error| format!("invalid server port '{value}': {error}"))
}

pub(super) fn parse_xrm_override(
    resource: &str,
    content_mode: &mut ContentMode,
    session_overrides: &mut ClientSessionOverrides,
) -> Result<(), String> {
    let Some((name, value)) = resource
        .split_once(':')
        .or_else(|| resource.split_once('='))
    else {
        return Err(format!(
            "-xrm resource override must be name: value, got '{resource}'"
        ));
    };
    let resource_name = canonical_xrm_resource_name(name);
    let value = value.trim();
    match resource_name.as_str() {
        "sleep" => {
            session_overrides.start_screen = parse_xrm_bool(value)?.then_some(ClientScreen::Sleep)
        }
        "rrated" => {
            *content_mode = if parse_xrm_bool(value)? {
                ContentMode::Rated
            } else {
                ContentMode::Normal
            };
        }
        "mute" => session_overrides.mute = parse_xrm_bool(value)?,
        "noserver" => session_overrides.no_server = parse_xrm_bool(value)?,
        "serverhost" => session_overrides.lobby_host = Some(value.to_string()),
        "serverport" => session_overrides.lobby_port = Some(parse_lobby_port(value)?),
        "headphones" | "ateam" | "keymappings" => {}
        _ => {}
    }
    Ok(())
}

pub(super) fn canonical_xrm_resource_name(name: &str) -> String {
    name.rsplit(['*', '.'])
        .next()
        .unwrap_or(name)
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn parse_xrm_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        _ => Err(format!("invalid boolean resource value '{value}'")),
    }
}

pub(super) fn server_addr_with_overrides(
    current: &str,
    host_override: Option<&str>,
    port_override: Option<u16>,
) -> Result<String, String> {
    let current = current
        .parse::<SocketAddr>()
        .map_err(|error| format!("current lobby address '{current}' is invalid: {error}"))?;
    let host_socket = host_override.and_then(|host| host.trim().parse::<SocketAddr>().ok());
    let port = port_override
        .or_else(|| host_socket.map(|addr| addr.port()))
        .unwrap_or_else(|| current.port());
    let host = if let Some(addr) = host_socket {
        addr.ip().to_string()
    } else {
        host_override
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| current.ip().to_string())
    };
    let candidate = match host.parse::<IpAddr>() {
        Ok(ip) => SocketAddr::new(ip, port).to_string(),
        Err(_) => format!("{host}:{port}"),
    };
    match candidate.parse::<SocketAddr>() {
        Ok(_) => Ok(candidate),
        Err(error) => Err(format!(
            "invalid server host/port override '{candidate}': {error}"
        )),
    }
}

pub(super) fn parse_headless_args(
    args: &[OsString],
    content_mode: ContentMode,
    session_overrides: ClientSessionOverrides,
) -> Result<ClientRunConfig, String> {
    let Some(command) = args.first() else {
        return Err("headless requires a command: capture or capture-all".to_string());
    };
    if is_help_arg(command) {
        return Err(client_usage());
    }

    match command.to_str() {
        Some("capture") => parse_headless_capture_args(&args[1..], content_mode, session_overrides),
        Some("capture-all") => {
            parse_headless_capture_all_args(&args[1..], content_mode, session_overrides)
        }
        Some(other) => Err(format!("unrecognized headless command: {other}")),
        None => Err(format!(
            "headless command is not valid UTF-8: {}",
            display_arg(command)
        )),
    }
}

pub(super) fn parse_headless_capture_args(
    args: &[OsString],
    content_mode: ContentMode,
    session_overrides: ClientSessionOverrides,
) -> Result<ClientRunConfig, String> {
    let mut fixture = None;
    let mut theme = ThemeChoice::Original;
    let mut output = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if let Some(value) = option_value(arg, "--fixture") {
            fixture = Some(parse_visual_fixture(value)?);
        } else if arg == OsStr::new("--fixture") {
            index += 1;
            fixture = Some(parse_visual_fixture(required_arg(
                args,
                index,
                "--fixture",
            )?)?);
        } else if let Some(value) = option_value(arg, "--theme") {
            theme = parse_theme_choice(value)?;
        } else if arg == OsStr::new("--theme") {
            index += 1;
            theme = parse_theme_choice(required_arg(args, index, "--theme")?)?;
        } else if let Some(value) = option_value(arg, "--output") {
            output = Some(PathBuf::from(value));
        } else if arg == OsStr::new("--output") {
            index += 1;
            output = Some(PathBuf::from(required_os_arg(args, index, "--output")?));
        } else {
            return Err(format!(
                "unrecognized headless capture argument: {}",
                display_arg(arg)
            ));
        }
        index += 1;
    }

    Ok(ClientRunConfig {
        capture: Some(VisualCaptureSpec::One {
            fixture: fixture.ok_or_else(|| "headless capture requires --fixture".to_string())?,
            theme,
            output: output.ok_or_else(|| "headless capture requires --output".to_string())?,
        }),
        deterministic_capture: true,
        content_mode,
        session_overrides,
    })
}

pub(super) fn parse_headless_capture_all_args(
    args: &[OsString],
    content_mode: ContentMode,
    session_overrides: ClientSessionOverrides,
) -> Result<ClientRunConfig, String> {
    let mut theme = ThemeChoice::Original;
    let mut out_dir = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if let Some(value) = option_value(arg, "--theme") {
            theme = parse_theme_choice(value)?;
        } else if arg == OsStr::new("--theme") {
            index += 1;
            theme = parse_theme_choice(required_arg(args, index, "--theme")?)?;
        } else if let Some(value) = option_value(arg, "--out-dir") {
            out_dir = Some(PathBuf::from(value));
        } else if arg == OsStr::new("--out-dir") {
            index += 1;
            out_dir = Some(PathBuf::from(required_os_arg(args, index, "--out-dir")?));
        } else {
            return Err(format!(
                "unrecognized headless capture-all argument: {}",
                display_arg(arg)
            ));
        }
        index += 1;
    }

    Ok(ClientRunConfig {
        capture: Some(VisualCaptureSpec::All {
            theme,
            out_dir: out_dir
                .ok_or_else(|| "headless capture-all requires --out-dir".to_string())?,
        }),
        deterministic_capture: true,
        content_mode,
        session_overrides,
    })
}

pub(super) fn parse_visual_fixture(value: &str) -> Result<VisualFixture, String> {
    VisualFixture::from_id(value).ok_or_else(|| {
        format!(
            "unknown visual fixture '{value}'; expected one of: {}",
            visual_fixture_list()
        )
    })
}

pub(super) fn parse_theme_choice(value: &str) -> Result<ThemeChoice, String> {
    ThemeChoice::from_id(value)
        .ok_or_else(|| format!("unknown theme '{value}'; expected original or high-contrast"))
}

pub(super) fn required_arg<'a>(
    args: &'a [OsString],
    index: usize,
    option: &str,
) -> Result<&'a str, String> {
    required_os_arg(args, index, option)?
        .to_str()
        .ok_or_else(|| {
            format!(
                "{option} value is not valid UTF-8: {}",
                display_arg(&args[index])
            )
        })
}

pub(super) fn required_os_arg<'a>(
    args: &'a [OsString],
    index: usize,
    option: &str,
) -> Result<&'a OsStr, String> {
    args.get(index)
        .map(OsString::as_os_str)
        .ok_or_else(|| format!("{option} requires a value"))
}

pub(super) fn option_value<'a>(arg: &'a OsStr, option: &str) -> Option<&'a str> {
    arg.to_str()
        .and_then(|arg| arg.strip_prefix(option))
        .and_then(|rest| rest.strip_prefix('='))
}

pub(super) fn display_arg(arg: &OsStr) -> String {
    arg.to_string_lossy().into_owned()
}

pub(super) fn is_help_arg(arg: &OsStr) -> bool {
    arg == OsStr::new("--help") || arg == OsStr::new("-h")
}

pub(super) fn visual_fixture_list() -> String {
    VisualFixture::ALL
        .into_iter()
        .map(VisualFixture::id)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn client_usage() -> String {
    format!(
        "Usage:\n  client [options]\n  client [options] --smoke-screenshot <path>\n  client [options] headless capture --fixture <fixture> --theme <theme> --output <path>\n  client [options] headless capture-all --theme <theme> --out-dir <dir>\n\nOptions:\n  -r, --rated              Enable legacy rated content for this run\n  -s, --sleep              Start on the Sleep screen\n  -m, --mute               Mute sound for this run\n  -X, --no-server          Disable self-hosted lobby/server features for this run\n  -S, --server-host <ip>   Override lobby server host for this run\n  -P, --server-port <port> Override lobby server port for this run\n  -xrm <resource: value>   Apply a legacy X resource override for known resources\n  -p, --headphones         Accepted as a legacy no-op\n  -a, --a-team             Accepted as a legacy no-op\n\nKnown -xrm resources: sleep, r_rated, mute, no_server, serverHost, serverPort.\nServer host overrides currently require a numeric IP address.\nFixtures: {}\nThemes: original, high-contrast",
        visual_fixture_list()
    )
}
