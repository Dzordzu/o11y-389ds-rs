#!/usr/bin/env python3
"""Helper script to be used as a pre-commit hook."""
import os
import sys
import subprocess


def get_project_root():
    return subprocess.getoutput("git rev-parse --show-toplevel")


def gitleaks_enabled():
    """Determine if the pre-commit hook for gitleaks is enabled."""
    out = subprocess.getoutput("git config --bool hooks.gitleaks")
    if out == "false":
        return False
    return True


def handle_taplo():
    if not os.environ.get("COMMIT_SKIP_TAPLO"):
        exitcode = os.WEXITSTATUS(os.system("taplo format --check"))
        if exitcode != 0:
            print("found taplo errors. (set COMMIT_SKIP_TAPLO=1 to skip this check)")
            sys.exit(1)


def handle_gitleaks():
    exitCode = os.WEXITSTATUS(os.system("gitleaks protect -v --staged"))
    if exitCode == 1:
        print(
            """Warning: gitleaks has detected sensitive information in your changes.
To disable the gitleaks precommit hook run the following command:

    git config hooks.gitleaks false
"""
        )
        sys.exit(1)


def handle_cargo():
    if not os.environ.get("COMMIT_SKIP_FMT"):
        exitcode = os.WEXITSTATUS(os.system("cargo fmt --check"))

        if exitcode != 0:
            print("found rustfmt errors. (set COMMIT_SKIP_FMT=1 to skip this check)")
            sys.exit(1)

    if not os.environ.get("COMMIT_SKIP_CLIPPY"):
        exitcode = os.WEXITSTATUS(os.system("cargo clippy -- -D warnings"))

        if exitcode != 0:
            print(
                "found clippy warnings. (set COMMIT_SKIP_CLIPPY=1 to skip this check)"
            )
            sys.exit(1)


handle_cargo()
handle_taplo()

if gitleaks_enabled():
    handle_gitleaks()
else:
    print(
        "gitleaks precommit disabled\
     (enable with `git config hooks.gitleaks true`)"
    )
