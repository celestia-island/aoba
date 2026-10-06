#!/usr/bin/env python3
"""Local CI Test Runner.

This script runs GitHub Actions CI tests locally.
It replicates the CI environment (as much as possible) and outputs results to a specified directory
for easy debugging and inspection.

Usage:
  ./scripts/run_ci_locally.py [OPTIONS]

Options:
  --workflow <name>    Workflow to run: tui-rendering, tui-drilldown, cli, all (default: all)
  --module <name>      Specific module to test (optional, runs all if not specified)
  --output-dir <path>  Output directory for test results (default: ./ci-results)
  --help               Show this help message

Examples:
  # Run all tests
  ./scripts/run_ci_locally.py

  # Run only TUI rendering tests
  ./scripts/run_ci_locally.py --workflow tui-rendering

  # Run specific module
  ./scripts/run_ci_locally.py --workflow tui-rendering --module single_station_master_coils

  # Specify custom output directory
  ./scripts/run_ci_locally.py --output-dir /tmp/my-ci-results

Converted from scripts/run_ci_locally.sh to Python 3 on 2026-10-06 per the
workspace tool-script policy (AGENTS §7.5).
"""

from __future__ import annotations

import glob
import os
import re
import shutil
import subprocess
import sys
import time

# Color codes for output
RED = "\033[0;31m"
GREEN = "\033[0;32m"
YELLOW = "\033[1;33m"
BLUE = "\033[0;34m"
NC = "\033[0m"  # No Color

BOX_TOP = f"{BLUE}╔════════════════════════════════════════════════════════╗{NC}"
BOX_TITLE = f"{BLUE}║         Local CI Test Runner for Aoba Project          ║{NC}"
BOX_BOTTOM = f"{BLUE}╚════════════════════════════════════════════════════════╝{NC}"
DIVIDER = f"{BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{NC}"

DEFAULT_VIRTUAL_PORT_UUID = "00000000-0000-0000-0000-000000000001"

# Define test modules
TUI_RENDERING_MODULES = [
    "single_station_master_coils",
    "single_station_master_discrete_inputs",
    "single_station_master_holding",
    "single_station_master_input",
    "single_station_slave_coils",
    "single_station_slave_discrete_inputs",
    "single_station_slave_holding",
    "single_station_slave_input",
    "multi_station_master_mixed_types",
    "multi_station_master_spaced_addresses",
    "multi_station_master_mixed_ids",
    "multi_station_slave_mixed_types",
    "multi_station_slave_spaced_addresses",
    "multi_station_slave_mixed_ids",
    "data_source_http_server",
    "data_source_ipc_pipe",
    "data_source_mqtt_client",
    "data_source_port_forwarding",
    "data_source_virtual_port_slave",
    "data_source_virtual_port_ipc_master",
    "data_source_virtual_port_http_master",
    "write_single_station_slave_coils",
    "write_single_station_slave_holding",
]

TUI_DRILLDOWN_MODULES = list(TUI_RENDERING_MODULES)

CLI_MODULES = [
    "help",
    "list_ports",
    "list_ports_json",
    "list_ports_status",
    "modbus_basic_master_slave",
    "data_source_http",
    "data_source_ipc",
    "data_source_ipc_pipe",
    "data_source_virtual_port",
    "data_source_mqtt",
    "slave_write_coils",
    "slave_write_holding",
    "api_master_cli_slave",
    "api_slave_cli_master",
]


def print_help() -> None:
    """Show the usage header (mirrors the original shell --help output)."""
    lines = (__doc__ or "").splitlines()
    end = len(lines)
    for index, line in enumerate(lines):
        if line.startswith("Converted from"):
            end = index
            break
    while end > 0 and not lines[end - 1].strip():
        end -= 1
    for line in lines[:end]:
        print(line)


def is_non_negative_int(value: str) -> bool:
    return re.fullmatch(r"[0-9]+", value) is not None


class LocalCIRunner:
    """Runs the local CI test workflows, replicating the original shell runner."""

    def __init__(
        self,
        repo_root: str,
        output_dir: str,
        workflow: str,
        module: str,
        python_cmd: str,
        watchdog_helper: str,
        virtual_port_uuid: str,
        module_timeout_secs: int,
        use_timeout: bool,
        inactivity_timeout_secs: int,
        inactivity_notify_secs: int,
        tui_e2e_extra_args: str,
    ) -> None:
        self.repo_root = repo_root
        self.output_dir = output_dir
        self.workflow = workflow
        self.module = module
        self.python_cmd = python_cmd
        self.watchdog_helper = watchdog_helper
        self.virtual_port_uuid = virtual_port_uuid
        self.module_timeout_secs = module_timeout_secs
        self.use_timeout = use_timeout
        self.inactivity_timeout_secs = inactivity_timeout_secs
        self.inactivity_notify_secs = inactivity_notify_secs
        self.tui_e2e_extra_args = tui_e2e_extra_args

    # Helper: run a shell command string and append its combined stdout/stderr to a log file.
    # Returns the exit code of the command (not tee). The watchdog helper executes the string.
    def run_and_log_cmd(self, result_file: str, label: str, cmd: str) -> int:
        watchdog_args = [
            self.python_cmd,
            self.watchdog_helper,
            "--cmd",
            cmd,
            "--log-file",
            result_file,
            "--label",
            label,
            "--inactivity-timeout",
            str(self.inactivity_timeout_secs),
            "--notify-interval",
            str(self.inactivity_notify_secs),
        ]
        exit_code = subprocess.run(watchdog_args).returncode

        if exit_code == 124 and self.use_timeout:
            message = (
                f"Command timed out after {self.module_timeout_secs}s and was terminated."
            )
            print(message)
            append_line(result_file, message)
        elif exit_code == 137 and self.use_timeout:
            message = f"Command exceeded the timeout and was killed (exit {exit_code})."
            print(message)
            append_line(result_file, message)

        return exit_code

    def run_workflow_tests(self, workflow_type: str, modules: list[str]) -> int:
        modules_to_run = [self.module] if self.module else list(modules)

        total = len(modules_to_run)
        passed = 0
        failed = 0

        print(f"{YELLOW}Running locally. Will run socat_init.py before each module.{NC}")

        # Build packages once per workflow
        if workflow_type in ("tui-rendering", "tui-drilldown"):
            print("=== Building (local) packages: aoba, tui_e2e ===")
            build_exit = self.run_and_log_cmd(
                f"{self.output_dir}/{workflow_type}_build.log",
                f"build:{workflow_type}",
                f'cd "{self.repo_root}" && cargo build --package aoba --package tui_e2e',
            )
            if build_exit != 0:
                print(
                    f"{RED}Build failed for workflow {workflow_type} (exit {build_exit}). "
                    f"See {self.output_dir}/{workflow_type}_build.log{NC}"
                )
                return build_exit
            subprocess.run(
                [
                    "chmod",
                    "+x",
                    f"{self.repo_root}/target/debug/aoba",
                    f"{self.repo_root}/target/debug/tui_e2e",
                ],
                check=False,
            )
        elif workflow_type == "cli":
            print("=== Building (local) packages: aoba, cli_e2e ===")
            build_exit = self.run_and_log_cmd(
                f"{self.output_dir}/{workflow_type}_build.log",
                f"build:{workflow_type}",
                f'cd "{self.repo_root}" && cargo build --package aoba --package cli_e2e',
            )
            if build_exit != 0:
                print(
                    f"{RED}Build failed for workflow {workflow_type} (exit {build_exit}). "
                    f"See {self.output_dir}/{workflow_type}_build.log{NC}"
                )
                return build_exit
            subprocess.run(
                [
                    "chmod",
                    "+x",
                    f"{self.repo_root}/target/debug/aoba",
                    f"{self.repo_root}/target/debug/cli_e2e",
                ],
                check=False,
            )
        else:
            print(f"Unknown workflow type: {workflow_type}")
            return 2

        for module in modules_to_run:
            timestamp = time.strftime("%Y%m%d_%H%M%S")
            result_file = (
                f"{self.output_dir}/{workflow_type}_{module}_{timestamp}.log"
            )
            status_file = (
                f"{self.output_dir}/{workflow_type}_{module}_{timestamp}.status"
            )
            module_runner = ""
            if self.use_timeout:
                module_runner = f"timeout --foreground {self.module_timeout_secs}s "

            print(DIVIDER)
            print(f"{YELLOW}Running (local):{NC} {workflow_type} / {module}")
            print(DIVIDER)

            # Reset socat before each module if available
            socat_script = f"{self.repo_root}/scripts/socat_init.py"
            if os.access(socat_script, os.X_OK):
                message = f"=== Resetting virtual serial ports before module: {module} ==="
                print(message)
                append_line(result_file, message)
                self.run_and_log_cmd(
                    result_file,
                    "socat_init",
                    f'cd "{self.repo_root}" && ./scripts/socat_init.py',
                )
            else:
                message = f"Warning: {socat_script} not found or not executable"
                print(message)
                append_line(result_file, message)

            # Run module
            if workflow_type == "cli":
                cmd = (
                    f'cd "{self.repo_root}" && {module_runner}'
                    f'./target/debug/cli_e2e --module "{module}" '
                    f'--virtual-port-uuid "{self.virtual_port_uuid}"'
                )
            else:
                cmd = (
                    f'cd "{self.repo_root}" && {module_runner}'
                    f'./target/debug/tui_e2e --module "{module}" '
                    f"{self.tui_e2e_extra_args} --screen-capture-only"
                )
            exit_code = self.run_and_log_cmd(
                result_file, f"module:{workflow_type}/{module}", cmd
            )

            if exit_code == 0:
                write_file(status_file, "SUCCESS\n")
                print(f"{GREEN}✅ PASSED{NC}: {workflow_type} / {module}")
                passed += 1
            else:
                write_file(status_file, f"FAILED (exit code: {exit_code})\n")
                print(
                    f"{RED}❌ FAILED{NC}: {workflow_type} / {module} (exit code: {exit_code})"
                )
                failed += 1

            # Write a small summary
            timestamp_line = subprocess.run(
                ["date"], capture_output=True, text=True
            ).stdout.rstrip("\n")
            try:
                with open(result_file, "a", encoding="utf-8") as summary_file:
                    summary_file.write("\n")
                    summary_file.write("------\n")
                    summary_file.write("Test Summary\n")
                    summary_file.write("------\n")
                    summary_file.write(f"Workflow: {workflow_type}\n")
                    summary_file.write(f"Module: {module}\n")
                    summary_file.write(f"Exit Code: {exit_code}\n")
                    summary_file.write(f"Timestamp: {timestamp_line}\n")
                    summary_file.write(f"Log File: {result_file}\n")
                    summary_file.write("Status: ")
                    try:
                        with open(status_file, encoding="utf-8") as sf:
                            summary_file.write(sf.read())
                    except OSError:
                        pass
            except OSError:
                pass

            print(f"  Log saved to: {GREEN}{result_file}{NC}")

        print("")
        print(DIVIDER)
        print(f"{YELLOW}Summary for {workflow_type}:{NC}")
        print(f"  Total:  {total}")
        print(f"  Passed: {GREEN}{passed}{NC}")
        print(f"  Failed: {RED}{failed}{NC}")
        print(DIVIDER)
        print("")

        # run_workflow_tests completed for this workflow
        return failed

    def main(self) -> int:
        total_failed = 0

        # This runner executes tests locally by default.
        print(f"{YELLOW}Running tests locally.{NC}")
        print("")

        if self.workflow == "tui-rendering":
            total_failed = self.run_workflow_tests("tui-rendering", TUI_RENDERING_MODULES)
        elif self.workflow == "tui-drilldown":
            total_failed = self.run_workflow_tests("tui-drilldown", TUI_DRILLDOWN_MODULES)
        elif self.workflow == "cli":
            total_failed = self.run_workflow_tests("cli", CLI_MODULES)
        else:
            print(f"{YELLOW}Running all workflows...{NC}")
            print("")

            total_failed += self.run_workflow_tests(
                "tui-rendering", TUI_RENDERING_MODULES
            )
            total_failed += self.run_workflow_tests(
                "tui-drilldown", TUI_DRILLDOWN_MODULES
            )
            total_failed += self.run_workflow_tests("cli", CLI_MODULES)

        # Final summary
        print("")
        print(BOX_TOP)
        print(f"{BLUE}║                   Final Summary                        ║{NC}")
        print(BOX_BOTTOM)
        print("")
        print(f"Output directory: {GREEN}{self.output_dir}{NC}")
        print("")
        print("Test results have been saved to individual log files.")
        print("You can examine each test's output in the following format:")
        print(f"  {self.output_dir}/<workflow>_<module>_<timestamp>.log")
        print(f"  {self.output_dir}/<workflow>_<module>_<timestamp>.status")
        print("")

        if total_failed == 0:
            print(f"{GREEN}✅ All tests passed!{NC}")
            return 0

        print(f"{RED}❌ {total_failed} test(s) failed{NC}")
        print("")
        print("Failed tests:")
        for status_file in sorted(glob.glob(f"{self.output_dir}/*.status")):
            try:
                with open(status_file, encoding="utf-8") as sf:
                    content = sf.read()
            except OSError:
                continue
            if "FAILED" in content:
                log_file = status_file[: -len(".status")] + ".log"
                print(f"  {RED}❌{NC} {os.path.basename(log_file)}")
        return 1


def append_line(path: str, line: str) -> None:
    """Append a line to a log file, ignoring errors (tee -a)."""
    try:
        with open(path, "a", encoding="utf-8") as f:
            f.write(line + "\n")
    except OSError:
        pass


def write_file(path: str, content: str) -> None:
    """Write a file, ignoring errors (echo ... > file || true)."""
    try:
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
    except OSError:
        pass


def parse_args(argv: list[str]) -> tuple[str, str, str]:
    workflow = "all"
    module = ""
    output_dir = "./ci-results"

    index = 0
    while index < len(argv):
        arg = argv[index]
        if arg in ("--workflow", "--module", "--output-dir"):
            if index + 1 >= len(argv):
                print(f"{RED}Error: {arg} requires a value{NC}")
                print("Use --help for usage information")
                sys.exit(1)
            value = argv[index + 1]
            if arg == "--workflow":
                workflow = value
            elif arg == "--module":
                module = value
            else:
                output_dir = value
            index += 2
            continue
        if arg == "--help":
            print_help()
            sys.exit(0)
        print(f"{RED}Error: Unknown option {arg}{NC}")
        print("Use --help for usage information")
        sys.exit(1)

    return workflow, module, output_dir


def build_runner() -> LocalCIRunner:
    script_dir = os.path.abspath(os.path.dirname(__file__))
    repo_root = os.path.abspath(os.path.join(script_dir, ".."))

    os.environ["AOBA_VIRTUAL_PORT_UUID"] = (
        os.environ.get("AOBA_VIRTUAL_PORT_UUID") or DEFAULT_VIRTUAL_PORT_UUID
    )

    # Force English locale so UI text matches workflow expectations
    os.environ["LANGUAGE"] = "en_US"
    os.environ["LC_ALL"] = "en_US.UTF-8"
    os.environ["LANG"] = "en_US.UTF-8"

    module_timeout_raw = os.environ.get("MODULE_TIMEOUT_SECS") or "60"
    use_timeout = False
    module_timeout_secs = 0
    if not module_timeout_raw:
        module_timeout_secs = 0
    elif is_non_negative_int(module_timeout_raw):
        module_timeout_secs = int(module_timeout_raw)
        if module_timeout_secs > 0:
            if shutil.which("timeout") is None:
                print(
                    f"{RED}Error: timeout command not found but "
                    f"MODULE_TIMEOUT_SECS={module_timeout_raw}{NC}"
                )
                sys.exit(1)
            use_timeout = True
    else:
        print(
            f"{RED}Error: MODULE_TIMEOUT_SECS must be a non-negative integer "
            f"(current: {module_timeout_raw}){NC}"
        )
        sys.exit(1)

    python_cmd = os.environ.get("PYTHON_CMD") or "python3"
    watchdog_helper = os.path.join(script_dir, "run_with_watchdog.py")

    inactivity_timeout_raw = os.environ.get("INACTIVITY_TIMEOUT_SECS") or "60"
    inactivity_notify_raw = os.environ.get("INACTIVITY_NOTIFY_SECS") or "10"

    if shutil.which(python_cmd) is None:
        print(f"{RED}Error: {python_cmd} not found in PATH{NC}")
        sys.exit(1)

    if not os.path.isfile(watchdog_helper):
        print(f"{RED}Error: Watchdog helper not found at {watchdog_helper}{NC}")
        print(f"{RED}Please ensure scripts/run_with_watchdog.py exists.{NC}")
        sys.exit(1)

    for name, value in (
        ("INACTIVITY_TIMEOUT_SECS", inactivity_timeout_raw),
        ("INACTIVITY_NOTIFY_SECS", inactivity_notify_raw),
    ):
        if not is_non_negative_int(value):
            print(
                f"{RED}Error: {name} must be a non-negative integer (current: {value}){NC}"
            )
            sys.exit(1)

    # Parse command line arguments (the original validates the environment first)
    workflow, module, output_dir = parse_args(sys.argv[1:])

    # Validate workflow argument
    if not re.fullmatch(r"(tui-rendering|tui-drilldown|cli|all)", workflow):
        print(
            f"{RED}Error: Invalid workflow. Must be one of: "
            f"tui-rendering, tui-drilldown, cli, all{NC}"
        )
        sys.exit(1)

    # Create output directory
    os.makedirs(output_dir, exist_ok=True)
    output_dir = os.path.abspath(output_dir)

    # Docker support removed: this script runs tests locally only.
    tui_e2e_extra_args = os.environ.get("TUI_E2E_EXTRA_ARGS") or ""

    print(BOX_TOP)
    print(BOX_TITLE)
    print(BOX_BOTTOM)
    print("")
    print(f"{YELLOW}Configuration:{NC}")
    print(f"  Workflow:      {GREEN}{workflow}{NC}")
    print(f"  Module:        {GREEN}{module or 'all'}{NC}")
    print(f"  Output Dir:    {GREEN}{output_dir}{NC}")
    print(f"  Repo Root:     {GREEN}{repo_root}{NC}")
    print(f"  Virtual UUID:  {GREEN}{os.environ['AOBA_VIRTUAL_PORT_UUID']}{NC}")
    if use_timeout:
        print(f"  Module Timeout:{GREEN}{module_timeout_secs}s{NC}")
    else:
        print(f"  Module Timeout:{GREEN}disabled{NC}")
    inactivity_timeout_secs = int(inactivity_timeout_raw)
    inactivity_notify_secs = int(inactivity_notify_raw)
    if inactivity_timeout_secs > 0:
        print(f"  Inactivity Timeout:{GREEN}{inactivity_timeout_secs}s{NC}")
    else:
        print(f"  Inactivity Timeout:{GREEN}disabled{NC}")
    if inactivity_notify_secs > 0:
        print(f"  Inactivity Notify :{GREEN}{inactivity_notify_secs}s{NC}")
    else:
        print(f"  Inactivity Notify :{GREEN}disabled{NC}")
    print(f"  Python Runner  : {GREEN}{python_cmd}{NC}")
    print("")

    return LocalCIRunner(
        repo_root=repo_root,
        output_dir=output_dir,
        workflow=workflow,
        module=module,
        python_cmd=python_cmd,
        watchdog_helper=watchdog_helper,
        virtual_port_uuid=os.environ["AOBA_VIRTUAL_PORT_UUID"],
        module_timeout_secs=module_timeout_secs,
        use_timeout=use_timeout,
        inactivity_timeout_secs=inactivity_timeout_secs,
        inactivity_notify_secs=inactivity_notify_secs,
        tui_e2e_extra_args=tui_e2e_extra_args,
    )


def main() -> int:
    sys.stdout.reconfigure(line_buffering=True)
    sys.stderr.reconfigure(line_buffering=True)
    runner = build_runner()
    return runner.main()


if __name__ == "__main__":
    final_rc = main()
    if final_rc != 0:
        print(f"{RED}Runner exited with code {final_rc}{NC}")
    sys.exit(final_rc)
