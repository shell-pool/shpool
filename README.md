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

```
sudo glinux-add-repo shpool unstable && \
sudo apt update && \
sudo apt install shpool
```

### Shell Config

#### bash

If you use bash, you may want to ensure that the `huponexit` option
is set to make sure that child processes exit when you leave a
shell. Without this setting, background processes you have
spawned over the course of your shell session will stick around
in the shpool daemon's process tree and eat up memory. To set
this option add

```
shopt -s huponexit
```

to your `~/.bashrc`.

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

### Configuration

You can specify some additional configuration options to the daemon
by passing a `-c /path/to/config.toml` flag, or by creating and
editing `~/.config/shpool/config.toml`. The options available
are documented in detail in `src/daemon/config.rs`, but the most
common thing to want to configure is your detach keybinding.
By default, shpool will detach from the current user session when you
press the sequence `Ctrl-Space Ctrl-q` (press `Ctrl-Space` then release
it and press `Ctrl-q`, don't try to hold down all three keys at once),
but this you can configure a different binding by adding an entry
like.

```
[[keybinding]]
binding = "Ctrl-a d"
action = "Detach"
```

for the moment, control is the only modifier key supported, but the keybinding
engine is designed to be able to handle more, so if you want a different one,
you can file a bug with your feature request.

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

#### Local tty based

To set up your system to automatically generate a shpool session name
based on your local terminal emulator's tty number, add the following
line to the .profile or .bashrc on your local machine

```
export LC__SHPOOL_SET_SESSION_NAME="ssh-$(basename $(tty))"
```

then add an entry for the remote machine in your local .ssh/config,
making sure to add the line `SendEnv LC__SHPOOL_SET_SESSION_NAME`, for example

```
Host = remote
    User remoteuser
    Hostname remote.host.example.com
    SendEnv LC__SHPOOL_SET_SESSION_NAME
```

then in your *remote* .bashrc, add an entry to automatically exec into
a shpool session based on the tty variable we just forwarded

```
if [[ $- =~ i ]] && [[ -n "$LC__SHPOOL_SET_SESSION_NAME" ]]; then
   exec shpool attach "$LC__SHPOOL_SET_SESSION_NAME"
fi
```

Note that your remote machine must be configured to allow
`LC__SHPOOL_SET_SESSION_NAME` to get forwarded (this is why we use
the `LC_` prefix since it may be more likely to be accepted).

#### Explicitly named sessions

Rather than derive the session name from the local tty, you may prefer
to explicitly set the names for your sessions. In this case, in addition to
the changes for generating session names based on local tty number, you
can add a function like

```
function shpool-ssh () {
    if [ -z ${1+x} ] ; then
        echo "expect a shpool session name as an arg"
    fi
    env LC__SHPOOL_SET_SESSION_NAME=$1 ssh -oSendEnv=LC__SHPOOL_SET_SESSION_NAME remote.host.example.com
}
```

then invoke it like `shpool-ssh main`. You may want to change the function
name depending on the name of your remote host.

## Bugs

You can report a bug
[here](https://b.corp.google.com/issues/new?component=1320938&template=0).

## Hacking

For information on how to develop shpool, see [HACKING.md](./HACKING.md).
