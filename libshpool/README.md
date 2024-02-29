# libshpool

libshpool contains the meat of the implementation for the
shpool command line tool. You almost certainly don't want to
be using it directly, but with it you can create a wrapper
binary. It mostly exists because we want to add monitoring
to an internal google version of the tool, but don't believe
that telemetry belongs in an open-source tool. Other potential
use-cases such as incorporating a shpool daemon into an
IDE that hosts remote terminals could be imagined though.
