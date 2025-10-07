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

# Check if com0com is installed
$setupcPath = "C:\Program Files (x86)\com0com\setupc.exe"
if (-not (Test-Path $setupcPath)) {
    Write-Host "[com0com_init] ERROR: com0com not found at $setupcPath" -ForegroundColor Red
    Write-Host "[com0com_init] Please install com0com first" -ForegroundColor Red
    exit 1
}

Write-Host "[com0com_init] Found com0com at $setupcPath"

# List existing ports before cleanup
Write-Host "[com0com_init] Listing existing com0com ports..."
& $setupcPath list

# Remove existing port pairs (silently ignore errors)
Write-Host "[com0com_init] Removing existing com0com port pairs..."
for ($i = 0; $i -le 5; $i++) {
    & $setupcPath remove $i 2>&1 | Out-Null
}

Start-Sleep -Seconds 2

# Install new port pair with specific port names
Write-Host "[com0com_init] Installing port pair CNCA0 <-> CNCB0 with names COM1 and COM2..."
try {
    # Install using the correct syntax: install PortName=COM1,EmuBR=yes PortName=COM2,EmuBR=yes
    # Using EmuBR=yes to emulate baud rate settings
    $output = & $setupcPath install PortName=COM1,EmuBR=yes PortName=COM2,EmuBR=yes 2>&1
    Write-Host "[com0com_init] setupc output: $output"
    
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne $null) {
        throw "setupc install failed with exit code $LASTEXITCODE"
    }
} catch {
    Write-Host "[com0com_init] ERROR: Failed to install port pair: $_" -ForegroundColor Red
    Write-Host "[com0com_init] Attempting fallback: install default port pair and check available ports..."
    
    # Try installing without specific port names as fallback
    try {
        & $setupcPath install 2>&1 | Out-Null
        Start-Sleep -Seconds 3
        
        # List what was created
        Write-Host "[com0com_init] Installed default ports, listing:"
        & $setupcPath list
    } catch {
        Write-Host "[com0com_init] ERROR: Fallback installation also failed: $_" -ForegroundColor Red
        exit 1
    }
}

Write-Host "[com0com_init] Port pair installation completed"

# List ports after installation
Write-Host "[com0com_init] Listing com0com ports after installation..."
& $setupcPath list

# Wait for ports to be available
Write-Host "[com0com_init] Waiting for ports to be ready..."
Start-Sleep -Seconds 3

# Check what ports are available
Write-Host "[com0com_init] Checking available serial ports..."
try {
    $availablePorts = [System.IO.Ports.SerialPort]::GetPortNames()
    Write-Host "[com0com_init] Available ports: $($availablePorts -join ', ')"
    
    # Determine which ports to use (prefer COM1/COM2, but use what's available)
    $Port1 = if ($availablePorts -contains "COM1") { "COM1" } else { $availablePorts | Where-Object { $_ -match "^COM\d+$" } | Select-Object -First 1 }
    $Port2 = if ($availablePorts -contains "COM2") { "COM2" } else { $availablePorts | Where-Object { $_ -match "^COM\d+$" -and $_ -ne $Port1 } | Select-Object -First 1 }
    
    if (-not $Port1 -or -not $Port2) {
        # Try CNCA/CNCB format
        $Port1 = $availablePorts | Where-Object { $_ -match "CNCA" } | Select-Object -First 1
        $Port2 = $availablePorts | Where-Object { $_ -match "CNCB" } | Select-Object -First 1
    }
    
    if (-not $Port1 -or -not $Port2) {
        Write-Host "[com0com_init] ERROR: Could not find two virtual ports" -ForegroundColor Red
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

# Simplified connectivity test (optional, non-blocking)
Write-Host "[com0com_init] Performing quick connectivity test..."
try {
    $port1Obj = New-Object System.IO.Ports.SerialPort $Port1, 9600
    $port2Obj = New-Object System.IO.Ports.SerialPort $Port2, 9600
    
    $port1Obj.ReadTimeout = 1000
    $port2Obj.WriteTimeout = 1000
    
    $port1Obj.Open()
    $port2Obj.Open()
    
    $testData = "TEST"
    $port2Obj.Write($testData)
    Start-Sleep -Milliseconds 200
    
    if ($port1Obj.BytesToRead -gt 0) {
        Write-Host "[com0com_init] Connectivity test PASSED" -ForegroundColor Green
    } else {
        Write-Host "[com0com_init] Connectivity test: no data received (ports may still work)" -ForegroundColor Yellow
    }
    
    $port1Obj.Close()
    $port2Obj.Close()
} catch {
    Write-Host "[com0com_init] Connectivity test skipped: $_" -ForegroundColor Yellow
}

Write-Host "[com0com_init] Finished successfully" -ForegroundColor Green
Write-Host "[com0com_init] Virtual serial ports are ready: $Port1 <-> $Port2"
exit 0
