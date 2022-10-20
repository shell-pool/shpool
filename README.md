# shpool

`shpool` is a service that enables session persistence by allowing the
creation of named shell sessions owned by `shpool` so that the session
is not lost if the connection drops. `shpool` can be thought of as a lighter
weight alternative to `tmux` or GNU `screen`. While `tmux` and `screen` take over
the whole terminal and provide window splitting and tiling features, `shpool`
only provides persistent sessions. The biggest advantage of this approach is
that `shpool` does not break native scrollback.

## Hacking

For information on how to develop shpool, see [HACKING.md](./HACKING.md).

## Usage

`shpool` has a few different subcommands for its various modes.

### ssh Extension Mode

`shpool` can be used as an ssh extension to add session persistence to native
ssh invocations. When used in this mode, shpool will generate a name based
on the tty number of the terminal you are using and various metadata like your
username and client hostname. In order to set up the shpool extension for a given
remote host, edit your `~/.ssh/config` file to contain a block like the
following:

```
Host = your-ssh-target-name
    Hostname your.ssh.host.example.com

    RemoteCommand /usr/bin/shpool plumbing ssh-remote-command
    PermitLocalCommand yes
    LocalCommand ssh -oPermitLocalCommand=no -oRemoteCommand="/usr/bin/shpool plumbing ssh-local-command-set-name '%u@%h:%p$(tty)'" %n
```

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

#### `shpool kill`

Kills a named shell session.

#### `shpool ssh`

The `ssh` subcommand generates a random shell name, then invokes ssh to
access a remote host before invoking `shpool attach` on the remote host.
In the event that the ssh pipe breaks, `shpool ssh` redials the connection
and immediately attempts to reattach to the shell.

## Project Setup

Clone the repository with

```
git clone rpc://team/cloudtop-connectivity-eng-team/shpool && (cd shpool && f=`git rev-parse --git-dir`/hooks/commit-msg ; mkdir -p $(dirname $f) ; curl -Lo $f https://gerrit-review.googlesource.com/tools/hooks/commit-msg ; chmod +x $f)
```


