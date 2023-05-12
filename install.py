#!/usr/bin/env python3

"""
Helper script to install shpool and its dependencies.
"""

import argparse
from collections import abc
import logging
import os
import shutil
import subprocess
import sys
from typing import Any

LOG = None  # Logger

def _cargo() -> str:
  """Gets the absolute path to cargo binary."""
  abs_home = _expanded_abs_path("~")
  return os.path.join(abs_home, ".cargo", "bin", "cargo")


class _CustomLogFormatter(logging.Formatter):
  """Custom log formatter."""
  _yellow = "\x1b[33;20m"
  _reset = "\x1b[0m"
  _format = "%(asctime)s - %(name)s - %(levelname)s - %(message)s"

  FORMATS = {
      logging.WARNING: _yellow + _format + _reset,
  }

  def format(self, record: abc.Mapping[Any, Any]) -> str:
    """Formats a log record.

    Args:
      record: Log record.

    Returns:
      Formatted log record.
    """
    log_fmt = self.FORMATS.get(record.levelno)
    formatter = logging.Formatter(log_fmt)
    return formatter.format(record)


def _setup_logging(filepath: str) -> None:
  """Sets up console and file logging.

  Args:
   filepath: Path where the log file should be stored.
  """
  global LOG
  LOG = logging.getLogger()
  LOG.setLevel(logging.INFO)

  formatter = _CustomLogFormatter()
  stdout_handler = logging.StreamHandler(sys.stdout)
  stdout_handler.setLevel(logging.INFO)
  stdout_handler.setFormatter(formatter)
  LOG.addHandler(stdout_handler)

  file_handler = logging.FileHandler(filepath)
  file_handler.setLevel(logging.DEBUG)
  file_handler.setFormatter(formatter)
  LOG.addHandler(file_handler)


def _log_banner(msg: str,
                sep: str = "=",
                width: int = 80,
                level: int = logging.INFO) -> None:
  """Utility to print a banner in logs.

  Args:
    msg: Banner message.
    sep: Separator for outer boundary of the banner.
    width: Width of the banner.
    level: Log level at which the banner should be printed.
  """
  logger_level = {
      logging.INFO: LOG.info,
      logging.DEBUG: LOG.debug,
      logging.ERROR: LOG.error,
      logging.WARNING: LOG.warning,
  }
  logger = logger_level[level]
  logger("\n\n")
  logger(sep * width)
  logger(msg)
  logger(sep * width)


def _prompt_user_to_setup_toolchain() -> None:
  """Prompts user to setup rust toolchain."""
  _log_banner(
      "Please setup the rust toolchain now, script will wait for you till "
      "then. Install the toolchain via: https://rustup.rs/\n(Optional: add "
      "`source \"$HOME/.cargo/env\"` to your ~/.bashrc or ~/.profile, if you "
      "would like to use cargo in a different terminal)",
      sep="!",
      level=logging.WARNING)
  input("\n\n\t\tWhen done, hit enter to continue.\n\n")
  abs_path = _expanded_abs_path("~")
  LOG.debug("Fetching cargo version ... ")
  _execute_command(f"{_cargo()} --version")


def _expanded_abs_path(path: str) -> str:
  """Expands the variables in path as well as gets the absolute path.

  Args:
    path: Path to expand

  Returns:
    Absolute path with bash variables expanded.
  """
  return os.path.abspath(os.path.expanduser(os.path.expandvars(path)))


def _user_input(msg: str, default: str = "n", hint: str = None) -> bool:
  """Prompts user with the message and expects a yes / no input from user.

  Args:
    msg: Message in the prompt.
    default: Default selection.
    hint: Optional hint to tell user what will the script do based on their
      input.

  Returns:
    A bool with True meaning a yes from user and False as no.
  """
  reply = ""
  while reply.lower() not in ("y", "yes", "n", "no"):
    reply = input("\n\n" + msg + ("" if hint is None else f"\nHint: {hint}") +
                  " (y/n) : ").strip().lower()
    if reply.lower() in ("y", "yes"):
      return True
    elif reply.lower() in ("n", "no"):
      return False
    else:
      LOG.error("Did not get a valid y/n response, retrying.")
  return default.lower() in ("y", "yes")


def _execute_command(command: str,
                     strict: bool = True,
                     success_return_code: int = 0,
                     env: abc.Mapping[str, str] = None,
                     cwd: str = None,
                     quiet: bool = False) -> tuple[str, str, int]:
  """Executes the provided command on the host.

  Args:
    command: Command to execute.
    strict: Whether to raise an exception or not if the success criterion is not
      met.
    success_return_code: Expected return code on a successful execution.
    env: Environment dict to pass to the command being run.
    cwd: Current working directory for the command being run.
    quiet: If true, logs (including error level) go to debug level to avoid
      cluttering the console.

  Returns:
    A tuple containing stdout, stderr and returncode from the executed command.

  Raises:
    Exception: A generic exception about the failed execution.
  """
  LOG.debug("Running command: %s", command)
  p = subprocess.run(
      command,
      shell=True,
      capture_output=True,
      env=env,
      cwd=cwd,
      text=True,
      check=False)
  if p.stdout.strip():
    if quiet:
      LOG.debug(p.stdout)
    else:
      LOG.info(p.stdout)
  if p.stderr.strip():
    if quiet:
      LOG.debug("ERROR - %s", p.stderr)
    else:
      if strict:
        LOG.error(p.stderr)
      else:
        LOG.warning(p.stderr)
  if p.returncode != success_return_code and strict:
    LOG.error("Command failed with stderr: %s", p.stderr)
    LOG.error("Command failed with stdout: %s", p.stdout)
    exc_message = (f"Failed to execute: '{command}', observed return code: "
                   f"{p.returncode} (expected: {success_return_code})")
    raise Exception(exc_message)
  return p.stdout, p.stderr, p.returncode


def _get_parser() -> argparse.ArgumentParser:
  """Sets up args parser to get input arguments to the script.

  Returns:
    Arguments object containing all the options for the script.
  """
  parser = argparse.ArgumentParser(
      description="Utility to set up shpool on glinux machines")
  parser.add_argument(
      "--log-file",
      dest="log_file",
      default="/tmp/shpool_setup.log",
      help="Path where script execution logs will be stored")
  parser.add_argument(
      "--shpool-checkout-dir",
      dest="shpool_checkout_dir",
      default="~/shpool",
      help="Path where shpool repo will be checked out")
  return parser


def _check_possibly_remove_existing(name: str,
                                    target_dir: str,
                                    hint: str = None) -> None:
  """Checks and removes existing dir if user agrees to it.

  Args:
    name: User friendly / human readable label for the target directory. Helpful
      for logging purposes.
    target_dir: Directory path to verify for existence (and optional removal).
    hint: Hint to the user about what script will do if they choose to keep the
      target dir.

  Returns:
    True if either the directory didn't exist previously or if the user removed
    it now, else False.
  """
  abs_path = _expanded_abs_path(target_dir)
  if not os.path.exists(abs_path):
    return True
  is_dir = os.path.isdir(abs_path)
  dir_or_file = "directory" if is_dir else "file"
  if not _user_input(
      f"{name} {dir_or_file} already exists at: {target_dir}, "
      "Should I remove it and checkout again?",
      hint=hint):
    LOG.info("Skipping %s checkout as it already exists ...", target_dir)
    return False

  LOG.info("Removing existing %s ...", target_dir)
  if is_dir:
    shutil.rmtree(abs_path)
  else:
    os.remove(abs_path)
  return True


def _get_shpool_source(target_dir: str) -> None:
  """Gets shpool source into the target directory.

  Args:
    target_dir: Directory path where the source will be checked out.
  """
  _log_banner(
      f"Fetching shpool source code into: {target_dir} ...\n", sep="-")
  clean_install = _check_possibly_remove_existing(
      "shpool source",
      target_dir,
      hint="Script will still continue to next step even if you choose to keep"
      " existing checkout.")
  if not clean_install:
    return
  abs_path = _expanded_abs_path(target_dir)
  git_repo = "rpc://team/cloudtop-connectivity-eng-team/shpool"
  _execute_command(f"git clone {git_repo} {abs_path}")

  LOG.info("Setting up git hook for shpool ...")
  hooks_dir = os.path.join(abs_path, ".git", "hooks")
  os.makedirs(hooks_dir, exist_ok=True)
  commit_msg = os.path.join(hooks_dir, "commit_msg")
  commit_msg_source = (
    "https://gerrit-review.googlesource.com/tools/hooks/commit-msg")
  _execute_command(f"curl -Lo {commit_msg} {commit_msg_source}", cwd=abs_path)
  _execute_command(f"chmod +x {commit_msg}", cwd=abs_path)


def _check_install_dependencies(shpool_checkout_dir: str) -> None:
  """Installs required dependencies if not already present."""
  _log_banner("Installing dependencies ...", sep="-")
  _execute_command(
    "sudo apt-get install -y git git-remote-google",
    env={
          **os.environ, "DEBIAN_FRONTEND": "noninteractive"
    }, quiet=True)

  _get_shpool_source(shpool_checkout_dir)

  abs_home = _expanded_abs_path("~")
  if not shutil.which(_cargo(), path=abs_home):
    _prompt_user_to_setup_toolchain()
  else:
    _execute_command(f"{_cargo()} --version")


def install_systemd_service(shpool_checkout_dir: str) -> None:
  """Installs shpool as a systemd service."""
  _log_banner("Installing shpool as systemd service ...", sep="-")
  abs_path = _expanded_abs_path("~/.config/systemd/user")
  os.makedirs(abs_path, exist_ok=True)
  _execute_command(f"cp systemd/* {abs_path}", cwd=shpool_checkout_dir)
  _execute_command("systemctl --user enable shpool")
  _execute_command("systemctl --user start shpool")


def main() -> None:
  """Entry point to the script."""
  parser = _get_parser()
  if "-h" in sys.argv or "--help" in sys.argv:
    parser.print_help(sys.stderr)
    return
  args = parser.parse_args()
  _setup_logging(args.log_file)

  shpool_checkout_dir = _expanded_abs_path(args.shpool_checkout_dir)

  _log_banner("Installing shpool on your system...", sep="=")
  _check_install_dependencies(shpool_checkout_dir)

  _log_banner("Installing shpool from source...", sep="-")
  _execute_command(f"{_cargo()} build --release", cwd=shpool_checkout_dir)
  cargo_bin_dir = os.path.join(os.environ["HOME"], ".cargo", "bin")
  if not os.path.exists(cargo_bin_dir):
    os.makedirs(cargo_bin_dir)
  shutil.copy(
      os.path.join(shpool_checkout_dir, "target", "release", "shpool"),
      os.path.join(cargo_bin_dir, "shpool"))
  install_systemd_service(shpool_checkout_dir)
  _log_banner("Almost done installing shpool on your system...", sep="-")
  _log_banner(
      "Remember to add (and/or manually execute) "
      "`source \"$HOME/.cargo/env\"` to your ~/.bashrc or ~/.profile so that "
      "you can access the `shpool` binary without specifying its full path.",
      sep="!",
      level=logging.WARNING)
  input("\n\n\t\t, Hit enter to finish the installation.\n\n")
  abs_path = _expanded_abs_path("~")
  _log_banner("shpool is installed on your system now", sep="=")



if __name__ == "__main__":
  main()
