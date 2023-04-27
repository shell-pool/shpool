### `[package.metadata.deb.systemd-units]` options

When this table is present in `Cargo.toml` AND `maintainer-scripts` is also specified, correct installation of systemd units will be handled automatically for you.

This works as follows:
1. Assets will be added for any matching systemd unit files found in the `unit-scripts` _(see below)_ directory.
2. Shell script fragments will be generated for enabling, disabling, starting, stopping, and restarting the corresponding systemd services, when the package is installed, updated, or removed.
3. `maintainer-scripts` (`prerm`, `postrm`, `preinst` and/or `postinst`) will be augmented (by replacing the special token `#DEBHELPER#`), or created if missing, using the generated shell script fragments.

**Note:** `<maintainer-scripts>` **MUST** be set, even if it is an empty directory. If non-empty, any maintainer scripts present **MUST** contain the `#DEBHELPER#` token denoting the point at which generated shell script fragments should be inserted.

The exact behaviour can be tuned using the following options:

 - **unit-scripts**: Directory containing zero or more [systemd unit files](https://www.freedesktop.org/software/systemd/man/systemd.unit.html) (see below for matching rules) (defaults to the value of the `maintainer-scripts` option).
 - **unit-name**: Only include systemd unit files for this unit (see below for matching rules).
 - **enable**: Enable the systemd unit on package installation and disable it on package removal (default `true`).
 - **start**: Start the systemd unit on package installation and stop it on package removal (default `true`).
 - **restart-after-upgrade**: If true, postpone systemd service restart until after upgrade is complete (+ = less downtime, - = can confuse some programs), otherwise stop the service before upgrade and start it again after upgrade (default `true`).
 - **stop-on-upgrade**: If true stop the systemd on package upgrade and removal, otherwise stop the sytemsd service only on package removal (default `true`).

#### System unit file naming

Systemd unit file names must match one of the following patterns:

 - `<package>.<unit>.<suffix>` - _only if `unit-name` is specified_
 - `<package>.<unit>@.<suffix>` - _only if `unit-name` is specified_
 - `<package>.<suffix>`
 - `<package>@.<suffix>`
 - `<unit>.<suffix>` - _only if `unit-name` is specified_
 - `<unit>@.<suffix>` - _only if `unit-name` is specified_

Where `<suffix>` is one of: `mount` (@ not supported), `path`, `service`, `socket`, `target`, `timer`, `tmpfile` (@ not supported)

#### Maintainer script file naming

User supplied `maintainer-scripts` file names must match one of the following patterns:

 - `<package>.<unit>.<script>` - _only if `unit-name` is specified_
 - `<package>.<script>`
 - `<unit>.<script>` - _only if `unit-name` is specified_
 - `<script>`

Where `<script>` is one of: `preinst`, `postinst`, `prerm`, `postrm`.

#### Interaction with the cargo-deb variants feature

**NOTE:** When using the variant feature, `<package>` will actually be `<package>-<variant>` unless the variant name has been overridden using `name` in the variant specific metadata table. You can use this to supply variant specific unit files and maintainer scripts.

#### References

See:
 - The [dh_installsystemd Ubuntu 20.04 man page](http://manpages.ubuntu.com/manpages/focal/en/man1/dh_installsystemd.1.html)
 - The [systemd documentation](https://www.freedesktop.org/software/systemd/man/systemd.unit.html#Description) for more details on unit naming.
 - The [Debian Policy Manual](https://www.debian.org/doc/debian-policy/ch-maintainerscripts.html) for more information about maintainer scripts.
 - A list of [shell fragments](https://github.com/kornelski/cargo-deb/tree/main/autoscripts) which may be inserted.

#### Minimal Example

`Cargo.toml`:

```toml
[package]
name = "example"
version = "1.2.3"
description = "An example package to demonstrate cargo-deb systemd-units support."
license = "MIT"
authors = ["cargo-deb team"]

[package.metadata.deb]
maintainer-scripts = "debian/"
systemd-units = { enable = false }
```

`debian/service`:
```
[Unit]
Description=Example

[Service]
ExecStart=/usr/bin/example

[Install]
WantedBy=multi-user.target
```

`src/main.rs`:
```rust
fn main() {
  println!("Hello World!");
}
```

Invoke cargo-deb with verbose output enabled:

```sh
$ cargo-deb -v
   Compiling example v1.2.3 (/tmp/t)
     Running `rustc --crate-name example src/main.rs --error-format=json --json=diagnostic-rendered-ansi --crate-type bin --emit=dep-info,link -C opt-level=3 -Cembed-bitcode=no -C metadata=25d9e83f3daf475a -C extra-filename=-25d9e83f3daf475a --out-dir /tmp/t/target/release/deps -L dependency=/tmp/t/target/release/deps`
    Finished release [optimized] target(s) in 0.12s
info: Stripped '/tmp/t/target/release/example'
info: /tmp/t/target/release/example -> usr/bin/example
info: - -> usr/share/doc/example/copyright
info: /tmp/t/debian/service -> lib/systemd/system/example.service
info: Determining augmentations needed for systemd unit example.service
info: Maintainer script postinst will be augmented with autoscript postinst-systemd-dont-enable
info: Maintainer script postrm will be augmented with autoscript postrm-systemd
info: Maintainer script postinst will be augmented with autoscript postinst-systemd-restart
info: Maintainer script prerm will be augmented with autoscript prerm-systemd-restart
info: Maintainer script postrm will be augmented with autoscript postrm-systemd-reload-only
info: Generating maintainer script postinst
info: Generating maintainer script prerm
info: Generating maintainer script postrm
info: compressed/original ratio 91596/243712 (37%)
/tmp/t/target/debian/example_1.2.3_amd64.deb
```

Use `dpkg` to inspect the created archives maintainer scripts:

```sh
$ dpkg -e target/debian/example_1.2.3_amd64.deb deb_out
$ ls -la deb_out/
total 28
drwxr-xr-x 2 ximon ximon 4096 aug 19 12:31 .
drwxrwxr-x 6 ximon ximon 4096 aug 19 12:31 ..
-rw-r--r-- 1 ximon ximon  249 aug 19 12:28 control
-rw-r--r-- 1 ximon ximon  185 aug 19 12:28 md5sums
-rwxr-xr-x 1 ximon ximon 1211 aug 19 12:28 postinst
-rwxr-xr-x 1 ximon ximon  599 aug 19 12:28 postrm
-rwxr-xr-x 1 ximon ximon  206 aug 19 12:28 prerm
```

Inspect one of the generated maintainer scripts:

```sh
$ cat deb_out/postinst
#!/bin/sh
set -e
# Automatically added by cargo-deb
if [ "$1" = "configure" ] || [ "$1" = "abort-upgrade" ] || [ "$1" = "abort-deconfigure" ] || [ "$1" = "abort-remove" ] ; then
	if deb-systemd-helper debian-installed example.service; then
		# This will only remove masks created by d-s-h on package removal.
		deb-systemd-helper unmask example.service >/dev/null || true

		if deb-systemd-helper --quiet was-enabled example.service; then
			# Create new symlinks, if any.
			deb-systemd-helper enable example.service >/dev/null || true
		fi
	fi

	# Update the statefile to add new symlinks (if any), which need to be cleaned
	# up on purge. Also remove old symlinks.
	deb-systemd-helper update-state example.service >/dev/null || true
fi
# End automatically added section
# Automatically added by cargo-deb
if [ "$1" = "configure" ] || [ "$1" = "abort-upgrade" ] || [ "$1" = "abort-deconfigure" ] || [ "$1" = "abort-remove" ] ; then
	if [ -d /run/systemd/system ]; then
		systemctl --system daemon-reload >/dev/null || true
		if [ -n "$2" ]; then
			_dh_action=restart
		else
			_dh_action=start
		fi
		deb-systemd-invoke $_dh_action example.service >/dev/null || true
	fi
fi
# End automatically added section
```

Note that two shell script fragments have been injected into the maintainer script and that the `#RESTART_ACTION#` and `#UNITFILE#` placeholder tokens have been replaced compared to the original autoscripts [here](https://github.com/kornelski/cargo-deb/blob/main/autoscripts/postinst-systemd-dont-enable) and [here](https://github.com/kornelski/cargo-deb/blob/main/autoscripts/postinst-systemd-restart).

#### Multiple Systemd Units Example

There is also an option to specify multiple systemd unit files, To expand on the minimal example, here is a minumal example with multiple systemd unit files.

`Cargo.toml`:

```toml
[package]
name = "example"
version = "1.2.3"
description = "An example package to demonstrate cargo-deb systemd-units support."
license = "MIT"
authors = ["cargo-deb team"]

[package.metadata.deb]
maintainer-scripts = "debian/"
systemd-units = [ 
        { unit-name = "unit-one", enable = false },
        { unit-name = "unit-two", enable = false } 
    ] 
```

#### Advanced Example

For a more advanced example you might want to look at the [NLnet Labs Krill project](https://github.com/NLnetLabs/krill/) use of cargo-deb (disclaimer: this author is a contributor) which shows:

- Use of `unit-name` ([here](https://github.com/NLnetLabs/krill/blob/master/Cargo.toml#L102)).
- Use of user provided maintainer scripts (e.g. [here](https://github.com/NLnetLabs/krill/blob/master/debian/postinst)) with included `#DEBHELPER#` token to add maintainer script fragments to existing scripts which create a shell user and create (and remove on purge) a config file.
- Use of operating system specific systemd service unit files via cargo-deb [variants](https://github.com/NLnetLabs/krill/blob/master/Cargo.toml#L111) and symbolic links (e.g. [here](https://github.com/NLnetLabs/krill/blob/master/debian/krill-debian10.krill.service)).
- Use of `--variant` and `--deb-version` command line arguments ([here](https://github.com/NLnetLabs/krill/blob/master/.github/workflows/pkg.yml#L191)).

Additionally, though not strictly related to systemd-units support but still cargo-deb related, it shows:
- Packaging on different operating systems with Docker ([here](https://github.com/NLnetLabs/krill/blob/master/.github/workflows/pkg.yml#L56))
- Use of the Lintian tool to verify the created package ([here](https://github.com/NLnetLabs/krill/blob/master/.github/workflows/pkg.yml#L198))
- Testing package install and upgrade on different operating systems using LXC/LXD containers (for systemd support) ([here](https://github.com/NLnetLabs/krill/blob/master/.github/workflows/pkg.yml#L218))
