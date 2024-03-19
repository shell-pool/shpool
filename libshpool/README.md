# libshpool

libshpool contains the meat of the implementation for the
shpool command line tool. You almost certainly don't want to
be using it directly, but with it you can create a wrapper
binary. It mostly exists because we want to add monitoring
to an internal google version of the tool, but don't believe
that telemetry belongs in an open-source tool. Other potential
use-cases such as incorporating a shpool daemon into an
IDE that hosts remote terminals could be imagined though.

## Integrating

In order to call libshpool, you must keep a few things in mind.
In spirit, you just need to call `libshpool::run(libshpoo::Args::parse())`,
but you need to take care of a few things manually.

1. Handle the `version` subcommand. Since libshpool is a library, the output
   will not be very good if the library handles the versioning.
2. Depend on the `motd` crate and call `motd::handle_reexec()` in your `main`
   function.
