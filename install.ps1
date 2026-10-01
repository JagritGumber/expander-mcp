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

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $TempDir | Out-Null
try {
    $Archive = Join-Path $TempDir $Asset
    Invoke-WebRequest -Uri $Url -OutFile $Archive
    Expand-Archive -Path $Archive -DestinationPath $TempDir
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item (Join-Path $TempDir "expander-mcp.exe") (Join-Path $InstallDir "expander-mcp.exe") -Force
    & (Join-Path $InstallDir "expander-mcp.exe") setup
    Write-Host "Installed to $InstallDir"
} finally {
    Remove-Item -Recurse -Force $TempDir
}
