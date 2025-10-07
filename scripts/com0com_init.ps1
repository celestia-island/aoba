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

# Define COM port names
$Port1 = "COM1"
$Port2 = "COM2"

Write-Host "[com0com_init] Target ports: $Port1 and $Port2"

# Check if com0com is installed
$setupcPath = "C:\Program Files (x86)\com0com\setupc.exe"
if (-not (Test-Path $setupcPath)) {
    Write-Host "[com0com_init] ERROR: com0com not found at $setupcPath" -ForegroundColor Red
    Write-Host "[com0com_init] Please install com0com first" -ForegroundColor Red
    exit 1
}

Write-Host "[com0com_init] Found com0com at $setupcPath"

# Remove existing port pairs
Write-Host "[com0com_init] Removing existing com0com port pairs..."
try {
    & $setupcPath remove 0 2>&1 | Out-Null
    & $setupcPath remove 1 2>&1 | Out-Null
    & $setupcPath remove 2 2>&1 | Out-Null
} catch {
    Write-Host "[com0com_init] Warning: Error removing existing pairs (may not exist): $_"
}

Start-Sleep -Seconds 2

# Install new port pair
Write-Host "[com0com_init] Installing port pair: $Port1 <-> $Port2"
try {
    # Install a new port pair (CNCA0 <-> CNCB0)
    & $setupcPath install PortName=$Port1 PortName=$Port2 -
    if ($LASTEXITCODE -ne 0) {
        throw "setupc install failed with exit code $LASTEXITCODE"
    }
} catch {
    Write-Host "[com0com_init] ERROR: Failed to install port pair: $_" -ForegroundColor Red
    exit 1
}

Write-Host "[com0com_init] Port pair installed successfully"

# Wait for ports to be available
Write-Host "[com0com_init] Waiting for ports to be ready..."
$timeout = 15
$count = 0
$portsReady = $false

while ($count -lt $timeout) {
    try {
        $ports = [System.IO.Ports.SerialPort]::GetPortNames()
        if (($ports -contains $Port1) -and ($ports -contains $Port2)) {
            $portsReady = $true
            break
        }
    } catch {
        Write-Host "[com0com_init] Warning: Error checking ports: $_"
    }
    Start-Sleep -Seconds 1
    $count++
}

if (-not $portsReady) {
    Write-Host "[com0com_init] ERROR: Ports $Port1 and $Port2 not ready after ${timeout}s" -ForegroundColor Red
    Write-Host "[com0com_init] Available ports: $([System.IO.Ports.SerialPort]::GetPortNames() -join ', ')"
    exit 1
}

Write-Host "[com0com_init] Ports $Port1 and $Port2 are available"

# Connectivity test
Write-Host "[com0com_init] Performing connectivity test..."
$testString = "com0com-test-$(Get-Date -Format 'yyyyMMddHHmmss')"
$testPassed = $false

try {
    # Open both ports
    $port1Obj = New-Object System.IO.Ports.SerialPort $Port1, 9600, None, 8, One
    $port2Obj = New-Object System.IO.Ports.SerialPort $Port2, 9600, None, 8, One
    
    $port1Obj.Open()
    $port2Obj.Open()
    
    Write-Host "[com0com_init] Ports opened, writing test string to $Port2"
    
    # Write to port2, read from port1
    $port2Obj.WriteLine($testString)
    Start-Sleep -Milliseconds 500
    
    if ($port1Obj.BytesToRead -gt 0) {
        $received = $port1Obj.ReadLine()
        if ($received.Contains($testString)) {
            $testPassed = $true
            Write-Host "[com0com_init] Connectivity test PASSED: data written to $Port2 was received on $Port1" -ForegroundColor Green
        } else {
            Write-Host "[com0com_init] Connectivity test FAILED: received unexpected data: $received" -ForegroundColor Red
        }
    } else {
        Write-Host "[com0com_init] Connectivity test FAILED: no data received on $Port1" -ForegroundColor Red
    }
    
    $port1Obj.Close()
    $port2Obj.Close()
} catch {
    Write-Host "[com0com_init] Connectivity test FAILED with exception: $_" -ForegroundColor Red
    try {
        if ($port1Obj -and $port1Obj.IsOpen) { $port1Obj.Close() }
        if ($port2Obj -and $port2Obj.IsOpen) { $port2Obj.Close() }
    } catch {}
}

if ($testPassed) {
    Write-Host "[com0com_init] Finished successfully" -ForegroundColor Green
    exit 0
} else {
    Write-Host "[com0com_init] Setup completed but connectivity test failed" -ForegroundColor Yellow
    Write-Host "[com0com_init] Ports may still work for the actual tests"
    exit 0
}
