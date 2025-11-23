#!/usr/bin/env python3
"""
Modbus Master/Slave Communication Test Runner

This script runs both the master and slave examples simultaneously,
prefixing their output for easy identification.

Usage:
    python scripts/run_master_slave_test.py
    python scripts/run_master_slave_test.py --master-port /tmp/vcom1 --slave-port /tmp/vcom2
"""

import subprocess
import sys
import threading
import time
import argparse
from pathlib import Path


class ProcessRunner:
    """Run a process and prefix its output"""
    
    def __init__(self, name: str, cmd: list, prefix: str, color_code: str):
        self.name = name
        self.cmd = cmd
        self.prefix = prefix
        self.color_code = color_code
        self.process = None
        self.thread = None
        
    def stream_output(self, stream, is_stderr=False):
        """Stream output from process with prefix"""
        stream_name = "stderr" if is_stderr else "stdout"
        try:
            for line in iter(stream.readline, b''):
                if line:
                    decoded = line.decode('utf-8', errors='replace').rstrip()
                    print(f"{self.color_code}[{self.prefix}]{self._reset_color()} {decoded}", flush=True)
        except Exception as e:
            print(f"{self.color_code}[{self.prefix}]{self._reset_color()} Error reading {stream_name}: {e}", flush=True)
    
    def _reset_color(self):
        """Reset terminal color"""
        return "\033[0m"
    
    def start(self):
        """Start the process"""
        print(f"{self.color_code}[{self.prefix}]{self._reset_color()} Starting: {' '.join(self.cmd)}")
        self.process = subprocess.Popen(
            self.cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=1
        )
        
        # Start threads to stream stdout and stderr
        self.thread_stdout = threading.Thread(
            target=self.stream_output,
            args=(self.process.stdout, False),
            daemon=True
        )
        self.thread_stderr = threading.Thread(
            target=self.stream_output,
            args=(self.process.stderr, True),
            daemon=True
        )
        
        self.thread_stdout.start()
        self.thread_stderr.start()
        
        return self.process
    
    def wait(self):
        """Wait for process to complete"""
        if self.process:
            return self.process.wait()
    
    def terminate(self):
        """Terminate the process"""
        if self.process:
            print(f"{self.color_code}[{self.prefix}]{self._reset_color()} Terminating...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                print(f"{self.color_code}[{self.prefix}]{self._reset_color()} Force killing...")
                self.process.kill()


def main():
    parser = argparse.ArgumentParser(description='Run Modbus Master/Slave test')
    parser.add_argument('--master-port', default='/tmp/vcom1', help='Master port (default: /tmp/vcom1)')
    parser.add_argument('--slave-port', default='/tmp/vcom2', help='Slave port (default: /tmp/vcom2)')
    parser.add_argument('--duration', type=int, default=0, help='Test duration in seconds (0 = run until Ctrl+C)')
    parser.add_argument('--no-build', action='store_true', help='Skip auto-build step (use existing binaries)')
    args = parser.parse_args()
    
    # Find project root
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    
    print("=" * 80)
    print("🚀 Modbus Master/Slave Communication Test")
    print("=" * 80)
    print(f"📍 Master port: {args.master_port}")
    print(f"📍 Slave port:  {args.slave_port}")
    print(f"⏱️  Duration:    {'Unlimited (Ctrl+C to stop)' if args.duration == 0 else f'{args.duration} seconds'}")
    print("=" * 80)
    print()
    
    # ANSI color codes
    COLOR_MASTER = "\033[94m"  # Blue
    COLOR_SLAVE = "\033[92m"   # Green
    
    # Check if binaries exist, if not build them
    slave_bin = project_root / "target" / "debug" / "api_slave"
    master_bin = project_root / "target" / "debug" / "api_master"
    
    if not args.no_build and (not slave_bin.exists() or not master_bin.exists()):
        print("🔨 Building examples...")
        subprocess.run(
            ["cargo", "build", "--package", "api_master", "--package", "api_slave"],
            cwd=project_root,
            check=True
        )
        print("✅ Build completed")
        print()
    
    # Create process runners using pre-built binaries
    slave = ProcessRunner(
        name="Slave",
        cmd=[str(slave_bin), args.slave_port],
        prefix="SLAVE",
        color_code=COLOR_SLAVE
    )
    
    master = ProcessRunner(
        name="Master",
        cmd=[str(master_bin), args.master_port],
        prefix="MASTER",
        color_code=COLOR_MASTER
    )
    
    try:
        # Start slave first (it needs to be listening)
        print("📡 Starting slave (listener)...")
        slave.start()
        
        # Wait a bit for slave to initialize
        print("⏳ Waiting 3 seconds for slave to initialize...")
        time.sleep(3)
        
        # Start master
        print("📡 Starting master (poller)...")
        master.start()
        
        print()
        print("✅ Both processes started!")
        print("💡 Press Ctrl+C to stop both processes")
        print("=" * 80)
        print()
        
        # Wait for specified duration or until Ctrl+C
        if args.duration > 0:
            time.sleep(args.duration)
            print()
            print("=" * 80)
            print(f"⏰ Test duration ({args.duration}s) completed")
        else:
            # Wait indefinitely
            while True:
                time.sleep(1)
                # Check if either process has died
                if slave.process.poll() is not None:
                    print()
                    print("=" * 80)
                    print("❌ Slave process terminated unexpectedly")
                    break
                if master.process.poll() is not None:
                    print()
                    print("=" * 80)
                    print("❌ Master process terminated unexpectedly")
                    break
    
    except KeyboardInterrupt:
        print()
        print("=" * 80)
        print("🛑 Interrupted by user (Ctrl+C)")
    
    finally:
        print("=" * 80)
        print("🔄 Shutting down processes...")
        master.terminate()
        slave.terminate()
        print("✅ All processes terminated")
        print("=" * 80)


if __name__ == "__main__":
    main()
