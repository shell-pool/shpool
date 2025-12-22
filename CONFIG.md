# config

The canonical documentation of shpool's config is the comments
on the `Config` struct defined in `libshpool/src/config.rs`, but
this document aims to provide some high level explanations of
some common configuration options.

You can specify the path to your config file by passing a
`-c /path/to/config.toml` flag, or by creating and
editing `~/.config/shpool/config.toml`.

## Prompt Prefix

By default, `shpool` will detect when you are using a shell it knows
how to inject a prompt into. Currently, those shells include `bash`,
`zsh` and `fish`, but more may be added in the future. If it noticed
you are using one such shell, it will inject the prompt prefix
`shpool:$SHPOOL_SESSION_NAME` at the beginning of your prompt
in order to hint to you when you are inside of a `shpool` session.

You can customize this prompt prefix by setting a new value in
your config. For example, to show the `shpool` session name
inside square brackets, you can put

```
prompt_prefix = "[$SHPOOL_SESSION_NAME]"
```

in your config file. If you want to instead completely suppress
the prompt injection, you can just set a blank `prompt_prefix`
with

```
prompt_prefix = ""
```

this allows you to write a custom prompt hook in your .rc files
that examines the `$SHPOOL_SESSION_NAME` environment variable
directly, or eschew a `shpool` prompt customization entirely.

## Session Restore Mode

`shpool` can do a few different things when you re-attach to an existing
session. You can choose what you want it to do with the `session_restore_mode`
configuration option.

### `"screen"` (default) - restore a screenful of history

The `"screen"` option causes `shpool` to re-draw sufficient output to fill the
entire screen of the client terminal as well as using the SIGWINCH trick
described in the `"simple"` section below. This will help restore
context for interactive terminal sessions that are not full blown ncurses
apps. `"screen"` is the default reattach behavior for `shpool`.
You can choose this option explicitly by adding

```
session_restore_mode = "screen"
```

to your `~/.config/shpool/config.toml`.

### `"simple"` - only ask child processes to redraw

The `"simple"` option avoids restoring any output. In this reconnect mode, `shpool` will
issue some SIGWINCH signals to try to convince full screen ncurses apps
such as vim or emacs to re-draw the screen, but will otherwise do nothing.
Any shell output produced when there was no client connected to the session
will be lost. You can choose this connection mode by adding

```
session_restore_mode = "simple"
```

to your `~/.config/shpool/config.toml`.

### `{ lines = n }` - restore the last n lines of history

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

## Detach Keybinding

You may wish to configure your detach keybinding.
By default, `shpool` will detach from the current user session when you
press the sequence `Ctrl-Space Ctrl-q` (press `Ctrl-Space` then release
it and press `Ctrl-q`, don't try to hold down all three keys at once),
but you can configure a different binding by adding an entry
like

```
[[keybinding]]
binding = "Ctrl-a d"
action = "detach"
```

to your `~/.config/shpool/config.toml`.

For the moment, control is the only modifier key supported, but the keybinding
engine is designed to be able to handle more, so if you want a different one,
you can file a bug with your feature request.

## Initial Directory

By default, shpool will always drop you off in your home directory when it
creates a new shell, but you can tell it to start in a different place if
you want. You can either use the `-d/--dir` switch on the `shpool attach`
command to specify a directory, or you can set a new default in your
config with

```
default_dir = "/path/to/default-dir"
```

Note that the path `.` has a special meaning. It indicates that shpool should
start your shell in whatever directory `shpool attach` is invoked from. For
example you can run `shpool attach -d . mysession` to start a session in the
current directory. You can also put

```
default_dir = "."
```

in your config.

## motd

`shpool` has support for displaying the message of the day (the message `sshd`
shows you when you first log into a system). This is most relevant to users
in institutional settings where important information gets communicated
via the message of the day.

### never mode

```
motd = "never"
```

currently, this is the default mode. In this mode, the message of the day will
not be shown by `shpool`.

### dump mode

```
motd = "dump"
```

in dump mode, `shpool` will dump out the motd inline the first time you
start a new session, but you will not see it when you re-attach to an
existing session.

### pager mode

```
[motd.pager]
bin = "less"
```

in pager mode, `shpool` will display the message of the day in a configurable
pager program. The pager must accept a file name to display as its first argument.
`shpool` will launch the pager in a pty and wait until it exits before moving
on to the actual terminal session. Pager mode is more disruptive than
dump mode, but it allows shpool to show you the motd even if you have a single
long running session you keep around for months and continually reattach to.
