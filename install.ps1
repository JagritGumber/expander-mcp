$ErrorActionPreference = "Stop"

$Repo = "JagritGumber/expander-mcp"
$Version = if ($env:EXPANDER_VERSION) { $env:EXPANDER_VERSION } else { "latest" }
$InstallDir = if ($env:EXPANDER_INSTALL_DIR) { $env:EXPANDER_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\expander-mcp" }
$Arch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
    "X64" { "x86_64" }
    default { throw "No Windows release is available for this architecture yet." }
}
$Asset = "expander-mcp-windows-$Arch.zip"
$Url = if ($Version -eq "latest") {
    "https://github.com/$Repo/releases/latest/download/$Asset"
} else {
    "https://github.com/$Repo/releases/download/$Version/$Asset"
}
$ChecksumsUrl = if ($Version -eq "latest") {
    "https://github.com/$Repo/releases/latest/download/SHA256SUMS"
} else {
    "https://github.com/$Repo/releases/download/$Version/SHA256SUMS"
}

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $TempDir | Out-Null
try {
    $Archive = Join-Path $TempDir $Asset
    Invoke-WebRequest -Uri $Url -OutFile $Archive
    $Checksums = (Invoke-WebRequest -Uri $ChecksumsUrl).Content
    $Expected = (($Checksums -split "`n") | Where-Object { $_ -match "\s+$([regex]::Escape($Asset))\s*$" } | Select-Object -First 1) -split "\s+" | Select-Object -First 1
    $Actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
    if (-not $Expected -or $Actual -ne $Expected.ToLowerInvariant()) {
        throw "Checksum verification failed for $Asset"
    }
    Expand-Archive -Path $Archive -DestinationPath $TempDir
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item (Join-Path $TempDir "expander-mcp.exe") (Join-Path $InstallDir "expander-mcp.exe") -Force
    & (Join-Path $InstallDir "expander-mcp.exe") setup
    Write-Host "Installed to $InstallDir"
} finally {
    Remove-Item -Recurse -Force $TempDir
}
