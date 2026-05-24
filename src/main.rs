use std::{ffi::OsString, path::PathBuf};

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

fn print_help() {
    println!(
        "{name} {version}\n{description}\n\nUsage:\n  {name} [OPTIONS] [initial-path]\n\nArguments:\n  [initial-path]  Optional initial page path, e.g. /docker/container\n\nOptions:\n  -c, --config <path>  Use a custom config file or config directory\n  -h, --help           Print help\n  -V, --version        Print version",
        name = APP_NAME,
        version = APP_VERSION,
        description = APP_DESCRIPTION,
    );
}

fn print_version() {
    println!("{APP_NAME} {APP_VERSION}");
}

pub use app::App;
pub use events::Event;
pub use keymap::*;
pub use mode::*;
pub use page::*;
pub use state::*;
pub use state::{ConfirmButton, ConfirmDialog, SelectDialog, SelectOption};
use tokio::task;
pub use widgets::InputDialogState;
pub use widgets::InputState;

mod app;
mod confirm_handler;
mod errors;
mod events;
mod input_handler;
mod keymap;
mod log;
mod mode;
mod page;
mod path_codec;
mod plugin;
mod select_handler;
mod state;
mod term;
mod widgets;

#[derive(Debug, Default)]
struct CliOptions {
    initial_path: Vec<String>,
    config_path: Option<PathBuf>,
}

fn parse_cli_options(args: impl IntoIterator<Item = OsString>) -> anyhow::Result<Option<CliOptions>> {
    let mut args = args.into_iter();
    let _program = args.next();

    let mut opt = CliOptions::default();
    let mut initial_path_set = false;
    let mut args = args.peekable();

    while let Some(raw_arg) = args.next() {
        let arg = raw_arg
            .into_string()
            .map_err(|_| anyhow::anyhow!("argument must be valid UTF-8"))?;

        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "-V" | "--version" => {
                print_version();
                return Ok(None);
            }
            "-c" | "--config" => {
                let Some(raw_path) = args.next() else {
                    anyhow::bail!("Option {arg} requires a path argument");
                };
                opt.config_path = Some(PathBuf::from(raw_path));
            }
            _ if arg.starts_with("--config=") => {
                let path = arg.trim_start_matches("--config=");
                if path.is_empty() {
                    anyhow::bail!("Option --config requires a path argument");
                }
                opt.config_path = Some(PathBuf::from(path));
            }
            _ if arg.starts_with('-') => {
                anyhow::bail!(
                    "Unknown option: {arg}\nTry '{APP_NAME} --help' for more information."
                );
            }
            _ => {
                if initial_path_set {
                    anyhow::bail!(
                        "Usage: {APP_NAME} [OPTIONS] [initial-path]\nTry '{APP_NAME} --help' for more information."
                    );
                }

                let trimmed = arg.trim_matches('/');
                let path = if trimmed.is_empty() {
                    Vec::new()
                } else {
                    trimmed
                        .split('/')
                        .filter(|segment| !segment.is_empty())
                        .map(path_codec::decode_path_segment_input)
                        .collect::<anyhow::Result<Vec<_>>>()?
                };
                opt.initial_path = path;
                initial_path_set = true;
            }
        }
    }

    Ok(Some(opt))
}

fn resolve_config_path(path: &PathBuf) -> PathBuf {
    if path.extension().and_then(|ext| ext.to_str()) == Some("lua") {
        path.clone()
    } else {
        path.join("init.lua")
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    log::Logs::start()?;
    errors::install_hooks();
    let Some(cli) = parse_cli_options(std::env::args_os())? else {
        return Ok(());
    };

    if let Some(config_path) = cli.config_path.as_ref() {
        let config_file = resolve_config_path(config_path);
        std::env::set_var("LAZYDECK_CONFIG_FILE", &config_file);
        if let Some(dir) = config_file.parent() {
            std::env::set_var("LAZYDECK_CONFIG_BASE_DIR", dir);
        }
    }

    let local = task::LocalSet::new();

    // Run the local task set.
    local
        .run_until(async move {
            // Initialize terminal first (required for crossterm event stream)
            let term = term::init()?;

            let events = events::Events::new();

            let mut app = App::new(events.sender(), term, cli.initial_path);

            if let Err(e) = app.run(events).await {
                term::restore();
                eprintln!("Error: {}", e);
                return Err(e);
            }

            term::restore();
            Ok::<_, anyhow::Error>(())
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_cli_options;
    use std::ffi::OsString;

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn parse_cli_options_defaults_to_root() {
        let opt = parse_cli_options(os_args(&["lazydeck"])).unwrap().unwrap();
        assert_eq!(opt.initial_path, Vec::<String>::new());
        assert!(opt.config_path.is_none());
    }

    #[test]
    fn parse_cli_options_parses_initial_path() {
        let opt = parse_cli_options(os_args(&["lazydeck", "/docker/container"]))
            .unwrap()
            .unwrap();
        assert_eq!(opt.initial_path, vec!["docker".to_string(), "container".to_string()]);
    }

    #[test]
    fn parse_cli_options_parses_config_flag() {
        let opt = parse_cli_options(os_args(&["lazydeck", "-c", "/tmp/demo/init.lua"]))
            .unwrap()
            .unwrap();
        assert_eq!(opt.config_path.as_deref(), Some(std::path::Path::new("/tmp/demo/init.lua")));
    }

    #[test]
    fn parse_cli_options_parses_config_equals_form() {
        let opt = parse_cli_options(os_args(&["lazydeck", "--config=/tmp/demo"]))
            .unwrap()
            .unwrap();
        assert_eq!(opt.config_path.as_deref(), Some(std::path::Path::new("/tmp/demo")));
    }

    #[test]
    fn parse_cli_options_rejects_missing_config_path() {
        assert!(parse_cli_options(os_args(&["lazydeck", "-c"])).is_err());
        assert!(parse_cli_options(os_args(&["lazydeck", "--config"])).is_err());
    }

    #[test]
    fn parse_cli_options_rejects_extra_args() {
        assert!(parse_cli_options(os_args(&["lazydeck", "/docker", "/extra"])).is_err());
    }

    #[test]
    fn parse_cli_options_supports_help_flag() {
        assert!(parse_cli_options(os_args(&["lazydeck", "--help"])).unwrap().is_none());
        assert!(parse_cli_options(os_args(&["lazydeck", "-h"])).unwrap().is_none());
    }

    #[test]
    fn parse_cli_options_supports_version_flag() {
        assert!(parse_cli_options(os_args(&["lazydeck", "--version"])).unwrap().is_none());
        assert!(parse_cli_options(os_args(&["lazydeck", "-V"])).unwrap().is_none());
    }
}
