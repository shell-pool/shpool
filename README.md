# shpool

`shpool` is a service that enables session persistence by allowing the
creation of named shell sessions owned by `shpool` so that the session
is not lost if the connection drops. `shpool` can be thought of as a lighter
weight alternative to `tmux` or GNU `screen`. While `tmux` and `screen` take over
the whole terminal and provide window splitting and tiling features, `shpool`
only provides persistent sessions. The biggest advantage of this approach is
that `shpool` does not break native scrollback.

## Project Status

`shpool` has all the usability basics covered, but still has a few
warts. I've been using it as a daily driver for a few weeks now.
The biggest thing I'm having to work around is:
- sometimes I need to manually run `shpool detach` to free up
  named sessions after my connection drops and I ssh back onto
  my cloudtop to reattach.

## Installation & Setup

First, make sure you have an up-to-date rust toolchain from `rustup`.
Make sure you do not already have the out-of-date debian rust toolchain
installed. If you do, run `sudo apt remove rustc` to remove it, then
follow the instructions on https://rustup.rs to install the latest
rustup and rust toolchain. If you already have rustup installed, run
`rustup update stable` to update to the latest stable release.

The easiest way to install shpool is to use the installer script

```
$ /google/data/ro/users/pa/pailes/shpool/install.py --shpool-checkout-dir=/tmp/shpool-install
```

If this script runs into trouble, you might need to fall back on the more
manual install described in [HACKING.md](./HACKING.md).

Once shpool is installed, make sure the user-level systemd unit is
running. You can check its status with

```
systemctl --user status shpool
```

Enable and start it with

```
systemctl --user enable shpool
systemctl --user start shpool
```

## Usage

Generally `shpool` is used to provide persistent sessions when
sshing in to a remote host. To do so, `shpool` must be installed
on the remote host. No extra software is required on the client.
After installing and setting up, the typical usage pattern
is to ssh into the host you have installed shpool on, then create
a new named session by running `shpool attach main`. Here `main`
is the name of the session. You'll want a separate named session
for each terminal you use to connect to your remote host. If your
connection drops or becomes stuck, you can ssh back into the remote
host and re-attach to the same named session by running `shpool attach main`
again.

If your terminal gets stuck and you forcibly close the window, you
might find that shpool still think a terminal is connected to
your session when you attempt to reattach. This is likely because
an ssh proxy is holding the connection open in the vain hope that
it will get some traffic again. You can just run `shpool detach main`
to force the session to detach and allow you to attach.

### Subcommands

#### `shpool daemon`

The `daemon` subcommand causes shpool to run in daemon mode. When running in
this mode, `shpool` listens for incoming connections and opens up subshells,
retaining ownership of them in a table. In general, this subcommand will not
be invoked directly by users, but will instead be called from a systemd unit
file.

#### `shpool attach`

The `attach` subcommand connects to the `shpool daemon` instance, passing in a
name. If the name is new, a new shell is created, and if it already exists it
just attaches to the existing session so long as no other terminal is currently
connected to that session.

#### `shpool list`

Lists all the current shell sessions.

#### `shpool detach`

Detach from a one or more sessions without stopping them.
Will detach the current session if run from inside a shpool
session with no session name arguments.

#### `shpool kill`

Kills a named shell session.

### (Optional) `ssh` Plugin Mode

By adding a few lines to your `.bashrc` on your remote host, you can have ssh
automatically attach to a shpool session derived from the `$SSH_TTY` variable.
`ssh` ensures that this variable is stable across connections from the same
local terminal, so you will be able to restore the same shpool session by
simply sshing to your cloudtop from the same local terminal going forward.
To use shpool in this mode, just add

```
if [[ $- =~ i ]] && [[ -n "$SSH_TTY" ]]; then
   exec $HOME/.cargo/bin/shpool attach "ssh-$(basename $SSH_TTY)"
fi
```

to the .bashrc on your remote workstation.

## Bugs

The TODO file in the root of this repo is the most sophisticated project
planning tool in use by the shpool project at the moment. It contains
a list of known bugs and future plans. If you wish to report a bug or
are having trouble with shpool, feel free to ping me (pailes@) directly
and I'll add your bug to the file and try to help you work around it.

## Hacking

For information on how to develop shpool, see [HACKING.md](./HACKING.md).
