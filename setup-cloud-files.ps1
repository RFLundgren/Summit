#Requires -Version 5.1
<#
.SYNOPSIS
    Installs a sparse MSIX package that gives Summit package identity,
    enabling StorageProviderSyncRootManager::Register and the "Free up space"
    context menu item for cloud placeholder files.

.DESCRIPTION
    Run this ONCE after cloning the repo (or whenever you change the app's
    install location).  It creates a self-signed code-signing certificate,
    builds a minimal sparse MSIX, and registers it pointing at the Tauri dev
    output directory.

    Re-run any time you move the project folder.

.NOTES
    Requires:
      - MakeAppx.exe and SignTool.exe  (Windows SDK / Visual Studio)
      - PowerShell 5.1+
      - Administrator rights (to install the root certificate once)
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# --- Configuration -----------------------------------------------------------

# Directory that contains tauri-app.exe at runtime.
# For "tauri dev" this is the Tauri debug output; adjust if needed.
$AppDir = Join-Path $PSScriptRoot "src-tauri\target\release"

$PackageName    = "Summit"
$PackageVersion = "1.0.0.0"
$Publisher      = "CN=Summit"
$DisplayName    = "Summit"
$WorkDir        = Join-Path $env:TEMP "SummitSparse"
$MsixPath       = Join-Path $WorkDir "sparse.msix"
$CertPath       = Join-Path $WorkDir "Summit.pfx"
$CertPassword   = "ImmichDev"

# --- Find SDK tools ----------------------------------------------------------

function Find-SdkTool {
    param([string]$Name)
    $sdkRoots = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "${env:ProgramFiles}\Windows Kits\10\bin"
    )
    foreach ($root in $sdkRoots) {
        if (Test-Path $root) {
            $tool = Get-ChildItem -Path $root -Recurse -Filter $Name -ErrorAction SilentlyContinue |
                    Sort-Object FullName -Descending | Select-Object -First 1
            if ($tool) { return $tool.FullName }
        }
    }
    $inPath = Get-Command $Name -ErrorAction SilentlyContinue
    if ($inPath) { return $inPath.Source }
    return $null
}

$MakeAppx = Find-SdkTool "MakeAppx.exe"
$SignTool  = Find-SdkTool "SignTool.exe"

if (-not $MakeAppx) {
    Write-Error "MakeAppx.exe not found. Install the Windows SDK or Visual Studio."
}
if (-not $SignTool) {
    Write-Error "SignTool.exe not found. Install the Windows SDK or Visual Studio."
}

Write-Host "MakeAppx : $MakeAppx"
Write-Host "SignTool : $SignTool"

# --- Prepare working directory -----------------------------------------------

if (Test-Path $WorkDir) { Remove-Item $WorkDir -Recurse -Force }
New-Item -ItemType Directory -Path $WorkDir | Out-Null
New-Item -ItemType Directory -Path (Join-Path $WorkDir "Assets") | Out-Null

# --- Create placeholder logo PNGs --------------------------------------------
# Minimal valid 4x4 blue PNG (base64). MakeAppx /nv skips image validation.

$PngBase64 = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAD0lEQVQI12NgYGD4TwABAAQAAeJQQbsAAAAASUVORK5CYII="
$PngBytes  = [Convert]::FromBase64String($PngBase64)

foreach ($logo in @("StoreLogo.png","Square150x150Logo.png","Square44x44Logo.png","Square71x71Logo.png")) {
    [IO.File]::WriteAllBytes((Join-Path $WorkDir "Assets\$logo"), $PngBytes)
}

# --- Write AppxManifest.xml --------------------------------------------------

$Manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap10 rescap">

  <Identity
    Name="$PackageName"
    Publisher="$Publisher"
    Version="$PackageVersion"
    ProcessorArchitecture="x64" />

  <Properties>
    <DisplayName>$DisplayName</DisplayName>
    <PublisherDisplayName>$DisplayName</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
    <uap10:AllowExternalContent>true</uap10:AllowExternalContent>
  </Properties>

  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop"
                        MinVersion="10.0.17134.0"
                        MaxVersionTested="10.0.22621.0" />
  </Dependencies>

  <Resources>
    <Resource Language="en-us" />
  </Resources>

  <Applications>
    <Application Id="App"
                 Executable="tauri-app.exe"
                 EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="$DisplayName"
        Description="Summit Sync"
        BackgroundColor="transparent"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png" />
      <Extensions>
        <uap:Extension Category="windows.cloudFiles">
          <uap:CloudFiles>
            <uap:SyncRoot Id="Summit">
              <uap:DisplayNameResource>Summit</uap:DisplayNameResource>
              <uap:IconResource>Assets\StoreLogo.png</uap:IconResource>
              <uap:HydrationPolicy Primary="Full" Modifier="AutoDehydrationAllowed" />
              <uap:PopulationPolicy Primary="Full" />
              <uap:SupportedFileOperations />
            </uap:SyncRoot>
          </uap:CloudFiles>
        </uap:Extension>
      </Extensions>
    </Application>
  </Applications>

  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"@

$Manifest | Set-Content -Path (Join-Path $WorkDir "AppxManifest.xml") -Encoding UTF8

# --- Create self-signed certificate ------------------------------------------

Write-Host "`nCreating self-signed certificate..."

$Cert = New-SelfSignedCertificate `
    -Subject $Publisher `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -KeyUsage DigitalSignature `
    -Type CodeSigningCert `
    -NotAfter (Get-Date).AddYears(10)

$SecurePassword = ConvertTo-SecureString -String $CertPassword -Force -AsPlainText
Export-PfxCertificate -Cert $Cert -FilePath $CertPath -Password $SecurePassword | Out-Null

# Install certificate to Trusted Root (requires elevation).
$IsAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if ($IsAdmin) {
    $store = New-Object System.Security.Cryptography.X509Certificates.X509Store(
        [System.Security.Cryptography.X509Certificates.StoreName]::Root,
        [System.Security.Cryptography.X509Certificates.StoreLocation]::LocalMachine)
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    $store.Add($Cert)
    $store.Close()
    Write-Host "Certificate installed to LocalMachine\Root."
} else {
    Write-Warning "Not running as Administrator - installing to CurrentUser\Root instead."
    Write-Warning "If installation fails with a trust error, re-run as Administrator."
    $store = New-Object System.Security.Cryptography.X509Certificates.X509Store(
        [System.Security.Cryptography.X509Certificates.StoreName]::Root,
        [System.Security.Cryptography.X509Certificates.StoreLocation]::CurrentUser)
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    $store.Add($Cert)
    $store.Close()
    Write-Host "Certificate installed to CurrentUser\Root."
}

# --- Pack the sparse MSIX ----------------------------------------------------

Write-Host "`nPacking sparse MSIX..."
& $MakeAppx pack /d $WorkDir /p $MsixPath /nv /o | Out-Host
if ($LASTEXITCODE -ne 0) { Write-Error "MakeAppx failed." }

# --- Sign the MSIX -----------------------------------------------------------

Write-Host "`nSigning MSIX..."
& $SignTool sign /fd SHA256 /p $CertPassword /f $CertPath $MsixPath | Out-Host
if ($LASTEXITCODE -ne 0) { Write-Error "SignTool failed." }

# --- Register the sparse package ---------------------------------------------

Write-Host "`nRegistering sparse package with external location: $AppDir"

$existing = Get-AppxPackage -Name $PackageName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "Removing existing package $($existing.PackageFullName)..."
    Remove-AppxPackage -Package $existing.PackageFullName
}

if (-not (Test-Path $AppDir)) {
    Write-Warning "App directory '$AppDir' does not exist yet."
    Write-Warning "Run 'cargo build' first, then re-run this script, or adjust the AppDir variable at the top."
}

Add-AppxPackage -Path $MsixPath -ExternalLocation $AppDir -ForceApplicationShutdown

# --- Report results ----------------------------------------------------------

$pkg = Get-AppxPackage -Name $PackageName -ErrorAction SilentlyContinue
if ($pkg) {
    $pfn = $pkg.PackageFamilyName
    Write-Host ""
    Write-Host "SUCCESS" -ForegroundColor Green
    Write-Host "  Package Family Name : $pfn"
    Write-Host ""
    Write-Host "The next time you launch Summit (tauri dev or the built binary)"
    Write-Host "it will have package identity and 'Free up space' will appear on hydrated"
    Write-Host "placeholder files after the first sync."
    Write-Host ""
    Write-Host "If you move the project folder, re-run this script."
} else {
    Write-Error "Package registration failed - check the errors above."
}
