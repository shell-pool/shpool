# shpool

`shpool` is a service that enables session persistence by allowing the
creation of named shell sessions owned by `shpool` so that the session
is not lost if the connection drops. `shpool` can be thought of as a lighter
weight alternative to `tmux` or GNU `screen`. While `tmux` and `screen` take over
the whole terminal and provide window splitting and tiling features, `shpool`
only provides persistent sessions. The biggest advantage of this approach is
that `shpool` does not break native scrollback or copy-paste.

## Installation

### Installing from crates.io

Run

```
cargo install shpool
curl -fLo "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/shpool.service" --create-dirs https://raw.githubusercontent.com/shell-pool/shpool/master/systemd/shpool.service
sed -i "s|/usr|$HOME/.cargo|" .config/systemd/user/shpool.service
curl -fLo "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/shpool.socket" --create-dirs https://raw.githubusercontent.com/shell-pool/shpool/master/systemd/shpool.socket
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

### Configuration

You can specify some additional configuration options to the daemon
by passing a `-c /path/to/config.toml` flag, or by creating and
editing `~/.config/shpool/config.toml`. The options available
are documented in detail in `libshpool/src/config.rs`, but there
are a few common things you may wish to tweak.

#### Detach Keybinding

You may wish to configure your detach keybinding.
By default, shpool will detach from the current user session when you
press the sequence `Ctrl-Space Ctrl-q` (press `Ctrl-Space` then release
it and press `Ctrl-q`, don't try to hold down all three keys at once),
but you can configure a different binding by adding an entry
like

```
[[keybinding]]
binding = "Ctrl-a d"
action = "Detach"
```

to you `~/.config/shpool/config.toml`.

For the moment, control is the only modifier key supported, but the keybinding
engine is designed to be able to handle more, so if you want a different one,
you can file a bug with your feature request.

#### Session Restore Mode

Shpool can do a few different things when you re-attach to an existing
session. You can choose what you want it to do with the `session_restore_mode`
configuration option.

##### `"screen"` (default) - restore a screenful of history

The `"screen"` option causes shpool to re-draw sufficient output to fill the
entire screen of the client terminal as well as using the SIGWINCH trick
described in the `"simple"` section below. This will help restore
context for interactive terminal sessions that are not full blown ncurses
apps. `"screen"` is the default reattach behavior for shpool.
You can choose this option explicitly by adding

```
session_restore_mode = "screen"
```

to your `~/.config/shpool/config.toml`.

##### `"simple"` - only ask child processes to redraw

The `"simple"` avoids restoring any output. In this reconnect mode, shpool will
issue some SIGWINCH signals to try to convince full screen ncurses apps
such as vim or emacs to re-draw the screen, but will otherwise do nothing.
Any shell output produced when there was no client connected to the session
will be lost. You can choose this connection mode by adding

```
session_restore_mode = "simple"
```

to your `~/.config/shpool/config.toml`.

##### `{ lines = n }` - restore the last n lines of history

The lines option is much like the `"screen"` option, except that rather
than just a screenful of text, it restores the last n lines of text
from the terminal being re-attached to. This could be useful if you
wish to have more context than a single screenful of text. Note that
n cannot exceed the value of the `output_spool_lines` configuration
option, but it defaults to the value of the lines option, so you likely
won't need to change it.

```
session_restore_mode = { lines = n }
```

where n is a number to your `~/.config/shpool/config.toml`.

#### Shell Config

##### bash

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

### Subcommands

#### shpool daemon

The `daemon` subcommand causes shpool to run in daemon mode. When running in
this mode, `shpool` listens for incoming connections and opens up subshells,
retaining ownership of them in a table. In general, this subcommand will not
be invoked directly by users, but will instead be called from a systemd unit
file.

#### shpool attach

The `attach` subcommand connects to the `shpool daemon` instance, passing in a
name. If the name is new, a new shell is created, and if it already exists it
just attaches to the existing session so long as no other terminal is currently
connected to that session. The `--ttl` flag can be used to limit how long the
session will last.

#### shpool list

Lists all the current shell sessions.

#### shpool detach

Detach from a one or more sessions without stopping them.
Will detach the current session if run from inside a shpool
session with no session name arguments.

#### shpool kill

Kills a named shell session.

### (Optional) Automatically Connect to shpool

#### Explicitly named sessions

Specifying session names yourself lets you assign logical
roles such as text editing to each session.

##### ssh config

If you typically connect to a small number of sessions with
the same jobs on a particular machine, custom ssh config
blocks on your client machine are probably the best
fit.

To do this, you can add a config block named `edit` like so

```
Host = edit
    Hostname remote.host.example.com

    RemoteCommand shpool attach -f edit
    RequestTTY yes
```

to `~/.ssh/config` on your client machine. You will need one
such block per session name. You can then invoke this with
`ssh edit`.

##### shell function

If you would rather have a little more flexibility in
specifying the session name and machine you are targeting,
you can make a custom shell function to let you specify
both at invocation time. Add

```
function shpool-ssh () {
    if [ $# -ne 2 ] ; then
        echo "usage: shpool-ssh <remote-machine> <session-name>" >&2
        return 1
    fi
    ssh -t "-oRemoteCommand=shpool attach -f $2" "$1"
}
```

to your `.bashrc` then invoke it like
`shpool-ssh remote.host.example.com main`.

#### Local tty based

Rather than specify an explicit name when you connect, you
can set up your system to automatically generate a shpool
session name based on your local terminal emulator's tty
number. To do so, you can add a block of custom ssh config
in the `~/.ssh/config` of your local machine like

```
Host = remote
    User remoteuser
    Hostname remote.host.example.com

    RemoteCommand shpool attach -f "ssh-$(basename $(tty))"
    RequestTTY yes
```

which you then invoke with `ssh by-tty`. You can apply the same principle
of using `$(basename $(tty))` to get a unique id for your local terminal
to the custom shell function approach as well.

The local-tty based approach has the advantage that you don't
need to specify a session name, but it can run into problems
if you have to close the local window and open a new terminal,
which can come up if your connection freezes rather than drops.

## Bugs

You can report a bug
[here](https://b.corp.google.com/issues/new?component=1320938&template=0).

## Hacking

For information on how to develop shpool, see [HACKING.md](./HACKING.md).
