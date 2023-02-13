
# Hacking

Some tips for working on shpool.

## Installing From Source

### Clone the repo

If you have not worked with git-on-borg before, install
a required helper tool with

```
$ sudo apt-get install git-remote-google
```

now you can clone the actual repo with

```
$ git clone rpc://team/cloudtop-connectivity-eng-team/shpool
```

If you plan to work on `shpool`, install the gerrit Change-Id
hook with

```
$ (cd shpool && f=`git rev-parse --git-dir`/hooks/commit-msg ; mkdir -p $(dirname $f) ; curl -Lo $f https://gerrit-review.googlesource.com/tools/hooks/commit-msg ; chmod +x $f)
```

if you just want to install and use `shpool` there is no need to
install this hook.

### Install a rust toolchain

If you have not already done so, install a rust toolchain.
The minimum rust version for shpool is `1.63.0`, so make sure that
`cargo --version` reports that version or higher before attempting
to build shpool. The easiest way to install an up to date
rust toolchain is with [`rustup`](https://rustup.rs/),
a nice tool maintained by the rust project that allows
you to easily use different toolchain versions.

Make sure that `~/.cargo/bin` is on you `PATH` so you can use
binaries installed with cargo. An entry like

```
$ source "$HOME/.cargo/env"
```

in your `.profile` file should do the trick.

### Install systemd dependencies

```
$ sudo apt install libsystemd-dev
```

### Build `shpool`

To build and install `shpool` run

```
$ cargo install --path .
```

### Install the systemd user service unit file

A convenient way to run the shpool daemon is to use systemd
to start and run it as a user-level systemd service. You
can use the `systemd/shpool.{service,socket}` files
to do this. Install it by running

```
$ mkdir -p ~/.config/systemd/user
$ cp systemd/* ~/.config/systemd/user
```

enable and start it up with

```
$ systemctl --user enable shpool
$ systemctl --user start shpool
```

## Formatting

## Preserving Logs in Tests

By default, tests will clean up log files emitted by the various
shpool subprocesses they spawn. In order get the tests to leave
log files around for later inspection, you can set the
`SHPOOL_LEAVE_TEST_LOGS` environment variable to `true`.

For example to run `happy_path` from the `attach` suite and
leave log files in place you might run

```
$ SHPOOL_LEAVE_TEST_LOGS=true cargo test --test attach happy_path -- --nocapture
```


