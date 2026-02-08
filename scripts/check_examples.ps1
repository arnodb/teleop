$ErrorActionPreference = "Stop"

cargo build --examples

$env:PID_FILE = [System.IO.Path]::GetTempFileName()

try {
    Start-Process cargo -ArgumentList "run --example server" -NoNewWindow

    Start-Sleep -Seconds 2

    $ServerPid = Get-Content -Path $env:PID_FILE -Raw | ForEach-Object { $_.Trim() }

    if (-not $ServerPid) {
        Write-Error "Cannot find PID in $env:PID_FILE"
        exit 1
    }

    foreach ($i in 1..10) {
        cargo run --example client $ServerPid
    }
}
finally {
    Remove-Item $env:PID_FILE
}
