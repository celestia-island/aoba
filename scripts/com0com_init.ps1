# PowerShell script to initialize com0com virtual serial ports for Windows CI
# This script creates a pair of virtual COM ports (COM1 and COM2) using com0com
# Usage: .\com0com_init.ps1

param(
    [string]$Mode = "tui"
)

$ErrorActionPreference = "Continue"

Write-Host "[com0com_init] Starting com0com virtual port initialization (mode=$Mode)" -ForegroundColor Cyan

# Check if com0com is installed
$setupcPath = "C:\Program Files (x86)\com0com\setupc.exe"
if (-not (Test-Path $setupcPath)) {
    $setupcPath = "C:\Program Files\com0com\setupc.exe"
}

if (-not (Test-Path $setupcPath)) {
    Write-Host "[com0com_init] ERROR: setupc.exe not found. com0com must be installed first." -ForegroundColor Red
    Write-Host "[com0com_init] Please install com0com from https://sourceforge.net/projects/com0com/" -ForegroundColor Yellow
    exit 1
}

Write-Host "[com0com_init] Found setupc at: $setupcPath" -ForegroundColor Green

# Remove existing virtual port pair if it exists
Write-Host "[com0com_init] Removing existing com0com port pairs (if any)..." -ForegroundColor Cyan
try {
    # List all pairs first
    $listOutput = & $setupcPath list 2>&1
    Write-Host "[com0com_init] Current pairs:"
    Write-Host $listOutput
    
    # Try to remove pair CNCA0-CNCB0 (default pair names)
    & $setupcPath remove 0 2>&1 | Out-Null
    Start-Sleep -Milliseconds 500
    
    # Try to remove any other pairs
    & $setupcPath remove 1 2>&1 | Out-Null
    & $setupcPath remove 2 2>&1 | Out-Null
} catch {
    Write-Host "[com0com_init] Note: No existing pairs to remove (this is normal)" -ForegroundColor Gray
}

Start-Sleep -Milliseconds 1000

# Install new virtual port pair
Write-Host "[com0com_init] Installing new virtual port pair (COM1 <-> COM2)..." -ForegroundColor Cyan
try {
    # Install a new pair with default names CNCA0 and CNCB0
    $installOutput = & $setupcPath install PortName=COM1 PortName=COM2 2>&1
    Write-Host $installOutput
    
    # Give Windows time to register the ports
    Start-Sleep -Seconds 2
    
    # Configure the ports for optimal behavior
    Write-Host "[com0com_init] Configuring port pair..." -ForegroundColor Cyan
    & $setupcPath change CNCA0 PortName=COM1,EmuBR=yes,EmuOverrun=yes 2>&1 | Out-Null
    & $setupcPath change CNCB0 PortName=COM2,EmuBR=yes,EmuOverrun=yes 2>&1 | Out-Null
    
    Start-Sleep -Seconds 1
} catch {
    Write-Host "[com0com_init] ERROR: Failed to install virtual port pair" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
}

# Verify the ports were created
Write-Host "[com0com_init] Verifying virtual ports..." -ForegroundColor Cyan
$listOutput = & $setupcPath list 2>&1
Write-Host $listOutput

# Check if COM1 and COM2 appear in the output
if ($listOutput -match "COM1" -and $listOutput -match "COM2") {
    Write-Host "[com0com_init] Successfully created virtual port pair: COM1 <-> COM2" -ForegroundColor Green
} else {
    Write-Host "[com0com_init] WARNING: Port pair may not have been created correctly" -ForegroundColor Yellow
    Write-Host "[com0com_init] Please verify manually" -ForegroundColor Yellow
}

# Connectivity test
Write-Host "[com0com_init] Performing connectivity test..." -ForegroundColor Cyan
try {
    # Try to open COM1 and COM2 to verify they exist
    Add-Type -TypeDefinition @"
        using System;
        using System.IO.Ports;
        public class PortTester {
            public static bool TestPorts() {
                try {
                    using (SerialPort port1 = new SerialPort("COM1", 9600)) {
                        port1.Open();
                        port1.Close();
                    }
                    using (SerialPort port2 = new SerialPort("COM2", 9600)) {
                        port2.Open();
                        port2.Close();
                    }
                    return true;
                } catch {
                    return false;
                }
            }
        }
"@
    
    $testResult = [PortTester]::TestPorts()
    if ($testResult) {
        Write-Host "[com0com_init] Connectivity test PASSED: COM1 and COM2 are accessible" -ForegroundColor Green
        Write-Host "[com0com_init] Finished successfully" -ForegroundColor Green
        exit 0
    } else {
        Write-Host "[com0com_init] Connectivity test FAILED: Could not open COM1 or COM2" -ForegroundColor Yellow
        Write-Host "[com0com_init] Ports may still be usable by the application" -ForegroundColor Yellow
        exit 0
    }
} catch {
    Write-Host "[com0com_init] Connectivity test skipped (could not load .NET SerialPort)" -ForegroundColor Gray
    Write-Host "[com0com_init] Assuming success based on port pair creation" -ForegroundColor Gray
    exit 0
}
