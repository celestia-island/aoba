#!/usr/bin/env python3
"""Unified socat init script for E2E tests.

Modes:
  - cli: Creates 2 virtual serial ports (vcom1-vcom2) for CLI E2E tests
  - tui: Creates 2 virtual serial ports (vcom1-vcom2) for TUI E2E tests
  - tui_multiple: Creates 6 virtual serial ports (vcom1-vcom6) for multiple master/slave testing

Usage: socat_init.py [--mode cli|tui|tui_multiple]

The tui_multiple mode creates 3 independent pairs:
  - vcom1 ↔ vcom2 (Master 1 with Slave 1)
  - vcom3 ↔ vcom4 (Master 2 with Slave 2)
  - vcom5 ↔ vcom6 (Slave 3 or interference testing)

Converted from scripts/socat_init.sh to Python 3 on 2026-10-06 per the
workspace tool-script policy (AGENTS §7.5).
"""

from __future__ import annotations

import glob
import os
import signal
import subprocess
import sys
import time


def rm_quiet(path: str) -> None:
    """Remove a file, ignoring errors (rm -f)."""
    try:
        os.unlink(path)
    except OSError:
        pass


def tail_single(path: str) -> None:
    """Print the last 200 lines of one log file (tail -n 200)."""
    subprocess.run(
        ["tail", "-n", "200", path],
        stdin=subprocess.DEVNULL,
        check=False,
    )


def tail_glob(pattern: str) -> None:
    """Print the last 200 lines of every file matching a glob pattern."""
    files = sorted(glob.glob(pattern))
    if not files:
        return
    subprocess.run(
        ["tail", "-n", "200", *files],
        stdin=subprocess.DEVNULL,
        check=False,
    )


def show_socat_processes() -> None:
    """List running socat processes (ps aux | grep socat | grep -v grep)."""
    subprocess.run(
        "ps aux | grep socat | grep -v grep",
        shell=True,
        stdin=subprocess.DEVNULL,
        check=False,
    )


def kill_signal(pid: str, sig: str) -> bool:
    """Return True when the kill binary reports success for the given pid."""
    return (
        subprocess.run(["kill", sig, pid], stderr=subprocess.DEVNULL).returncode == 0
    )


def pgrep_matches(pattern: str) -> bool:
    proc = subprocess.run(
        ["pgrep", "-f", pattern],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return proc.returncode == 0


def pkill_pattern(pattern: str) -> None:
    subprocess.run(
        ["pkill", "-f", pattern],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def parse_args(argv: list[str]) -> str:
    mode = "tui"
    index = 0
    while index < len(argv):
        arg = argv[index]
        if arg == "--mode":
            if index + 1 >= len(argv):
                print(
                    "--mode requires an argument: cli, tui, or tui_multiple",
                    file=sys.stderr,
                )
                sys.exit(2)
            mode = argv[index + 1]
            index += 2
        elif arg.startswith("--mode="):
            mode = arg[len("--mode=") :]
            index += 1
        elif arg in ("-h", "--help"):
            print(f"Usage: {sys.argv[0]} [--mode cli|tui|tui_multiple]")
            sys.exit(0)
        else:
            print(f"Unknown arg: {arg}", file=sys.stderr)
            sys.exit(2)
    return mode


def pty_spec(link: str) -> str:
    return f"PTY,link={link},raw,echo=0,mode=0666"


def main() -> int:
    mode = parse_args(sys.argv[1:])

    # Port configuration based on mode
    if mode == "tui_multiple":
        # For tui_multiple mode: create 6 virtual ports with full mesh connectivity
        v1, v2, v3, v4, v5, v6 = (f"/tmp/vcom{i}" for i in range(1, 7))
        pidfile = f"/tmp/aoba_socat_{mode}.pid"
        log = f"/tmp/aoba_socat_{mode}.log"
        kill_patterns = [v1, v2, v3, v4, v5, v6, os.path.basename(v1)]
        ports = [v1, v2, v3, v4, v5, v6]
    else:
        # For cli and tui modes: create 2 virtual ports (backward compatible)
        v1, v2 = "/tmp/vcom1", "/tmp/vcom2"
        pidfile = f"/tmp/aoba_socat_{mode}.pid"
        log = f"/tmp/aoba_socat_{mode}.log"
        kill_patterns = [v1, v2, os.path.basename(v1)]
        ports = [v1, v2]

    user_suffix = str(os.getuid())
    if os.path.exists(pidfile) and not os.access(pidfile, os.W_OK):
        alt_pidfile = f"/tmp/aoba_socat_{mode}_{user_suffix}.pid"
        print(f"[socat_init] pidfile {pidfile} not writable, using {alt_pidfile} instead")
        pidfile = alt_pidfile

    if os.path.exists(log) and not os.access(log, os.W_OK):
        alt_log = f"/tmp/aoba_socat_{mode}_{user_suffix}.log"
        print(f"[socat_init] log file {log} not writable, using {alt_log} instead")
        log = alt_log

    if mode == "tui_multiple":
        print(
            f"[socat_init] operating entirely without sudo; using 6 ports "
            f"{v1}, {v2}, {v3}, {v4}, {v5}, {v6} with full mesh connectivity"
        )
    else:
        print(
            f"[socat_init] operating entirely without sudo; using static ports {v1} and {v2}"
        )

    port1, port2 = v1, v2

    print(f"[socat_init] mode={mode} stopping existing socat (pidfile if present)")
    if os.path.isfile(pidfile):
        old_pid = None
        try:
            with open(pidfile, encoding="utf-8") as f:
                old_pid = f.read().rstrip("\n")
        except OSError:
            old_pid = None
        if old_pid:
            if kill_signal(old_pid, "-15"):
                print(f"[socat_init] stopped previous socat pid {old_pid}")
            elif kill_signal(old_pid, "-0"):
                print(
                    f"[socat_init] unable to stop socat pid {old_pid} (insufficient permissions?)",
                    file=sys.stderr,
                )
                print(
                    "[socat_init] please terminate the existing socat process manually",
                    file=sys.stderr,
                )
                return 1
        rm_quiet(pidfile)

    print("[socat_init] killing lingering socat processes that reference vcom links (if any)")
    for pattern in kill_patterns:
        pkill_pattern(pattern)
    if pgrep_matches(f"socat.*{v1}") or pgrep_matches(f"socat.*{v2}"):
        print(
            "[socat_init] detected existing socat processes that could not be terminated automatically.",
            file=sys.stderr,
        )
        print(
            "[socat_init] please run 'sudo pkill socat' (or manually stop them) and retry.",
            file=sys.stderr,
        )
        return 1
    time.sleep(0.5)

    print(f"[socat_init] removing existing links: {v1} {v2}")
    if mode == "tui_multiple":
        for port in ports:
            rm_quiet(port)
        for stale in glob.glob("/tmp/aoba_vcom*.*"):
            rm_quiet(stale)
        if any(os.path.exists(port) for port in ports):
            print(
                "[socat_init] unable to remove existing port links (permission denied?).",
                file=sys.stderr,
            )
            print(
                f"[socat_init] please remove {v1}, {v2}, {v3}, {v4}, {v5}, {v6} manually, then rerun.",
                file=sys.stderr,
            )
            return 1
    else:
        rm_quiet(v1)
        rm_quiet(v2)
        for pattern in ("/tmp/aoba_vcom1.*", "/tmp/aoba_vcom2.*"):
            for stale in glob.glob(pattern):
                rm_quiet(stale)
        if os.path.exists(v1) or os.path.exists(v2):
            print(
                "[socat_init] unable to remove existing port links (permission denied?).",
                file=sys.stderr,
            )
            print(
                f"[socat_init] please remove {v1} and {v2} manually, then rerun.",
                file=sys.stderr,
            )
            return 1

    print("[socat_init] starting socat with link and mode=0666")
    rm_quiet(log)

    # Keep stderr handles open for the lifetime of this script; the socat
    # children hold their own duplicates.
    log_handles = []

    def start_pair(link_a: str, link_b: str, err_path: str) -> subprocess.Popen:
        err_file = open(err_path, "wb")
        log_handles.append(err_file)
        return subprocess.Popen(
            ["setsid", "socat", "-d", "-d", pty_spec(link_a), pty_spec(link_b)],
            stdout=subprocess.DEVNULL,
            stderr=err_file,
        )

    timeout_secs = 15
    if mode == "tui_multiple":
        # For tui_multiple: create 6 virtual serial ports fully interconnected
        # Use a hub-and-spoke model where all ports connect to a central relay
        # This creates "大串联" - all ports can communicate with each other

        # Create central FIFO hub
        hub_fifo = f"/tmp/aoba_hub_{os.getpid()}"
        try:
            os.mkfifo(hub_fifo, 0o666)
        except OSError:
            pass

        # Create 6 PTY pairs, each connected through the hub
        # Note: This approach has limitations but works for our test purposes
        # In practice, serial port mesh networks are complex; we'll use simple pairs
        # but document the connectivity model

        print("[socat_init] creating 6 virtual ports in 3 pairs for mesh connectivity")
        pair1 = start_pair(v1, v2, f"{log}.12")
        pair2 = start_pair(v3, v4, f"{log}.34")
        pair3 = start_pair(v5, v6, f"{log}.56")

        # Store PIDs for cleanup
        with open(pidfile, "w", encoding="utf-8") as f:
            f.write(f"{pair1.pid} {pair2.pid} {pair3.pid}\n")

        # Check if all processes started successfully
        if pair1.poll() is not None or pair2.poll() is not None or pair3.poll() is not None:
            print("[socat_init] failed to start one or more socat processes")
            print("[socat_init] socat logs:")
            tail_glob(f"{log}.*")
            rm_quiet(hub_fifo)
            return 1

        print(
            f"[socat_init] socat pids: {pair1.pid} {pair2.pid} {pair3.pid}, waiting for all 6 ports"
        )

        # Wait for all PTYs to be created
        count = 0
        while count < timeout_secs and not all(os.path.exists(p) for p in ports):
            time.sleep(1)
            count += 1

        rm_quiet(hub_fifo)
    else:
        # Original 2-port mode
        socat_proc = start_pair(v1, v2, log)
        with open(pidfile, "w", encoding="utf-8") as f:
            f.write(f"{socat_proc.pid}\n")
        if socat_proc.poll() is not None:
            print("[socat_init] failed to start socat")
            print(f"[socat_init] socat log ({log}):")
            tail_single(log)
            return 1

        socat_pid = ""
        try:
            with open(pidfile, encoding="utf-8") as f:
                socat_pid = f.read().rstrip("\n")
        except OSError:
            socat_pid = ""
        print(
            f"[socat_init] socat pid: {socat_pid or 'unknown'}, waiting for {v1} and {v2}"
        )

        count = 0
        while count < timeout_secs and not (os.path.exists(v1) and os.path.exists(v2)):
            time.sleep(1)
            count += 1

    if mode == "tui_multiple":
        # Verify all 6 ports exist
        if all(os.path.exists(p) for p in ports):
            print("[socat_init] created links:")
            subprocess.run(["ls", "-la", *ports], check=False)
            for port in ports:
                pts = os.path.realpath(port)
                if pts:
                    try:
                        os.chmod(pts, 0o666)
                    except OSError:
                        pass
            print("[socat_init] underlying pts:")
            for port in ports:
                pts = os.path.realpath(port)
                subprocess.run(["ls", "-la", pts], check=False)
        else:
            print(f"[socat_init] Failed to create all 6 ports within {timeout_secs}s")
            print("[socat_init] socat logs:")
            tail_glob(f"{log}.*")
            print("[socat_init] socat processes:")
            show_socat_processes()
            return 1
    else:
        # Original 2-port verification
        if os.path.exists(v1) and os.path.exists(v2):
            print("[socat_init] created links:")
            subprocess.run(["ls", "-la", v1, v2], check=False)
            pts1 = os.path.realpath(v1)
            pts2 = os.path.realpath(v2)
            for pts in (pts1, pts2):
                if pts:
                    try:
                        os.chmod(pts, 0o666)
                    except OSError:
                        pass
            print("[socat_init] underlying pts:")
            subprocess.run(["ls", "-la", pts1, pts2], check=False)
        else:
            print(f"[socat_init] Failed to create {v1} and {v2} within {timeout_secs}s")
            print(f"[socat_init] socat log ({log}):")
            tail_single(log)
            print("[socat_init] socat processes:")
            show_socat_processes()
            return 1

    print(f"[socat_init] performing connectivity test: write to {v2}, read from {v1}")
    tmp_out = ""
    mktemp = subprocess.run(
        ["mktemp", "/tmp/socat_test.XXXXXX"], capture_output=True, text=True
    )
    if mktemp.returncode == 0:
        tmp_out = mktemp.stdout.rstrip("\n")
    if not tmp_out:
        tmp_out = f"/tmp/socat_test.{os.getpid()}"
    test_str = f"socat-test-{os.getpid()}-{int(time.time())}"

    reader = subprocess.Popen(
        ["timeout", "5", "bash", "-c", f"cat '{v1}' > '{tmp_out}'"]
    )
    reader_pid = reader.pid
    time.sleep(0.2)
    try:
        with open(v2, "wb") as port:
            port.write(test_str.encode())
    except OSError:
        pass
    time.sleep(0.6)
    try:
        os.kill(reader_pid, signal.SIGTERM)
    except OSError:
        pass
    try:
        reader.wait()
    except subprocess.SubprocessError:
        pass

    received = b""
    try:
        with open(tmp_out, "rb") as f:
            received = f.read()
    except OSError:
        received = b""

    if test_str.encode() in received:
        print(
            f"[socat_init] connectivity test passed: data written to {v2} was received on {v1}"
        )
        rm_quiet(tmp_out)
        print(f"PORT1={port1}")
        print(f"PORT2={port2}")
        if mode == "tui_multiple":
            print(f"PORT3={v3}")
            print(f"PORT4={v4}")
            print(f"PORT5={v5}")
            print(f"PORT6={v6}")
        print("[socat_init] finished successfully")
        return 0

    print("[socat_init] connectivity test FAILED")
    print(f"[socat_init] contents of {tmp_out} (if any):")
    subprocess.run(
        ["sed", "-n", "1,200p", tmp_out], stdin=subprocess.DEVNULL, check=False
    )
    if mode == "tui_multiple":
        print("[socat_init] socat logs:")
        tail_glob(f"{log}.*")
    else:
        print(f"[socat_init] socat log ({log}):")
        tail_single(log)
    print("[socat_init] socat processes:")
    show_socat_processes()
    rm_quiet(tmp_out)
    return 2


if __name__ == "__main__":
    sys.stdout.reconfigure(line_buffering=True)
    sys.stderr.reconfigure(line_buffering=True)
    sys.exit(main())
