#!/bin/bash
# A fake shell that reads stdin but never executes anything.
# This simulates a shell that is slow to start up, which causes
# wait_for_startup to block forever.
sleep 3600
