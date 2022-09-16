# shpool

`shpool` is a service that enables session persistence by allowing the
creation of named shell sessions owned by `shpool` so that the session
is not lost if the connection drops. `shpool` can be thought of as a lighter
weight alternative to `tmux` or GNU `screen`. While `tmux` and `screen` take over
the whole terminal and provide window splitting and tiling features, `shpool`
only provides persistent sessions. The biggest advantage of this approach is
that `shpool` does not break native scrollback.

## Usage

`shpool` has a few different subcommands for its various modes.

### `shpool daemon`

The `daemon` subcommand causes shpool to run in daemon mode. When running in
this mode, `shpool` listens for incoming connections and opens up subshells,
retaining ownership of them in a table. In general, this subcommand will not
be invoked directly by users, but will instead be called from a systemd unit
file.

TODO: set a magic variable inside subshells saying what the shpool session
      name is.

### `shpool attach`

The `attach` subcommand connects to the `shpool daemon` instance, passing in a
name. If the name is new, a new shell is created, and if it already exists it
just attaches to the existing session so long as no other terminal is currently
connected to that session.

### `shpool list`

Lists all the current shell sessions.

### `shpool kill`

Kills a named shell session.

### `shpool ssh`

The `ssh` subcommand generates a random shell name, then invokes ssh to
access a remote host before invoking `shpool attach` on the remote host.
In the event that the ssh pipe breaks, `shpool ssh` redials the connection
and immediately attempts to reattach to the shell.

## Project Setup

Clone the repository with

```
git clone rpc://team/cloudtop-connectivity-eng-team/shpool && (cd shpool && f=`git rev-parse --git-dir`/hooks/commit-msg ; mkdir -p $(dirname $f) ; curl -Lo $f https://gerrit-review.googlesource.com/tools/hooks/commit-msg ; chmod +x $f)
```


