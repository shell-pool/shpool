// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use anyhow::{anyhow, Context};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
pub use hooks::Hooks;
use tracing::error;
use tracing_subscriber::{fmt::format::FmtSpan, prelude::*};

mod attach;
mod common;
mod config;
mod config_watcher;
mod consts;
mod daemon;
mod daemonize;
mod detach;
mod duration;
mod hooks;
mod kill;
mod list;
mod protocol;
mod session_restore;
mod set_log_level;
mod test_hooks;
mod tty;
mod user;

/// The command line arguments that shpool expects.
/// These can be directly parsed with clap or manually
/// constructed in order to present some other user
/// interface.
///
/// NOTE: You must check `version()` and handle it yourself
/// if it is set. Clap won't do a good job with its
/// automatic version support for a library.
#[derive(Parser, Debug, Default)]
#[clap(author, about, name = "shpool")]
pub struct Args {
    #[clap(
        short,
        long,
        action,
        long_help = "The file to write logs to

In most modes logs are discarded by default, but if shpool is
running in daemon mode, the logs will go to stderr by default."
    )]
    pub log_file: Option<String>,

    #[clap(
        short,
        long,
        action = clap::ArgAction::Count,
        help = "Show more in logs, may be provided multiple times",
    )]
    pub verbose: u8,

    #[clap(
        short,
        long,
        action,
        long_help = "The path for the unix socket to listen on

This defaults to $XDG_RUNTIME_DIR/shpool/shpool.socket or ~/.local/run/shpool/shpool.socket
if XDG_RUNTIME_DIR is unset.

This flag gets overridden by systemd socket activation when
the daemon is launched by systemd."
    )]
    pub socket: Option<String>,

    #[clap(short, long, action, help = "a toml file containing configuration")]
    pub config_file: Option<String>,

    #[clap(short, long, action, help = "automatically launch a daemon if one is not running")]
    pub daemonize: bool,

    #[clap(short = 'D', long, action, help = "do not automatically launch a daemon")]
    pub no_daemonize: bool,

    #[clap(subcommand)]
    pub command: Commands,

    // A hidden field rather than using the #[non_exhaustive] attribute
    // allows users to build this struct using the default value plus
    // modifications, while the #[non_exhaustive] attribute would not.
    // See https://rust-lang.github.io/rfcs/2008-non-exhaustive.html#functional-record-updates
    // (the attribute behaves as if there is implicitly a field like this
    // that is private).
    #[clap(skip)]
    #[doc(hidden)]
    pub __non_exhaustive: (),
}

/// The subcommds that shpool supports.
#[derive(Subcommand, Debug, Default)]
#[non_exhaustive]
pub enum Commands {
    #[clap(about = "Print version")]
    #[default]
    Version,

    #[clap(about = "Starts running a daemon that holds a pool of shells")]
    Daemon,

    #[clap(about = "Creates or attaches to an existing shell session")]
    #[non_exhaustive]
    Attach {
        #[clap(short, long, help = "If a tty is already attached to the session, detach it first")]
        force: bool,
        #[clap(
            short = 'b',
            long,
            help = "Create/attach the session and immediately detach (use with --force to detach any existing client first)"
        )]
        background: bool,
        #[clap(
            long,
            long_help = "Automatically kill the session after the given time

This option only applies when first creating a session, it is ignored on
reattach.

The duration can be specified either in a colon seperated format
of the form dd:hh:mm:ss where any prefix may be left off (i.e. '01:00:30:00'
for 1 day and 30 minutes or '10:45:00' for 10 hours and 45 minutes), or
using a number with a trailing letter to indicate time unit
(i.e. '3d', '19h', or '5s')."
        )]
        ttl: Option<String>,
        #[clap(
            short,
            long,
            long_help = "A command to run instead of the user's default shell

The command is broken up into a binary to invoke and a list of arguments to
pass to the binary using the shell-words crate."
        )]
        cmd: Option<String>,
        #[clap(
            short,
            long,
            long_help = "The directory to start the shell in.

$HOME by default. Use '.' for pwd."
        )]
        dir: Option<String>,
        #[clap(help = "The name of the shell session to create or attach to")]
        name: String,
    },

    #[clap(about = "Make the given session detach from shpool

This does not close the shell. If no session name is provided
$SHPOOL_SESSION_NAME will be used if it is present in the
environment.")]
    #[non_exhaustive]
    Detach {
        #[clap(help = "sessions to detach")]
        sessions: Vec<String>,
    },

    #[clap(about = "Kill the given sessions

This detaches the session if it is attached and kills the underlying
shell with a SIGHUP followed by a SIGKILL if the shell fails to exit
quickly enough. If no session name is provided $SHPOOL_SESSION_NAME
will be used if it is present in the environment.")]
    #[non_exhaustive]
    Kill {
        #[clap(help = "sessions to kill")]
        sessions: Vec<String>,
    },

    #[clap(about = "lists all the running shell sessions")]
    #[non_exhaustive]
    List {
        #[clap(short, long, help = "Output as JSON, includes extra fields")]
        json: bool,
    },

    #[clap(about = "Generate shell completion script for bash, fish, zsh, or elvish")]
    #[non_exhaustive]
    Completion {
        #[clap(help = "The shell type to generate completions for")]
        shell: CompletionShell,
        #[clap(short, long, help = "Write completions to a file instead of stdout")]
        output: Option<PathBuf>,
    },

    #[clap(about = "Dynamically change daemon log level

This command changes the log level of the shpool daemon without
restarting. It may to useful if the daemon gets into a state that
needs debugging, but would be clobbered by a restart.")]
    #[non_exhaustive]
    SetLogLevel {
        #[clap(help = "new log level")]
        level: shpool_protocol::LogLevel,
    },
}

impl Args {
    /// Version indicates if the wrapping binary must display the
    /// version then exit.
    pub fn version(&self) -> bool {
        matches!(self.command, Commands::Version)
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    Zsh,
}

impl From<CompletionShell> for clap_complete::aot::Shell {
    fn from(value: CompletionShell) -> Self {
        match value {
            CompletionShell::Bash => clap_complete::aot::Shell::Bash,
            CompletionShell::Elvish => clap_complete::aot::Shell::Elvish,
            CompletionShell::Fish => clap_complete::aot::Shell::Fish,
            CompletionShell::Zsh => clap_complete::aot::Shell::Zsh,
        }
    }
}

/// Writes a shell completion script to `writer`.
///
/// Bash and fish are augmented with runtime lookups for live session names.
fn write_completion<W: Write>(
    shell: clap_complete::aot::Shell,
    mut writer: W,
) -> anyhow::Result<()> {
    let mut cmd = Args::command();
    let bin_name = cmd.get_name().to_owned();
    let mut generated = Vec::new();
    clap_complete::generate(shell, &mut cmd, &bin_name, &mut generated);

    match shell {
        clap_complete::aot::Shell::Bash => {
            let script = String::from_utf8(generated)
                .context("generated bash completion was not valid UTF-8")?;
            writer
                .write_all(augment_bash_completion(script, &bin_name)?.as_bytes())
                .context("writing bash completion")
        }
        clap_complete::aot::Shell::Fish => {
            let script = String::from_utf8(generated)
                .context("generated fish completion was not valid UTF-8")?;
            writer
                .write_all(augment_fish_completion(script, &bin_name).as_bytes())
                .context("writing fish completion")
        }
        _ => writer.write_all(&generated).context("writing completion"),
    }
}

fn write_completion_output(shell: CompletionShell, output: Option<&Path>) -> anyhow::Result<()> {
    match output {
        Some(file_path) => {
            if let Some(parent_dir) = file_path.parent() {
                if !parent_dir.as_os_str().is_empty() {
                    fs::create_dir_all(parent_dir).with_context(|| {
                        format!("Failed to create directory: {}", parent_dir.display())
                    })?;
                }
            }
            let mut file = fs::File::create(file_path)
                .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
            write_completion(shell.into(), &mut file)
        }
        None => write_completion(shell.into(), io::stdout()),
    }
}

/// Wraps clap's generated bash completion so dynamic session names can be
/// queried at completion time via `shpool list`.
fn augment_bash_completion(script: String, bin_name: &str) -> anyhow::Result<String> {
    let generated_fn = format!("_{bin_name}_generated");
    let wrapper_fn = format!("_{bin_name}");
    let wrapper_decl = format!("{wrapper_fn}() {{");
    anyhow::ensure!(
        script.contains(&wrapper_decl),
        "generated bash completion no longer contains expected function declaration: {wrapper_decl}"
    );
    let script = script.replacen(&wrapper_decl, &format!("{generated_fn}() {{"), 1);
    let completion_registration = format!(
        "if [[ \"${{BASH_VERSINFO[0]}}\" -eq 4 && \"${{BASH_VERSINFO[1]}}\" -ge 4 || \"${{BASH_VERSINFO[0]}}\" -gt 4 ]]; then\n    complete -F {wrapper_fn} -o nosort -o bashdefault -o default {bin_name}\nelse\n    complete -F {wrapper_fn} -o bashdefault -o default {bin_name}\nfi"
    );
    let script = script.replacen(&completion_registration, "", 1);

    Ok(format!(
        r#"{script}

_{bin_name}_collect_global_args() {{
    _{bin_name}_global_args=()
    local word
    local index=1

    while [[ $index -lt ${{#COMP_WORDS[@]}} ]]; do
        word="${{COMP_WORDS[$index]}}"
        case "$word" in
            # Keep in sync with the Commands enum in libshpool/src/lib.rs.
            version|daemon|attach|detach|kill|list|completion|set-log-level|help)
                break
                ;;
            -l|--log-file|-s|--socket|-c|--config-file)
                _{bin_name}_global_args+=("$word")
                ((index++))
                if [[ $index -lt ${{#COMP_WORDS[@]}} ]]; then
                    _{bin_name}_global_args+=("${{COMP_WORDS[$index]}}")
                fi
                ;;
            -v|--verbose|-d|--daemonize|-D|--no-daemonize)
                _{bin_name}_global_args+=("$word")
                ;;
        esac
        ((index++))
    done
}}

_{bin_name}_session_words() {{
    _{bin_name}_collect_global_args
    command {bin_name} "${{_{bin_name}_global_args[@]}}" list 2>/dev/null | awk 'NR > 1 {{ print $1 }}'
}}

_{bin_name}_current_subcommand() {{
    local word
    local index=1

    while [[ $index -lt ${{#COMP_WORDS[@]}} ]]; do
        word="${{COMP_WORDS[$index]}}"
        case "$word" in
            # Keep in sync with the Commands enum in libshpool/src/lib.rs.
            version|daemon|attach|detach|kill|list|completion|set-log-level|help)
                printf '%s\n' "$word"
                return 0
                ;;
            -l|--log-file|-s|--socket|-c|--config-file)
                ((index++))
                ;;
        esac
        ((index++))
    done

    return 1
}}

{wrapper_fn}() {{
    local cur prev cmd

    COMPREPLY=()
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    prev=""
    if [[ $COMP_CWORD -gt 0 ]]; then
        prev="${{COMP_WORDS[COMP_CWORD-1]}}"
    fi
    cmd="$(_{bin_name}_current_subcommand)"

    case "$cmd" in
        attach)
            case "$prev" in
                --ttl|--cmd|-c|--dir|-d)
                    {generated_fn} "$@"
                    return 0
                    ;;
            esac
            if [[ $cur != -* ]]; then
                COMPREPLY=( $(compgen -W "$(_{bin_name}_session_words)" -- "$cur") )
                return 0
            fi
            ;;
        detach|kill)
            if [[ $cur != -* ]]; then
                COMPREPLY=( $(compgen -W "$(_{bin_name}_session_words)" -- "$cur") )
                return 0
            fi
            ;;
    esac

    {generated_fn} "$@"
}}

if [[ "${{BASH_VERSINFO[0]}}" -eq 4 && "${{BASH_VERSINFO[1]}}" -ge 4 || "${{BASH_VERSINFO[0]}}" -gt 4 ]]; then
    complete -F _{bin_name} -o nosort -o bashdefault -o default {bin_name}
else
    complete -F _{bin_name} -o bashdefault -o default {bin_name}
fi
"#
    ))
}

/// Augments clap's generated fish completion with runtime session name lookups.
fn augment_fish_completion(script: String, bin_name: &str) -> String {
    format!(
        r#"{script}

function __fish_{bin_name}_completion_global_args
    set -l words (commandline -opc)
    set -e words[1]
    set -l globals
    set -l expects_value 0

    for word in $words
        if test $expects_value -eq 1
            set -a globals $word
            set expects_value 0
            continue
        end

        switch $word
            # Keep in sync with the Commands enum in libshpool/src/lib.rs.
            case version daemon attach detach kill list completion set-log-level help
                break
            case -l --log-file -s --socket -c --config-file
                set -a globals $word
                set expects_value 1
            case -v --verbose -d --daemonize -D --no-daemonize
                set -a globals $word
        end
    end

    printf '%s\n' $globals
end

function __fish_{bin_name}_sessions
    set -l globals (__fish_{bin_name}_completion_global_args)
    command {bin_name} $globals list 2>/dev/null | awk 'NR > 1 {{ print $1 }}'
end

function __fish_{bin_name}_needs_attach_session
    set -l words (commandline -opc)
    set -e words[1]
    set -l expects_value 0
    set -l seen_attach 0
    set -l positional_count 0

    for word in $words
        if test $expects_value -eq 1
            set expects_value 0
            continue
        end

        if test $seen_attach -eq 0
            switch $word
                case attach
                    set seen_attach 1
                case -l --log-file -s --socket -c --config-file
                    set expects_value 1
            end
            continue
        end

        switch $word
            case --ttl --cmd -c --dir -d
                set expects_value 1
            case -f --force -b --background -h --help
            case '-*'
            case '*'
                set positional_count (math $positional_count + 1)
        end
    end

    test $seen_attach -eq 1; and test $positional_count -eq 0
end

function __fish_{bin_name}_needs_log_level
    set -l words (commandline -opc)
    set -e words[1]
    set -l expects_value 0
    set -l seen_command 0
    set -l value_count 0

    for word in $words
        if test $expects_value -eq 1
            set expects_value 0
            continue
        end

        if test $seen_command -eq 0
            switch $word
                case set-log-level
                    set seen_command 1
                case -l --log-file -s --socket -c --config-file
                    set expects_value 1
            end
            continue
        end

        switch $word
            case -h --help
            case '-*'
            case '*'
                set value_count (math $value_count + 1)
        end
    end

    test $seen_command -eq 1; and test $value_count -eq 0
end

complete -c {bin_name} -n '__fish_{bin_name}_needs_attach_session' -f -a '(__fish_{bin_name}_sessions)'
complete -c {bin_name} -n '__fish_seen_subcommand_from detach kill' -f -a '(__fish_{bin_name}_sessions)'
complete -c {bin_name} -n '__fish_{bin_name}_needs_log_level' -f -a 'off error warn info debug trace'
"#
    )
}

#[cfg(test)]
mod completion_tests {
    use clap::Parser;

    use super::{write_completion, Args};

    #[test]
    fn bash_completion_includes_dynamic_session_lookup() {
        let mut output = Vec::new();
        write_completion(clap_complete::aot::Shell::Bash, &mut output)
            .expect("bash completion generation should succeed");
        let script = String::from_utf8(output).expect("bash completion should be utf-8");

        assert!(script.contains("_shpool_generated() {"));
        assert!(script.contains("_shpool_session_words() {"));
        assert!(script.contains(
            "command shpool \"${_shpool_global_args[@]}\" list 2>/dev/null | awk 'NR > 1 { print $1 }'"
        ));
        assert!(!script.contains("[NON_EXHAUSTIVE]"));
        assert!(script.contains("complete -F _shpool -o bashdefault -o default shpool"));
        assert_eq!(script.matches("complete -F _shpool").count(), 2);
    }

    #[test]
    fn fish_completion_includes_dynamic_session_lookup() {
        let mut output = Vec::new();
        write_completion(clap_complete::aot::Shell::Fish, &mut output)
            .expect("fish completion generation should succeed");
        let script = String::from_utf8(output).expect("fish completion should be utf-8");

        assert!(script.contains("function __fish_shpool_sessions"));
        assert!(script.contains("function __fish_shpool_needs_attach_session"));
        assert!(
            script.contains("command shpool $globals list 2>/dev/null | awk 'NR > 1 { print $1 }'")
        );
        assert!(!script.contains("[NON_EXHAUSTIVE]"));
        assert!(script.contains(
            "complete -c shpool -n '__fish_seen_subcommand_from detach kill' -f -a '(__fish_shpool_sessions)'"
        ));
        assert!(script.contains(
            "complete -c shpool -n '__fish_shpool_needs_log_level' -f -a 'off error warn info debug trace'"
        ));
    }

    #[test]
    fn completion_rejects_powershell_shell() {
        let err = Args::try_parse_from(["shpool", "completion", "powershell"])
            .expect_err("powershell should not be accepted as a completion target");
        let err = err.to_string();

        assert!(err.contains("invalid value 'powershell'"));
        assert!(err.contains("[possible values: bash, elvish, fish, zsh]"));
    }
}

// Copied from the tracing-subscriber crate. This is public in
// a future version of the crate, but for now we don't have
// access to it. If tracing-subscriber is 0.3.19 or better,
// it is worth checking to see if we can rip this out.
#[derive(Debug)]
pub struct MutexGuardWriter<'a, W>(MutexGuard<'a, W>);
impl<W> io::Write for MutexGuardWriter<'_, W>
where
    W: io::Write,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.write_all(buf)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> io::Result<()> {
        self.0.write_fmt(fmt)
    }
}

struct LogWriterBuilder {
    log_file: Option<Mutex<fs::File>>,
    is_daemon: bool,
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for LogWriterBuilder {
    type Writer = Box<dyn io::Write + 'writer>;

    fn make_writer(&'writer self) -> Self::Writer {
        if let Some(log_file) = &self.log_file {
            Box::new(MutexGuardWriter(log_file.lock().expect("poisoned")))
        } else if self.is_daemon {
            Box::new(io::stderr())
        } else {
            Box::new(io::empty())
        }
    }
}

/// Run the shpool tool with the given arguments. If hooks is provided,
/// inject the callbacks into the daemon.
pub fn run(args: Args, hooks: Option<Box<dyn hooks::Hooks + Send + Sync>>) -> anyhow::Result<()> {
    match (&args.command, env::var(consts::SENTINEL_FLAG_VAR).as_deref()) {
        (Commands::Daemon, Ok("prompt")) => {
            println!("{}", consts::PROMPT_SENTINEL);
            std::process::exit(0);
        }
        (Commands::Daemon, Ok("startup")) => {
            println!("{}", consts::STARTUP_SENTINEL);
            std::process::exit(0);
        }
        _ => {}
    }

    if let Commands::Completion { shell, output } = &args.command {
        write_completion_output(*shell, output.as_deref())?;
        return Ok(());
    }

    let log_level_filter = if args.verbose == 0 {
        tracing_subscriber::filter::LevelFilter::INFO
    } else if args.verbose == 1 {
        tracing_subscriber::filter::LevelFilter::DEBUG
    } else {
        tracing_subscriber::filter::LevelFilter::TRACE
    };
    let (log_level_layer, log_level_handle) =
        tracing_subscriber::reload::Layer::new(log_level_filter);

    let log_writer_builder = LogWriterBuilder {
        log_file: if let Some(lf) = &args.log_file {
            Some(Mutex::new(fs::File::create(lf).context("unable to create log file")?))
        } else {
            None
        },
        is_daemon: matches!(args.command, Commands::Daemon),
    };
    tracing_subscriber::registry::Registry::default()
        .with(log_level_layer)
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_thread_ids(true)
                .with_target(false)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_writer(log_writer_builder),
        )
        .init();

    let mut runtime_dir = match env::var("XDG_RUNTIME_DIR") {
        Ok(runtime_dir) => PathBuf::from(runtime_dir),
        Err(_) => PathBuf::from(env::var("HOME").context("no XDG_RUNTIME_DIR or HOME")?)
            .join(".local")
            .join("run"),
    }
    .join("shpool");
    fs::create_dir_all(&runtime_dir).context("ensuring runtime dir exists")?;

    let socket = match &args.socket {
        Some(s) => {
            // The user can reasonably expect that if they provide seperate
            // sockets for differnt shpool instances to run on, they won't
            // stomp on one another. To respect this expectation we need to
            // namespace the rest of the runtime data if they provide a socket
            // name. A short hash is probably good enough.
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            let hash = hasher.finish();
            runtime_dir = runtime_dir.join(format!("{hash:x}"));

            PathBuf::from(s)
        }
        None => runtime_dir.join("shpool.socket"),
    };

    let config_manager = config::Manager::new(args.config_file.as_deref())?;

    if !config_manager.get().nodaemonize.unwrap_or(false) || args.daemonize {
        let arg0 = env::args().next().ok_or(anyhow!("arg0 missing"))?;
        if !args.no_daemonize
            && !matches!(
                args.command,
                Commands::Version | Commands::Daemon | Commands::Completion { .. }
            )
        {
            daemonize::maybe_fork_daemon(&config_manager, &args, arg0, &socket)?;
        }
    }

    #[cfg(feature = "test_hooks")]
    if let Ok(test_hook_sock) = std::env::var("SHPOOL_TEST_HOOK_SOCKET_PATH") {
        log::info!("spawning test hook sock at {}", test_hook_sock);
        test_hooks::TEST_HOOK_SERVER.set_socket_path(test_hook_sock.clone());
        std::thread::spawn(|| {
            test_hooks::TEST_HOOK_SERVER.start();
        });
        log::info!("waiting for test hook connection");
        test_hooks::TEST_HOOK_SERVER.wait_for_connect()?;
    }

    let res: anyhow::Result<()> = match args.command {
        Commands::Version => return Err(anyhow!("wrapper binary must handle version")),
        Commands::Daemon => daemon::run(
            config_manager,
            runtime_dir,
            hooks.unwrap_or(Box::new(NoopHooks {})),
            log_level_handle,
            socket,
        ),
        Commands::Attach { force, background, ttl, cmd, dir, name } => {
            attach::run(config_manager, name, force, background, ttl, cmd, dir, socket)
        }
        Commands::Detach { sessions } => detach::run(sessions, socket),
        Commands::Kill { sessions } => kill::run(sessions, socket),
        Commands::List { json } => list::run(socket, json),
        Commands::Completion { .. } => unreachable!("completion handled before runtime setup"),
        Commands::SetLogLevel { level } => set_log_level::run(level, socket),
    };

    if let Err(err) = res {
        error!("{:?}", err);
        std::process::exit(1);
    }

    Ok(())
}

struct NoopHooks {}
impl hooks::Hooks for NoopHooks {}
