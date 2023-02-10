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

## Installation

The easiest way to install shpool is to use the installer script

```
$ /google/data/ro/users/pa/pailes/shpool/install.py --shpool-checkout-dir=/tmp/shpool-install
```

If you want more details on the installation steps, they can be found
in HACKING.md

## Usage

In order to use `shpool` you must start the shpool daemon, either
by using the `systemd` user level unit file as described above,
or by manually running `shpool daemon`. Once the daemon is running,
you can connect to it either by running `shpool attach <session name>`
or by using the ssh plugin mode described above.

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

`shpool` can be used as an ssh extension to add session persistence to native
ssh invocations. When used in this mode, shpool will generate a name based
on the tty number of the terminal you are using and various metadata like your
username and client hostname. In order to set up the shpool extension for a given
remote host, edit your `~/.ssh/config` file *on the client machine* to contain
a block like the following:

```
Host = your-ssh-target-name
    Hostname your.ssh.host.example.com

    RemoteCommand $HOME/.cargo/bin/shpool plumbing ssh-remote-command
    PermitLocalCommand yes
    LocalCommand ssh -oPermitLocalCommand=no -oRemoteCommand="$HOME/.cargo/bin/shpool plumbing ssh-local-command-set-metadata '%u@%h:%p$(tty)'" %n
```

Note that due to limitations in the hooks that ssh exposes to us,
you will need to gnubby touch twice in order to use `shpool` in
this mode.

## Hacking

For information on how to develop shpool, see [HACKING.md](./HACKING.md).
