# PowerShell script to setup virtual serial ports using com0com on Windows
# Fixed version for GitHub Actions

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

# Find commandc.exe
$possiblePaths = @(
    "C:\Program Files (x86)\com0com\commandc.exe",
    "C:\Program Files\com0com\commandc.exe", 
    "$env:ProgramFiles\com0com\commandc.exe",
    "$env:ProgramFiles(x86)\com0com\commandc.exe",
    "$env:ChocolateyInstall\lib\com0com\tools\commandc.exe"
)

$commandcPath = $null
foreach ($path in $possiblePaths) {
    if (Test-Path $path) {
        $commandcPath = $path
        Write-Host "[com0com_init] Found commandc at: $path"
        break
    }
}

if (-not $commandcPath) {
    # Try finding in PATH
    try {
        $cmd = Get-Command "commandc.exe" -ErrorAction Stop
        $commandcPath = $cmd.Source
        Write-Host "[com0com_init] Found commandc in PATH: $commandcPath"
    } catch {
        Write-Host "[com0com_init] ERROR: Could not find commandc.exe" -ForegroundColor Red
        Write-Host "[com0com_init] Searched in: $($possiblePaths -join ', ')" -ForegroundColor Red
        exit 1
    }
}

# Clean up any existing ports first
Write-Host "[com0com_init] Cleaning up existing com0com ports..."
try {
    & $commandcPath remove 0 2>&1 | Out-Null
    & $commandcPath remove 1 2>&1 | Out-Null
    & $commandcPath remove 2 2>&1 | Out-Null
} catch {
    # Ignore errors during cleanup
}

Start-Sleep -Seconds 1

# Create COM1 <-> COM2 virtual serial port pair
Write-Host "[com0com_init] Creating COM1 <-> COM2 virtual serial port pair..."
try {
    # Method 1: Direct port assignment
    $result1 = & $commandcPath install PortName=COM1 PortName=COM2 2>&1
    Write-Host "[com0com_init] Creation output: $result1"

    # Verify the ports were created
    Start-Sleep -Seconds 2
    $portList = & $commandcPath list 2>&1
    Write-Host "[com0com_init] Current port pairs: $portList"

    # If first method failed, try alternative method
    if ($LASTEXITCODE -ne 0 -or $portList -notmatch "COM1.*COM2|COM2.*COM1") {
        Write-Host "[com0com_init] First method failed, trying alternative..."

        # Method 2: Use CNCA/CNCB naming
        & $commandcPath remove 0 2>&1 | Out-Null
        $result2 = & $commandcPath install CNCA0=COM1 CNCB0=COM2 2>&1
        Write-Host "[com0com_init] Alternative method output: $result2"

        Start-Sleep -Seconds 2
        $portList = & $commandcPath list 2>&1
        Write-Host "[com0com_init] Port pairs after alternative: $portList"
    }

} catch {
    Write-Host "[com0com_init] Error during port creation: $_" -ForegroundColor Red
    exit 1
}

# Check available ports
Write-Host "[com0com_init] Checking available serial ports..."
try {
    $availablePorts = [System.IO.Ports.SerialPort]::GetPortNames()
    Write-Host "[com0com_init] Available ports: $($availablePorts -join ', ')"

    # Look specifically for COM1 and COM2
    $Port1 = $availablePorts | Where-Object { $_ -eq "COM1" } | Select-Object -First 1
    $Port2 = $availablePorts | Where-Object { $_ -eq "COM2" } | Select-Object -First 1

    if ($Port1 -and $Port2) {
        Write-Host "[com0com_init] Successfully created COM1 and COM2" -ForegroundColor Green
    } elseif ($Port1 -or $Port2) {
        Write-Host "[com0com_init] WARNING: Only one port was created" -ForegroundColor Yellow
        # Fallback to whatever ports are available
        $Port1 = $availablePorts | Select-Object -First 1
        $Port2 = $availablePorts | Select-Object -Skip 1 -First 1
    } else {
        # If no COM1/COM2, use the first two available ports
        $Port1 = $availablePorts | Select-Object -First 1
        $Port2 = $availablePorts | Select-Object -Skip 1 -First 1
    }

    if (-not $Port1 -or -not $Port2) {
        Write-Host "[com0com_init] ERROR: Could not find two virtual ports" -ForegroundColor Red
        exit 1
    }

    if ($Port1 -eq $Port2) {
        Write-Host "[com0com_init] ERROR: Both ports are the same: $Port1" -ForegroundColor Red
        exit 1
    }

    Write-Host "[com0com_init] Using ports: $Port1 and $Port2" -ForegroundColor Green

    # Set environment variables
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
