# PowerShell script to setup virtual serial ports using com0com on Windows
# Enhanced version for GitHub Actions CI environment

param(
    [string]$Mode = "tui"
)

Write-Host "[com0com_init] Starting in mode: $Mode"

# 1. Check for administrator privileges (critical for driver operations)
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "[com0com_init] ERROR: This script must be run as Administrator" -ForegroundColor Red
    exit 1
}

# 2. Locate commandc.exe with improved path discovery
# Chocolatey often installs to one of these locations
$searchPaths = @(
    "$env:ChocolateyInstall\lib\com0com\tools\commandc.exe",  # Most common Chocolatey location
    "C:\ProgramData\chocolatey\lib\com0com\tools\commandc.exe", # Alternative Chocolatey path
    "C:\Program Files (x86)\com0com\commandc.exe",
    "C:\Program Files\com0com\commandc.exe"
)

$commandcPath = $null
foreach ($path in $searchPaths) {
    if (Test-Path $path) {
        $commandcPath = $path
        Write-Host "[com0com_init] Found commandc.exe at: $path" -ForegroundColor Green
        break
    }
}

# Final attempt: try to find via PATH environment variable
if (-not $commandcPath) {
    try {
        $commandInPath = Get-Command "commandc.exe" -ErrorAction Stop
        $commandcPath = $commandInPath.Source
        Write-Host "[com0com_init] Found commandc.exe in PATH: $commandcPath" -ForegroundColor Green
    } catch {
        Write-Host "[com0com_init] WARNING: commandc.exe not found via PATH" -ForegroundColor Yellow
    }
}

# If still not found, report error and exit
if (-not $commandcPath) {
    Write-Host "[com0com_init] ERROR: Could not locate commandc.exe" -ForegroundColor Red
    Write-Host "[com0com_init] Searched in the following locations:" -ForegroundColor Red
    $searchPaths | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    Write-Host "[com0com_init] Please ensure com0com is properly installed via Chocolatey" -ForegroundColor Red
    exit 1
}

# 3. Clean up any existing port pairs
Write-Host "[com0com_init] Cleaning up existing com0com pairs..." 
& $commandcPath remove 0 2>&1 | Out-Null
Start-Sleep -Seconds 1

# 4. Create the COM1<->COM2 virtual pair
Write-Host "[com0com_init] Creating COM1 <-> COM2 virtual serial port pair..."
$creationOutput = & $commandcPath install PortName=COM1 PortName=COM2 2>&1
Write-Host "[com0com_init] Creation output: $creationOutput"

# 5. Brief pause to let the system register the new ports
Write-Host "[com0com_init] Allowing system to register new ports..."
Start-Sleep -Seconds 3

# 6. Verify the port pair was created
Write-Host "[com0com_init] Verifying port pair creation..."
$pairList = & $commandcPath list 2>&1
Write-Host "[com0com_init] Current com0com pairs: $pairList"

# 7. Check what serial ports are available to the system
Write-Host "[com0com_init] Checking system serial ports..."
$availablePorts = [System.IO.Ports.SerialPort]::GetPortNames()
Write-Host "[com0com_init] Available serial ports: $($availablePorts -join ', ')"

# 8. Determine which ports to use for testing
$Port1 = $availablePorts | Where-Object { $_ -eq "COM1" } | Select-Object -First 1
$Port2 = $availablePorts | Where-Object { $_ -eq "COM2" } | Select-Object -First 1

# Fallback logic if COM1 and COM2 aren't both available
if (-not $Port1 -or -not $Port2) {
    Write-Host "[com0com_init] WARNING: COM1 and/or COM2 not available. Using fallback ports." -ForegroundColor Yellow
    # Use the first two available ports
    $Port1 = $availablePorts | Select-Object -First 1
    $Port2 = $availablePorts | Select-Object -Skip 1 -First 1
}

# Final validation
if (-not $Port1 -or -not $Port2) {
    Write-Host "[com0com_init] ERROR: Could not identify two distinct ports for testing." -ForegroundColor Red
    Write-Host "[com0com_init] Available ports were: $($availablePorts -join ', ')" -ForegroundColor Red
    exit 1
}

if ($Port1 -eq $Port2) {
    Write-Host "[com0com_init] ERROR: Both ports are the same: $Port1" -ForegroundColor Red
    exit 1
}

Write-Host "[com0com_init] SUCCESS: Using ports $Port1 and $Port2" -ForegroundColor Green

# 9. Set environment variables for the test suite
[System.Environment]::SetEnvironmentVariable("AOBATEST_PORT1", $Port1, "Process")
[System.Environment]::SetEnvironmentVariable("AOBATEST_PORT2", $Port2, "Process")
Write-Host "[com0com_init] Exported AOBATEST_PORT1=$Port1 and AOBATEST_PORT2=$Port2"

Write-Host "[com0com_init] Virtual serial port setup completed successfully!" -ForegroundColor Green
exit 0
