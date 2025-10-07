# PowerShell script to setup virtual serial ports using com0com on Windows
# This is the Windows equivalent of socat_init.sh

param(
    [string]$Mode = "tui"
)

Write-Host "[com0com_init] mode=$Mode"

# Check if running with administrator privileges
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "[com0com_init] ERROR: This script must be run as Administrator" -ForegroundColor Red
    exit 1
}

# Check if com0com is installed via Chocolatey
$commandcPath = "commandc.exe"
if (-not (Get-Command $commandcPath -ErrorAction SilentlyContinue)) {
    Write-Host "[com0com_init] ERROR: commandc.exe not found in PATH" -ForegroundColor Red
    Write-Host "[com0com_init] Please ensure com0com is installed via Chocolatey" -ForegroundColor Red
    exit 1
}

Write-Host "[com0com_init] Found commandc.exe"

# List existing ports before cleanup
Write-Host "[com0com_init] Listing existing com0com ports..."
& $commandcPath list

# Remove existing port pairs using commandc (more reliable in CI)
Write-Host "[com0com_init] Removing existing com0com port pairs..."
try {
    # Get existing pairs and remove them properly
    $pairs = & $commandcPath list | Where-Object { $_ -match "CNCA" }
    foreach ($pair in $pairs) {
        if ($pair -match "CNCA(\d+).*CNCB(\d+)") {
            $pairNumber = $matches[1]
            Write-Host "[com0com_init] Removing pair $pairNumber"
            & $commandcPath remove $pairNumber 2>&1 | Out-Null
        }
    }
} catch {
    Write-Host "[com0com_init] Note: No existing pairs to remove or error during cleanup: $_" -ForegroundColor Yellow
}

Start-Sleep -Seconds 1

# Create COM1 <-> COM2 virtual serial port pair using commandc
Write-Host "[com0com_init] Creating COM1 <-> COM2 virtual serial port pair..."
try {
    # Use commandc instead of setupc - this is non-blocking
    $output = & $commandcPath install PortName=COM1 PortName=COM2 2>&1
    Write-Host "[com0com_init] commandc output: $output"

    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne $null) {
        Write-Host "[com0com_init] Warning: commandc returned exit code $LASTEXITCODE" -ForegroundColor Yellow
        # Try alternative approach if first method fails
        Write-Host "[com0com_init] Trying alternative port creation method..."
        & $commandcPath install CNCA0=COM1 CNCB0=COM2 2>&1 | Out-Null
    }

    Write-Host "[com0com_init] Port pair creation command completed" -ForegroundColor Green
} catch {
    Write-Host "[com0com_init] Error during port creation: $_" -ForegroundColor Yellow
    Write-Host "[com0com_init] Continuing with port detection..."
}

Write-Host "[com0com_init] Port pair installation completed"

# Wait for system to process the installation
Write-Host "[com0com_init] Waiting for system to register ports..."
Start-Sleep -Seconds 3

# List ports after installation
Write-Host "[com0com_init] Listing com0com ports after installation..."
& $commandcPath list

# Check what ports are available
Write-Host "[com0com_init] Checking available serial ports..."
try {
    $availablePorts = [System.IO.Ports.SerialPort]::GetPortNames()
    Write-Host "[com0com_init] Available ports: $($availablePorts -join ', ')"

    # Look for COM1 and COM2 specifically
    $Port1 = $availablePorts | Where-Object { $_ -eq "COM1" } | Select-Object -First 1
    $Port2 = $availablePorts | Where-Object { $_ -eq "COM2" } | Select-Object -First 1

    # If COM1/COM2 not found, look for CNCA/CNCB ports
    if (-not $Port1 -or -not $Port2) {
        $Port1 = $availablePorts | Where-Object { $_ -match "CNCA" } | Select-Object -First 1
        $Port2 = $availablePorts | Where-Object { $_ -match "CNCB" } | Select-Object -First 1
    }

    # Fallback to any two available ports
    if (-not $Port1 -or -not $Port2) {
        $Port1 = $availablePorts | Select-Object -First 1
        $Port2 = $availablePorts | Select-Object -Skip 1 -First 1
    }

    if (-not $Port1 -or -not $Port2) {
        Write-Host "[com0com_init] ERROR: Could not find two virtual ports" -ForegroundColor Red
        Write-Host "[com0com_init] Available ports were: $($availablePorts -join ', ')"
        exit 1
    }

    if ($Port1 -eq $Port2) {
        Write-Host "[com0com_init] ERROR: Both ports are the same: $Port1" -ForegroundColor Red
        Write-Host "[com0com_init] Available ports were: $($availablePorts -join ', ')"
        exit 1
    }

    Write-Host "[com0com_init] Using ports: $Port1 and $Port2" -ForegroundColor Green

    # Set environment variables for the tests to use
    [System.Environment]::SetEnvironmentVariable("AOBATEST_PORT1", $Port1, "Process")
    [System.Environment]::SetEnvironmentVariable("AOBATEST_PORT2", $Port2, "Process")
    Write-Host "[com0com_init] Set AOBATEST_PORT1=$Port1 and AOBATEST_PORT2=$Port2"

} catch {
    Write-Host "[com0com_init] ERROR: Failed to enumerate ports: $_" -ForegroundColor Red
    exit 1
}

Write-Host "[com0com_init] Finished successfully" -ForegroundColor Green
Write-Host "[com0com_init] Virtual serial ports are ready: $Port1 <-> $Port2"
exit 0