<#
.SYNOPSIS
    Build Summit installers for ARM64 and/or x64.

.PARAMETER Target
    Which architecture to build: arm64, x64, or both (default: both).

.EXAMPLE
    .\build.ps1
    .\build.ps1 -Target arm64
    .\build.ps1 -Target x64
#>

param(
    [ValidateSet("arm64", "x64", "both")]
    [string]$Target = "both"
)

$ErrorActionPreference = "Stop"

# ── Paths ─────────────────────────────────────────────────────────────────────

$MSVC  = "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717"
$WDK   = "C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0"

$LIB_ARM64 = "$MSVC\lib\arm64;$WDK\ucrt\arm64;$WDK\um\arm64"
$LIB_X64   = "$MSVC\lib\x64;$WDK\ucrt\x64;$WDK\um\x64"

$STAGING = "src-tauri\shell-ext\target\current"

# ── Helpers ───────────────────────────────────────────────────────────────────

function Step([string]$msg) {
    Write-Host "`n>>> $msg" -ForegroundColor Cyan
}

function Die([string]$msg) {
    Write-Host "`nERROR: $msg" -ForegroundColor Red
    exit 1
}

# ── Sparse MSIX package (provides package identity for cloud-files API) ───────

Step "Building sparse MSIX package"

$MakeAppx = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
    "${env:ProgramFiles}\Windows Kits\10\bin"
) | ForEach-Object {
    Get-ChildItem $_ -Recurse -Filter "MakeAppx.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1
} | Select-Object -First 1 -ExpandProperty FullName

$SignTool = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
    "${env:ProgramFiles}\Windows Kits\10\bin"
) | ForEach-Object {
    Get-ChildItem $_ -Recurse -Filter "SignTool.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1
} | Select-Object -First 1 -ExpandProperty FullName

if (-not $MakeAppx -or -not $SignTool) {
    Die "MakeAppx.exe or SignTool.exe not found. Install the Windows SDK."
}

$MsixWork  = Join-Path $env:TEMP "SummitMsix"
$MsixOut   = "src-tauri\resources\sparse.msix"
$CertOut   = "src-tauri\resources\Summit.cer"
$PfxPath   = Join-Path $MsixWork "Summit.pfx"
$PfxPass   = "SummitBuild"
$Publisher = "CN=Summit"

if (Test-Path $MsixWork) { Remove-Item $MsixWork -Recurse -Force }
New-Item -ItemType Directory -Path $MsixWork | Out-Null
New-Item -ItemType Directory -Path (Join-Path $MsixWork "Assets") | Out-Null

# Minimal valid PNG for logos.
$Png = [Convert]::FromBase64String("iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAD0lEQVQI12NgYGD4TwABAAQAAeJQQbsAAAAASUVORK5CYII=")
foreach ($logo in @("StoreLogo.png","Square150x150Logo.png","Square44x44Logo.png","Square71x71Logo.png")) {
    [IO.File]::WriteAllBytes((Join-Path $MsixWork "Assets\$logo"), $Png)
}

$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap10 rescap">
  <Identity Name="Summit" Publisher="$Publisher" Version="1.0.0.0" ProcessorArchitecture="neutral" />
  <Properties>
    <DisplayName>Summit</DisplayName>
    <PublisherDisplayName>Summit</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17134.0" MaxVersionTested="10.0.22621.0" />
  </Dependencies>
  <Resources><Resource Language="en-us" /></Resources>
  <Applications>
    <Application Id="App" Executable="tauri-app.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="Summit" Description="Summit"
        BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
</Package>
"@
# Write without BOM — MakeAppx rejects UTF-8 with BOM.
[System.IO.File]::WriteAllText(
    (Join-Path $MsixWork "AppxManifest.xml"),
    $manifest,
    (New-Object System.Text.UTF8Encoding $false)
)

# Self-signed cert for the MSIX.
$cert = New-SelfSignedCertificate -Subject $Publisher -CertStoreLocation "Cert:\CurrentUser\My" `
    -KeyUsage DigitalSignature -Type CodeSigningCert -NotAfter (Get-Date).AddYears(10)
$secure = ConvertTo-SecureString $PfxPass -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath $PfxPath -Password $secure | Out-Null
# Export as .cer (public key only) — bundled with installer so it can be trusted on target machine.
Export-Certificate -Cert $cert -FilePath $CertOut -Type CERT | Out-Null

& $MakeAppx pack /d $MsixWork /p (Join-Path $MsixWork "sparse_unsigned.msix") /nv /o
if ($LASTEXITCODE -ne 0) { Die "MakeAppx failed" }
& $SignTool sign /fd SHA256 /p $PfxPass /f $PfxPath (Join-Path $MsixWork "sparse_unsigned.msix")
if ($LASTEXITCODE -ne 0) { Die "SignTool failed" }
Copy-Item (Join-Path $MsixWork "sparse_unsigned.msix") $MsixOut -Force
Write-Host "Sparse MSIX: $MsixOut"
Write-Host "Certificate: $CertOut"

# ── Frontend build (once) ─────────────────────────────────────────────────────

Step "Building frontend"
npm run build
if ($LASTEXITCODE -ne 0) { Die "Frontend build failed" }

# ── Per-architecture build ────────────────────────────────────────────────────

function Build-Arch([string]$RustTarget, [string]$LibPath) {
    Step "Building shell extension for $RustTarget"
    $env:LIB = $LibPath
    cargo build --release --target $RustTarget `
        --manifest-path "src-tauri\shell-ext\Cargo.toml"
    if ($LASTEXITCODE -ne 0) { Die "Shell extension build failed for $RustTarget" }

    # Stage the DLL to a neutral location so tauri.conf.json can reference it
    # regardless of which architecture is being built.
    New-Item -ItemType Directory -Force -Path $STAGING | Out-Null
    $src = "src-tauri\shell-ext\target\$RustTarget\release\summit_shell_ext.dll"
    $dst = "$STAGING\summit_shell_ext.dll"
    Copy-Item $src $dst -Force
    Step "Staged DLL: $src → $dst"

    Step "Building Tauri app for $RustTarget"
    $env:LIB = $LibPath
    npx tauri build --target $RustTarget
    if ($LASTEXITCODE -ne 0) { Die "Tauri build failed for $RustTarget" }
}

if ($Target -eq "arm64" -or $Target -eq "both") {
    Build-Arch "aarch64-pc-windows-msvc" $LIB_ARM64
}

if ($Target -eq "x64" -or $Target -eq "both") {
    Build-Arch "x86_64-pc-windows-msvc" $LIB_X64
}

# ── Summary ───────────────────────────────────────────────────────────────────

Step "Build complete. Installers:"
Get-ChildItem "src-tauri\target\*\release\bundle\nsis\*.exe" -ErrorAction SilentlyContinue |
    ForEach-Object { Write-Host "  $($_.FullName)" -ForegroundColor Green }
