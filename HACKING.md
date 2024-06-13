
# Hacking

Some tips for working on shpool.

## Installing From Source

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

## Debugging with `rr`

The `rr` tool allows you to record and replay executions under a debugger,
which allows you to do fun stuff like step backwards. Additionally, when
`rr` records a trace, it records the trace for the whole process tree, so
you can debug events that happen in subprocesses. `rr` only works on Linux,
and requires certain performance counters, so it does not work well in
many virtualized environments.

To record a test under `rr`, build the test binary with

```
$ cargo test --test <test-suite-name> --no-run
```

then run

```
$ SHPOOL_LEAVE_TEST_LOGS=true rr ./path/to/test/exe <test_name> --nocapture
```

to replay, inspecting a subprocess, first run

```
$ rr ps
```

to view all the various processes that got launched, then run

```
$ rr replay --debugger=rust-gdb --onprocess=<PID>
```

where `<PID>` is taken from the output of `rr ps`.

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
