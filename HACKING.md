
# Preserving Logs in Tests

By default, tests will clean up log files emitted by the various
shpool subprocesses they spawn. In order get the tests to leave
log files around for later inspection, you can set the
`SHPOOL_LEAVE_TEST_LOGS` environment variable to `true`.

For example to run `attach_test` and leave log files in place
you might run

```
$ SHPOOL_LEAVE_TEST_LOGS=true cargo test --test attach happy_path -- --nocapture
```


