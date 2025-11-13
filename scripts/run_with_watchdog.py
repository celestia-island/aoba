#!/usr/bin/env python3
"""Execute a shell command with inactivity monitoring.

This helper mirrors the behaviour of `tee` by streaming command output to
stdout while appending it to a specified log file. It also tracks the elapsed
idle time since the last chunk of output. If the idle time exceeds the
configured limit, the command is terminated.
"""

from __future__ import annotations

import argparse
import os
import select
import subprocess
import sys
import time
from typing import Optional


def terminate_process(proc: subprocess.Popen) -> None:
    """Attempt to terminate the process gracefully, then force kill."""
    if proc.poll() is not None:
        return

    try:
        proc.terminate()
    except ProcessLookupError:
        return

    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        try:
            proc.kill()
        except ProcessLookupError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description="Run command with watchdog")
    parser.add_argument("--cmd", required=True,
                        help="Shell command to execute")
    parser.add_argument("--log-file", required=True,
                        help="File to append command output")
    parser.add_argument("--label", default="command",
                        help="Label shown in watchdog messages")
    parser.add_argument(
        "--inactivity-timeout",
        type=int,
        default=60,
        help="Seconds of no output before terminating (0 disables)",
    )
    parser.add_argument(
        "--notify-interval",
        type=int,
        default=10,
        help="Seconds between idle notifications (0 disables)",
    )
    args = parser.parse_args()

    inactivity_limit = max(0, args.inactivity_timeout)
    notify_interval = max(0, args.notify_interval)
    label = args.label or "command"

    log_path = os.path.abspath(args.log_file)
    log_dir = os.path.dirname(log_path)
    if log_dir and not os.path.exists(log_dir):
        os.makedirs(log_dir, exist_ok=True)

    start_time = time.time()
    last_output = start_time
    last_notify = start_time

    start_msg = f"[watchdog] Starting {label}"
    print(start_msg)
    sys.stdout.flush()

    with open(log_path, "a", encoding="utf-8") as log_file:
        log_file.write(start_msg + "\n")
        log_file.flush()

        proc = subprocess.Popen(
            ["bash", "-lc", args.cmd],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            bufsize=0,
        )

        exit_code: Optional[int] = None
        stdout_fd = proc.stdout.fileno() if proc.stdout else None

        try:
            while True:
                ready_fds: list[int] = []
                if stdout_fd is not None:
                    ready_fds, _, _ = select.select([stdout_fd], [], [], 1.0)
                now = time.time()

                if stdout_fd is not None and ready_fds:
                    chunk = os.read(stdout_fd, 4096)
                    if chunk:
                        text = chunk.decode("utf-8", errors="replace")
                        sys.stdout.write(text)
                        sys.stdout.flush()
                        log_file.write(text)
                        log_file.flush()
                        last_output = now
                        last_notify = now
                    else:
                        if proc.poll() is not None:
                            break
                else:
                    idle = now - last_output
                    if inactivity_limit and idle >= inactivity_limit:
                        note = (
                            f"[watchdog] No output for {int(idle)}s, terminating {label}"
                        )
                        print(note)
                        log_file.write(note + "\n")
                        log_file.flush()
                        terminate_process(proc)
                        exit_code = 124
                        break
                    if notify_interval and (now - last_notify) >= notify_interval:
                        note = (
                            f"[watchdog] Waiting for output ({int(idle)}s idle) from {label}"
                        )
                        print(note)
                        log_file.write(note + "\n")
                        log_file.flush()
                        last_notify = now

                if proc.poll() is not None and not ready_fds:
                    break
        except KeyboardInterrupt:
            note = f"[watchdog] Keyboard interrupt received, terminating {label}"
            print(note)
            log_file.write(note + "\n")
            log_file.flush()
            terminate_process(proc)
            exit_code = 130
        finally:
            if proc.stdout:
                proc.stdout.close()

        if exit_code is None:
            exit_code = proc.wait()
        else:
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass

        duration = time.time() - start_time
        summary = f"[watchdog] Finished {label} in {duration:.1f}s with exit code {exit_code}"
        print(summary)
        log_file.write(summary + "\n")
        log_file.flush()

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
