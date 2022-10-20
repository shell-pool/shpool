use anyhow::Context;

mod support;

use support::ExpectedOutput;

// the happy path for attaching
#[test]
fn attach_test() -> anyhow::Result<()> {
    let mut daemon_proc = support::DaemonProc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1")
        .context("starting attach proc")?;

    let echo_res = attach_proc.run_cmd("echo hi", ExpectedOutput::STDOUT)?;
    echo_res.stdout_re_match("(?m)^hi$")?;

    Ok(())
}

// TODO: there is something funky going on where the prompt sometimes
//       gets printed multiple times along with the command itself.
//       I'm pretty sure what is happening is that after starting, bash
//       is printing the initial prompt and the kernel tty subsystem's
//       line dicipline logic is echoing back our command, but there is
//       a race condition where sometimes it does not do this quickly
//       enough and we just wind up reading the initial prompt.
//
//       There is no real way to tell when termianl output is
//       "over", so it is hard to know when to stop slurping for tests.
//
//       My first attempt at fixing this by eagerly reading the initial
//       prompt when we first attach did not go so well.
