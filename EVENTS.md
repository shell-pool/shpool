# Events

`shpool` exposes an event stream so that external programs can react to changes
without polling. This way, a program (e.g. a TUI) can call `shpool list` (or the
equivalent `ConnectHeader::List` request over the main socket; see the
[`shpool-protocol`](./shpool-protocol) crate) after each event so that its model
is always consistent with shpool's state.

## The events socket

The daemon binds a sibling Unix socket next to the main shpool socket:

```bash
<runtime_dir>/shpool/shpool.socket   # main socket
<runtime_dir>/shpool/events.socket   # events socket (this protocol)
```

A subscriber connects to `events.socket` and reads events. The daemon ignores anything written to the events socket, so for subscribers it's effectively read-only.

## Event types

| `type`             | Meaning                                                  |
| ------------------ | -------------------------------------------------------- |
| `session.created`  | A new session was added to the table.                    |
| `session.attached` | A client attached or reattached to a session.            |
| `session.detached` | A client disconnected from a still-running session.      |
| `session.removed`  | A session was removed (shell exited, killed, or reaped). |

Subscribers should ignore unknown `type` values so that future event types do
not break older consumers.

## Wire format

The daemon writes one JSON object per line (JSONL). Each event looks like:

```json
{"type":"<event-type>"}
```

There are no other fields. To learn what the event refers to (which session,
when, etc.), call `shpool list` (or use `ConnectHeader::List`).

## Subscribing

For ad-hoc use, `shpool events` connects to the events socket and prints each
event line to stdout, flushing after each line:

```bash
shpool events | while read -r ev; do
  echo "got: $ev"
  shpool list
done

shpool events | jq .
```

## Slow subscribers

Each subscriber has a bounded outbound queue. A subscriber that falls too far
behind is dropped by the daemon (in which case the subscriber can always reconnect).
There is no replay, so events that fired while a subscriber was disconnected are
lost.
