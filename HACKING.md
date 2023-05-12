
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

In addition to the standard rust toolchain, shpool uses the
`cargo-vendor-filterer` tool, so you should install it with
`cargo install cargo-vendor-filterer`.

### Build `shpool`

To build and install `shpool` run

```
$ cargo build --release
$ cp target/release/shpool ~/.cargo/bin/shpool
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

## Adding or updating a dependency

Since shpool uses `cargo-vendor-filterer`, adding or updating a dependency requires
an extra step. After you edit `Cargo.toml` to reflect the change you want
to make as normal. If you have not already installed `cargo-vendor-filterer`
you can do so with `cargo install cargo-vendor-filterer`. Then run the command

```
./vendor.sh
```

## Formatting

Run `cargo +nightly fmt` to ensure that the code matches the expected
style.

## Measuring Latency

To check e2e latency, you can use the
[sshping](https://github.com/spook/sshping) tool to compare latency
between a raw ssh connection and one using shpool. First, get the
baseline measurement by running

```
sshping -H $REMOTE_HOST
```

on your local machine. Now get a comparison by shelling into your
remote host and starting a shpool session called `sshping` with
`shpool attach sshping`. In this session, run `cat > /dev/null`
to set up a tty that will just echo back chars. Now on your local
machine, run

```
sshping -H -e '/path/to/shpool attach -f sshping' $REMOTE_HOST
```

to collect latency measurements with shpool in the loop.

Some latency measurements I collected this way are:

```
$ sshping -H $REMOTE_HOST
ssh-Login-Time:               3.99  s
Minimum-Latency:              24.2 ms
Median-Latency:               26.6 ms
Average-Latency:              27.1 ms
Average-Deviation:            7.85 ms
Maximum-Latency:               180 ms
Echo-Count:                  1.00 kB
Upload-Size:                 8.00 MB
Upload-Rate:                 5.41 MB/s
Download-Size:               8.00 MB
Download-Rate:               7.06 MB/s
$ sshping -H -e '/path/to/shpool attach -f sshping' $REMOTE_HOST
ssh-Login-Time:               5.17  s
Minimum-Latency:              24.4 ms
Median-Latency:               25.7 ms
Average-Latency:              25.9 ms
Average-Deviation:            1.19 ms
Maximum-Latency:              50.8 ms
Echo-Count:                  1.00 kB
Upload-Size:                 8.00 MB
Upload-Rate:                 5.48 MB/s
Download-Size:               8.00 MB
Download-Rate:               7.31 MB/s
```

pretty good.

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
