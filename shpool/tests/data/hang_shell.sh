#!/bin/bash

# A shell that never starts up properly.
# It reads stdin to avoid SIGPIPE but never executes commands.
exec cat > /dev/null
