use cargo_deb::*;
use cargo_deb::control::ControlArchiveBuilder;
use std::env;
use std::path::Path;
use std::process;

struct CliOptions {
    no_build: bool,
    strip_override: Option<bool>,
    separate_debug_symbols: bool,
    fast: bool,
    verbose: bool,
    quiet: bool,
    install: bool,
    selected_package_name: Option<String>,
    output_path: Option<String>,
    variant: Option<String>,
    target: Option<String>,
    manifest_path: Option<String>,
    cargo_build_cmd: String,
    cargo_build_flags: Vec<String>,
    deb_version: Option<String>,
    deb_revision: Option<String>,
    system_xz: bool,
    profile: Option<String>,
}

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    let mut cli_opts = getopts::Options::new();
    cli_opts.optflag("", "no-build", "Assume project is already built");
    cli_opts.optflag("", "no-strip", "Do not strip debug symbols from the binary");
    cli_opts.optflag("", "strip", "Always try to strip debug symbols");
    cli_opts.optflag("", "separate-debug-symbols", "Strip debug symbols into a separate .debug file");
    cli_opts.optflag("", "fast", "Use faster compression, which yields larger archive");
    cli_opts.optflag("", "install", "Immediately install created package");
    cli_opts.optopt("", "target", "Rust target for cross-compilation", "triple");
    cli_opts.optopt("", "variant", "Alternative configuration section to use", "name");
    cli_opts.optopt("", "manifest-path", "Cargo project file location", "./Cargo.toml");
    cli_opts.optopt("p", "package", "Select one of packages belonging to a workspace", "name");
    cli_opts.optopt("o", "output", "Write .deb to this file or directory", "path");
    cli_opts.optflag("q", "quiet", "Don't print warnings");
    cli_opts.optflag("v", "verbose", "Print progress");
    cli_opts.optflag("h", "help", "Print this help menu");
    cli_opts.optflag("", "version", "Show the version of cargo-deb");
    cli_opts.optopt("", "deb-version", "Alternate version string for package", "version");
    cli_opts.optopt("", "deb-revision", "Alternate revision string for package", "revision");
    cli_opts.optflag("", "system-xz", "Compress using command-line xz command instead of built-in");
    cli_opts.optopt("", "profile", "select which project profile to package", "profile");
    cli_opts.optopt("", "cargo-build", "Override cargo build subcommand", "subcommand");

    let matches = match cli_opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(err) => {
            err_exit(&err);
        }
    };
    if matches.opt_present("h") {
        print!("{}", cli_opts.usage("Usage: cargo deb [options] [-- <cargo build flags>]"));
        return;
    }

    if matches.opt_present("version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let install = matches.opt_present("install");
    match process(CliOptions {
        no_build: matches.opt_present("no-build"),
        strip_override: if matches.opt_present("strip") { Some(true) } else if matches.opt_present("no-strip") { Some(false) } else { None },
        separate_debug_symbols: matches.opt_present("separate-debug-symbols"),
        quiet: matches.opt_present("quiet"),
        verbose: matches.opt_present("verbose"),
        install,
        // when installing locally it won't be transferred anywhere, so allow faster compression
        fast: install || matches.opt_present("fast"),
        variant: matches.opt_str("variant"),
        target: matches.opt_str("target"),
        output_path: matches.opt_str("output"),
        selected_package_name: matches.opt_str("package"),
        manifest_path: matches.opt_str("manifest-path"),
        deb_version: matches.opt_str("deb-version"),
        deb_revision: matches.opt_str("deb-revision"),
        system_xz: matches.opt_present("system-xz"),
        profile: matches.opt_str("profile"),
        cargo_build_cmd: matches.opt_str("cargo-build").unwrap_or("build".to_string()),
        cargo_build_flags: matches.free,
    }) {
        Ok(()) => {},
        Err(err) => {
            err_exit(&err);
        }
    }
}

#[allow(deprecated)]
fn err_cause(err: &dyn std::error::Error, max: usize) {
    if let Some(reason) = err.cause() { // we use cause(), not source()
        eprintln!("  because: {reason}");
        if max > 0 {
            err_cause(reason, max - 1);
        }
    }
}

fn err_exit(err: &dyn std::error::Error) -> ! {
    eprintln!("cargo-deb: {err}");
    err_cause(err, 3);
    process::exit(1);
}

fn process(
    CliOptions {
        manifest_path,
        output_path,
        selected_package_name,
        variant,
        target,
        install,
        no_build,
        strip_override,
        separate_debug_symbols,
        quiet,
        fast,
        verbose,
        cargo_build_cmd,
        mut cargo_build_flags,
        deb_version,
        deb_revision,
        system_xz,
        profile,
    }: CliOptions,
) -> CDResult<()> {
    let target = target.as_deref();
    let variant = variant.as_deref();

    if install || target.is_none() {
        warn_if_not_linux(); // compiling natively for non-linux = nope
    }

    // `cargo deb` invocation passes the `deb` arg through.
    if cargo_build_flags.first().map_or(false, |arg| arg == "deb") {
        cargo_build_flags.remove(0);
    }

    // Listener conditionally prints warnings
    let listener_tmp1;
    let listener_tmp2;
    let listener: &dyn listener::Listener = if quiet {
        listener_tmp1 = listener::NoOpListener;
        &listener_tmp1
    } else {
        listener_tmp2 = listener::StdErrListener { verbose };
        &listener_tmp2
    };

    // The profile is selected based on the given ClI options and then passed to
    // cargo build accordingly. you could argue that the other way around is
    // more desirable. However for now we want all commands coming in via the
    // same `interface`
    let selected_profile = profile.as_deref().unwrap_or("release");
    if selected_profile == "dev" {
        listener.warning("dev profile is not supported and will be a hard error in the future. \
            cargo-deb is for making releases, and it doesn't make sense to use it with dev profiles.".into());
        listener.warning("To enable debug symbols set `[profile.release] debug = true` instead.".into());
    }
    cargo_build_flags.push(format!("--profile={selected_profile}"));

    let manifest_path = manifest_path.as_ref().map_or("Cargo.toml", |s| s.as_str());
    let mut options = Config::from_manifest(
        Path::new(manifest_path),
        selected_package_name.as_deref(),
        output_path,
        target,
        variant,
        deb_version,
        deb_revision,
        listener,
        selected_profile,
    )?;
    reset_deb_temp_directory(&options)?;

    options.extend_cargo_build_flags(&mut cargo_build_flags);

    if !no_build {
        cargo_build(&options, target, &cargo_build_cmd, &cargo_build_flags, verbose)?;
    }

    options.resolve_assets()?;

    crate::data::compress_assets(&mut options, listener)?;

    if strip_override.unwrap_or(separate_debug_symbols || !options.debug_enabled) {
        strip_binaries(&mut options, target, listener, separate_debug_symbols)?;
    } else {
        log::debug!("not stripping profile.release.debug={} strip-flag={:?}", options.debug_enabled, strip_override);
    }

    // Obtain the current time which will be used to stamp the generated files in the archives.
    let default_timestamp = options.default_timestamp;

    let options = &options;
    let (control_builder, data_result) = rayon::join(
        move || {
            // The control archive is the metadata for the package manager
            let mut control_builder = ControlArchiveBuilder::new(compress::xz_or_gz(fast, system_xz)?, default_timestamp, listener);
            control_builder.generate_archive(options)?;
            Ok::<_, CargoDebError>(control_builder)
        },
        move || {
            // Initialize the contents of the data archive (files that go into the filesystem).
            let (compressed, asset_hashes) = data::generate_archive(compress::xz_or_gz(fast, system_xz)?, &options, default_timestamp, listener)?;
            let original_data_size = compressed.uncompressed_size;
            Ok::<_, CargoDebError>((compressed.finish()?, original_data_size, asset_hashes))
        },
    );
    let mut control_builder = control_builder?;
    let (data_compressed, original_data_size, asset_hashes) = data_result?;
    control_builder.generate_md5sums(options, asset_hashes)?;
    let control_compressed = control_builder.finish()?.finish()?;

    let mut deb_contents = DebArchive::new(&options)?;
    deb_contents.add_data("debian-binary".into(), default_timestamp, b"2.0\n")?;

    // Order is important for Debian
    deb_contents.add_data(format!("control.tar.{}", control_compressed.extension()), default_timestamp, &control_compressed)?;
    drop(control_compressed);
    let compressed_data_size = data_compressed.len();
    listener.info(format!(
        "compressed/original ratio {compressed_data_size}/{original_data_size} ({}%)",
        compressed_data_size * 100 / original_data_size
    ));
    deb_contents.add_data(format!("data.tar.{}", data_compressed.extension()), default_timestamp, &data_compressed)?;
    drop(data_compressed);

    let generated = deb_contents.finish()?;
    if !quiet {
        println!("{}", generated.display());
    }

    remove_deb_temp_directory(options);

    if install {
        install_deb(&generated)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn warn_if_not_linux() {}

#[cfg(not(target_os = "linux"))]
fn warn_if_not_linux() {
    eprintln!("warning: You're creating a package for your current operating system only, and not for Linux. Use --target if you want to cross-compile.");
}
