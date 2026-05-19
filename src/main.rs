use std::ffi::OsString;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

fn print_help() {
    println!(
        "{name} {version}\n{description}\n\nUsage:\n  {name} [OPTIONS] [initial-path]\n\nArguments:\n  [initial-path]  Optional initial page path, e.g. /docker/container\n\nOptions:\n  -h, --help      Print help\n  -V, --version   Print version",
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

fn parse_initial_path(
    args: impl IntoIterator<Item = OsString>,
) -> anyhow::Result<Option<Vec<String>>> {
    let mut args = args.into_iter();
    let _program = args.next();

    let mut initial_path = None;

    for raw_arg in args {
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
            _ if arg.starts_with('-') => {
                anyhow::bail!(
                    "Unknown option: {arg}\nTry '{APP_NAME} --help' for more information."
                );
            }
            _ => {
                if initial_path.is_some() {
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
                initial_path = Some(path);
            }
        }
    }

    Ok(Some(initial_path.unwrap_or_default()))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    log::Logs::start()?;
    errors::install_hooks();
    let Some(initial_path) = parse_initial_path(std::env::args_os())? else {
        return Ok(());
    };
    let local = task::LocalSet::new();

    // Run the local task set.
    local
        .run_until(async move {
            // Initialize terminal first (required for crossterm event stream)
            let term = term::init()?;

            let events = events::Events::new();

            let mut app = App::new(events.sender(), term, initial_path);

            if let Err(e) = app.run(events).await {
                term::restore();
                eprintln!("Error: {}", e);
                return Err(e);
            }

            term::restore();
            Ok::<_, anyhow::Error>(())
        })
        .await?;

    // errors::install_hooks()?;
    // state::init();
    // plugin::init()?;
    //
    // let term = term::init()?;
    // let events = events::Events::new();
    // App::new().run(term, events).await?;
    // //
    // term::restore()?;
    // sleep(Duration::from_millis(3000)).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_initial_path;
    use std::ffi::OsString;

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn parse_initial_path_defaults_to_root() {
        assert_eq!(
            parse_initial_path(os_args(&["lazydeck"])).unwrap(),
            Some(Vec::<String>::new())
        );
    }

    #[test]
    fn parse_initial_path_splits_segments() {
        assert_eq!(
            parse_initial_path(os_args(&["lazydeck", "/docker/container"])).unwrap(),
            Some(vec!["docker".to_string(), "container".to_string()])
        );
    }

    #[test]
    fn parse_initial_path_normalizes_repeated_slashes() {
        assert_eq!(
            parse_initial_path(os_args(&["lazydeck", "docker//container/"])).unwrap(),
            Some(vec!["docker".to_string(), "container".to_string()])
        );
    }

    #[test]
    fn parse_initial_path_rejects_extra_args() {
        assert!(parse_initial_path(os_args(&["lazydeck", "/docker", "/extra"])).is_err());
    }

    #[test]
    fn parse_initial_path_decodes_percent_encoded_segments() {
        assert_eq!(
            parse_initial_path(os_args(&[
                "lazydeck",
                "/github/repo/tpope/vim-abolish/tags/feature%2Ftest"
            ]))
            .unwrap(),
            Some(vec![
                "github".to_string(),
                "repo".to_string(),
                "tpope".to_string(),
                "vim-abolish".to_string(),
                "tags".to_string(),
                "feature/test".to_string(),
            ])
        );
    }

    #[test]
    fn parse_initial_path_supports_help_flag() {
        assert_eq!(
            parse_initial_path(os_args(&["lazydeck", "--help"])).unwrap(),
            None
        );
        assert_eq!(
            parse_initial_path(os_args(&["lazydeck", "-h"])).unwrap(),
            None
        );
    }

    #[test]
    fn parse_initial_path_supports_version_flag() {
        assert_eq!(
            parse_initial_path(os_args(&["lazydeck", "--version"])).unwrap(),
            None
        );
        assert_eq!(
            parse_initial_path(os_args(&["lazydeck", "-V"])).unwrap(),
            None
        );
    }

    #[test]
    fn parse_initial_path_rejects_unknown_option() {
        assert!(parse_initial_path(os_args(&["lazydeck", "--wat"])).is_err());
    }
}
