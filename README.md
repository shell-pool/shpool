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

#### Using systemd to run the daemon

Run

```
cargo install shpool
curl -fLo "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/shpool.service" --create-dirs https://raw.githubusercontent.com/shell-pool/shpool/master/systemd/shpool.service
sed -i "s|/usr|$HOME/.cargo|" "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/shpool.service"
curl -fLo "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/shpool.socket" --create-dirs https://raw.githubusercontent.com/shell-pool/shpool/master/systemd/shpool.socket
systemctl --user enable shpool
systemctl --user start shpool
loginctl enable-linger
```

#### Without systemd

To install without setting up systemd, run

```
cargo install shpool
```

If you don't use systemd, you can either port the `systemd/shpool.service`
file to your own init system and use that, or you can use autodaemonization
mode to tell shpool to just fork a daemon process on the fly if it notices
one is not missing. Autodaemonization is enabled by default, so you don't
need to do anything special to use it, though you can control its behavior
with the `nodaemonize` config option and the `-d/-D` command line switches.

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
might find that `shpool` still think a terminal is connected to
your session when you attempt to reattach. This is likely because
an ssh proxy is holding the connection open in the vain hope that
it will get some traffic again. You can just run `shpool detach main`
to force the session to detach and allow you to attach.

This README covers basic usage, but you can also check out
[the wiki](https://github.com/shell-pool/shpool/wiki) for
more tips and tricks.

### [Troubleshooting](https://github.com/shell-pool/shpool/wiki/Troubleshooting)

The [troubleshooting](https://github.com/shell-pool/shpool/wiki/Troubleshooting)
wiki page contains some information about known pitfalls.

### [Configuration](./CONFIG.md)

You can customize some of `shpool`s behavior by editing your
`~/.config/shpool/config.toml` file. For an in depth discussion
of configuration options see [CONFIG.md](./CONFIG.md).

### Shell Config

##### bash

If you use bash, you may want to ensure that the `huponexit` option
is set to make sure that child processes exit when you leave a
shell. Without this setting, background processes you have
spawned over the course of your shell session will stick around
in the `shpool` daemon's process tree and eat up memory. To set
this option add

```
shopt -s huponexit
```

to your `~/.bashrc`.

### Subcommands

#### shpool daemon

The `daemon` subcommand causes `shpool` to run in daemon mode. When running in
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
Will detach the current session if run from inside a `shpool`
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

To do this, you can define "hosts" for sessions named `main` and `edit`
in a config block in `~/.ssh/config` on your client machine, like so

```
Host = main edit
    Hostname remote.host.example.com

    RemoteCommand shpool attach -f %k
    RequestTTY yes
```

You can then attach to these sessions with `ssh main` or `ssh edit`.
`%k` expands to the "host" named on the command line.

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
can set up your system to automatically generate a `shpool`
session name based on your local terminal emulator's tty
number. To do so, you can add a block of custom ssh config
in the `~/.ssh/config` of your local machine like

```
Host = by-tty
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

## Comparison with other tools

### `tmux` and GNU `screen`

`tmux` is probably the best known session persistence tool, and
GNU `screen` has a similar feature set, so in comparison to `shpool`
it can be thought of as belonging to the same category.

The main way that `shpool` differs from `tmux` is that `tmux` is a
terminal multiplexer which necessarily means that it offers session
persistence features, while `shpool` only aims to be a session
persistence tool. In contrast to `tmux` the philosophy of `shpool`
is that managing different terminals is the job of your display or
window manager, not your session persistence tool. Every operating
system has its own idioms for switching between applications, and
there is no reason to switch to different idioms when switching
between terminals. Especially for users of tiling window managers
such as `i3`, `sway` or `xmonad`, tmux's multiplexing features are
redundant.

While `tmux` renders terminal contents remotely and only paints
the current view to the screen, `shpool` just directly sends
all shell output back to the user's local terminal. This means
that all rendering is handled by a single terminal state machine
rather than going through `tmux`s internal in-memory terminal
before getting formatted and re-rendered by the local terminal.
This has performance implications, and probably most
importantly means that a terminal using `shpool` will feel
completely native. Scrollback and copy-paste will work exactly
as they do in your native terminal, while they can behave differently
when using `tmux`.

### [`mosh`](https://github.com/mobile-shell/mosh)

`mosh` is another tool focused on providing persistent remote shell
sessions. It differs from the other tools discussed here in that it
has its own network protocol, which it bootstraps off of regular
ssh. Like `tmux`, it renders the screen contents remotely and sends
just the current view back. It is somewhat unique in trying to
predicatively guess the right output to display to the user if
there is a network lag.

`shpool` differs from `mosh` in that it has nothing to do with
the network, remaining confined to a single machine like most of
these other tools. Just like in the case of `tmux`, `mosh` will
impact the way scrollback and copy-paste work, while `shpool`
keeps them feeling entirely native.

### [`dtach`](https://github.com/crigler/dtach), [`abduco`](https://github.com/martanne/abduco), and [`diss`](https://github.com/yazgoo/diss)

These tools have the most in common with `shpool`. Just like `shpool`, they
eschew multiplexing and just send the raw bytes back to you for your local
terminal to render. While you could say that `shpool` aims to be a
simpler version of `tmux`, these tools follow the same philosophy with
an even greater laser focus on simplicity and doing one thing well.

`shpool` aims to be an easy and pleasant experience for people
who just want session persistence without having to care about
it too much, so it has a few more "cushy" features that would
not be as good a fit for the focus on simplicity of these
tools.

The most obvious of these features is the difference between
how `shpool` and these programs handle re-attaches. Though under normal operation,
`shpool` does not do any rendering and subsetting of the shell
output, it continually maintains an in-memory render of the
terminal state via the [`shpool_vt100`](https://crates.io/crates/shpool_vt100)
crate. On reattach, `shpool` will use this in-memory render to
re-draw the screen, so you can easily see where you were when
your connection dropped. This even allows you to see output
generated after your connection dropped.

Another such feature is the automatic prompt prefix. `shpool`
will detect when you are using a known shell (currently
`bash`, `zsh`, or `fish`) and automatically inject a prefix
into your prompt to let you know the name of the `shpool` session
you are in. This adds some nice context so you don't lose
track of your terminals and have some hint about the current
terminal state.

There are also some features `shpool` is missing which these
programs have. In particular, it seems that `dtach` and `abduco`
support shared sessions, while `shpool` only allows a single
client to be connected to a particular session at a time.
There may be more since I don't know these tools as well
as `shpool`.

## Hacking

For information on how to develop shpool, see [HACKING.md](./HACKING.md).
