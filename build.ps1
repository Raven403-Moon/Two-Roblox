<#
.SYNOPSIS
    Two-Roblox Rust Automated Build and Packaging Script

.DESCRIPTION
    Compiles release binary, creates UPX portable binary (Two-Roblox-Portable.exe),
    Inno Setup installer (Setup.exe), clean source code zip,
    and performs automated GPG signing and SHA-256 checksum generation.

.PARAMETER Upx
    Compress binary with UPX to produce standalone Two-Roblox-Portable.exe.

.PARAMETER UpxPath
    Explicit path to upx.exe binary.

.PARAMETER Setup
    Build Inno Setup installer (enabled by default).

.PARAMETER SourceZip
    Package clean source zip archive (enabled by default).

.PARAMETER GpgSign
    Perform detached GPG digital signature (.asc). Automatically detects registered key.

.PARAMETER GpgKey
    Specific GPG key ID or email to sign with (optional, auto-detects if omitted).

.PARAMETER All
    Execute full pipeline: Build, UPX Portable, Setup Installer, Source Zip, GPG Sign.

.PARAMETER Clean
    Clean target and dist directories before build.

.PARAMETER NonInteractive
    Skip interactive prompts and use provided parameters.
#>

[CmdletBinding()]
param(
    [switch]$Upx,
    [string]$UpxPath,
    [switch]$Setup,
    [switch]$SourceZip,
    [switch]$GpgSign,
    [string]$GpgKey,
    [switch]$All,
    [switch]$Clean,
    [switch]$NonInteractive
)

Set-StrictMode -Off
$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir

# Output formatting helpers (ASCII pure text, no emoji)
function Write-Header($text) {
    Write-Host ""
    Write-Host "======================================================" -ForegroundColor Cyan
    Write-Host "  $text" -ForegroundColor Cyan
    Write-Host "======================================================" -ForegroundColor Cyan
}

function Write-Info($text) {
    Write-Host "[INFO] $text" -ForegroundColor Green
}

function Write-Warn($text) {
    Write-Host "[WARN] $text" -ForegroundColor Yellow
}

function Write-Err($text) {
    Write-Host "[ERROR] $text" -ForegroundColor Red
}

function Write-Step($title) {
    Write-Host ""
    Write-Host ">> $title" -ForegroundColor Magenta
}

Write-Header "Two-Roblox Automated Build System"

# Extract version from Cargo.toml
$CargoTomlPath = Join-Path $ScriptDir "Cargo.toml"
if (-not (Test-Path $CargoTomlPath)) {
    Write-Err "Cargo.toml not found: $CargoTomlPath"
    exit 1
}

$Version = "0.1.0"
$CargoContent = Get-Content $CargoTomlPath -Raw
if ($CargoContent -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
}
Write-Info "Project Version: v$Version"

# Helper to find tools in PATH or candidate directories
function Find-Tool($toolName, $candidatePaths) {
    $cmd = Get-Command $toolName -ErrorAction SilentlyContinue
    if ($cmd) {
        return $cmd.Source
    }
    foreach ($p in $candidatePaths) {
        if ($p -and (Test-Path $p)) {
            return $p
        }
    }
    return $null
}

# Search candidate tool locations
$detectedUpx = Find-Tool "upx" @(
    (Join-Path $ScriptDir "tools\upx.exe"),
    (Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages\UPX.UPX_Microsoft.Winget.Source_8wekyb3d8bbwe\upx-5.3.0-win64\upx.exe"),
    "C:\ProgramData\chocolatey\bin\upx.exe"
)

$IsccPath = Find-Tool "iscc" @(
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)

$GpgPath = Find-Tool "gpg" @(
    "C:\Program Files\GnuPG\bin\gpg.exe",
    "C:\Program Files (x86)\GnuPG\bin\gpg.exe",
    "C:\Program Files\Gpg4win\bin\gpg.exe",
    "C:\Program Files\Git\usr\bin\gpg.exe",
    "C:\Program Files (x86)\Git\usr\bin\gpg.exe",
    (Join-Path $env:LOCALAPPDATA "Programs\Git\usr\bin\gpg.exe")
)

# Interactive mode handling
$IsInteractive = (-not $NonInteractive) -and ($PSBoundParameters.Count -eq 0)

if ($IsInteractive) {
    Write-Host ""
    Write-Host "[Build Options Selection]" -ForegroundColor White
    
    # 1. UPX executable path input for Portable standalone build
    $upxPromptHint = if ($detectedUpx) { " [Detected: $detectedUpx]" } else { " [Skip: Enter]" }
    $inputUpx = Read-Host "1. Enter UPX.exe path (for Portable standalone build)$upxPromptHint "
    if (-not [string]::IsNullOrWhiteSpace($inputUpx)) {
        $cleanPath = $inputUpx.Trim().Trim('"').Trim("'")
        if (Test-Path $cleanPath) {
            $UpxPath = (Resolve-Path $cleanPath).Path
            $Upx = $true
            Write-Info "Using specified UPX binary: $UpxPath"
        } else {
            Write-Warn "Specified UPX path not found: $cleanPath. Skipping portable compression."
            $Upx = $false
        }
    } elseif ($detectedUpx) {
        $UpxPath = $detectedUpx
        $Upx = $true
        Write-Info "Using auto-detected UPX binary: $UpxPath"
    } else {
        $Upx = $false
    }

    # 2. Inno Setup installer creation (Default: Y)
    $ansSetup = Read-Host "2. Create Inno Setup installer? (Setup.exe) (Y/n) [Default: Y] "
    if ([string]::IsNullOrWhiteSpace($ansSetup) -or ($ansSetup -notmatch '^(n|no)$')) {
        $Setup = $true
    } else {
        $Setup = $false
    }

    # 3. Clean source zip archive packaging (Default: Y)
    $ansZip = Read-Host "3. Package clean source zip archive? (Source.zip) (Y/n) [Default: Y] "
    if ([string]::IsNullOrWhiteSpace($ansZip) -or ($ansZip -notmatch '^(n|no)$')) {
        $SourceZip = $true
    } else {
        $SourceZip = $false
    }

    # 4. GPG digital signing (Default: N)
    $ansGpg = Read-Host "4. Sign distribution files with GPG? (y/N) [Default: N] "
    if ($ansGpg -match '^(y|yes)$') {
        $GpgSign = $true
    }
} elseif ($All) {
    $Upx = $true
    if (-not $UpxPath -and $detectedUpx) { $UpxPath = $detectedUpx }
    $Setup = $true
    $SourceZip = $true
    $GpgSign = $true
} else {
    # In non-interactive mode, default Setup and SourceZip to true if not specified
    if (-not $PSBoundParameters.ContainsKey('Setup')) { $Setup = $true }
    if (-not $PSBoundParameters.ContainsKey('SourceZip')) { $SourceZip = $true }
    if ($Upx -and -not $UpxPath -and $detectedUpx) { $UpxPath = $detectedUpx }
}

# Verify tool availability
if ($Upx -and -not $UpxPath) {
    Write-Warn "UPX binary not found. Skipping portable packing."
    $Upx = $false
}

if ($Setup -and -not $IsccPath) {
    Write-Warn "Inno Setup compiler (ISCC.exe) not found. Skipping installer build."
    $Setup = $false
}

if ($GpgSign -and -not $GpgPath) {
    Write-Warn "GPG binary (gpg.exe) not found. Skipping digital signing."
    $GpgSign = $false
}

# Auto-detect GPG signing key
if ($GpgSign -and $GpgPath -and [string]::IsNullOrWhiteSpace($GpgKey)) {
    Write-Info "Auto-detecting registered GPG secret keys..."
    try {
        $secKeys = & $GpgPath --list-secret-keys --keyid-format LONG 2>$null
        $keyMatched = $false
        foreach ($line in $secKeys) {
            if ($line -match 'sec\s+[^\/]+\/([0-9A-Fa-f]{8,})') {
                $GpgKey = $Matches[1]
                Write-Info "Auto-detected GPG secret key: $GpgKey"
                $keyMatched = $true
                break
            }
        }
        if (-not $keyMatched) {
            Write-Info "Signing with default GPG secret key."
        }
    } catch {
        Write-Warn "GPG key query failed: $_. Attempting default signature."
    }
}

# Prepare dist directory
$DistDir = Join-Path $ScriptDir "dist"
if ($Clean) {
    Write-Step "Cleaning previous build artifacts"
    if (Test-Path $DistDir) {
        Remove-Item -Path $DistDir -Recurse -Force
        Write-Info "Removed existing dist directory."
    }
    cargo clean
    Write-Info "cargo clean completed."
}

if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
}

# Step 1: Rust release binary compilation
Write-Step "Step 1: Compiling Rust release binary (cargo build --release)"
$CargoArgs = @("build", "--release")
$BuildStart = Get-Date
& cargo $CargoArgs
if ($LASTEXITCODE -ne 0) {
    Write-Err "Cargo build failed with exit code: $LASTEXITCODE"
    exit $LASTEXITCODE
}
$BuildDuration = [math]::Round(((Get-Date) - $BuildStart).TotalSeconds, 2)
Write-Info "Binary compiled in ${BuildDuration}s"

$ReleaseExe = Join-Path $ScriptDir "target\release\two_roblox_rs.exe"
if (-not (Test-Path $ReleaseExe)) {
    Write-Err "Compiled binary not found: $ReleaseExe"
    exit 1
}

$DistStandardExe = Join-Path $DistDir "Two-Roblox.exe"
Copy-Item $ReleaseExe -Destination $DistStandardExe -Force
$OrigSize = (Get-Item $DistStandardExe).Length
$OrigSizeMb = [math]::Round($OrigSize / 1MB, 2)
Write-Info "Standard executable ready: dist\Two-Roblox.exe (${OrigSizeMb} MB)"

$GeneratedArtifacts = @($DistStandardExe)

# Step 2: UPX portable binary packing
if ($Upx -and $UpxPath) {
    Write-Step "Step 2: Packing standalone portable binary (Two-Roblox-Portable.exe)"
    $DistPortableExe = Join-Path $DistDir "Two-Roblox-Portable.exe"
    Copy-Item $ReleaseExe -Destination $DistPortableExe -Force

    Write-Info "Executing UPX: $UpxPath --best --lzma `"$DistPortableExe`""
    & $UpxPath --best --lzma $DistPortableExe
    if ($LASTEXITCODE -eq 0) {
        $PackedSize = (Get-Item $DistPortableExe).Length
        $PackedSizeMb = [math]::Round($PackedSize / 1MB, 2)
        $Ratio = [math]::Round(($PackedSize / $OrigSize) * 100, 2)
        Write-Info "UPX portable ready: dist\Two-Roblox-Portable.exe"
        Write-Info "Packed size: ${PackedSizeMb} MB (${Ratio}% of original)"
        Write-Info "Portable version runs independently without installation."
        $GeneratedArtifacts += $DistPortableExe
    } else {
        Write-Warn "UPX compression finished with warnings or errors."
    }
}

# Step 3: Inno Setup installer creation
if ($Setup -and $IsccPath) {
    Write-Step "Step 3: Creating Inno Setup installer (Setup.exe)"
    $IssScript = Join-Path $ScriptDir "installer.iss"
    if (Test-Path $IssScript) {
        Write-Info "Running ISCC compiler..."
        & $IsccPath "/DMyAppVersion=$Version" "/O$DistDir" $IssScript
        if ($LASTEXITCODE -eq 0) {
            $SetupExe = Join-Path $DistDir "Two-Roblox-Setup-v$Version.exe"
            if (Test-Path $SetupExe) {
                $SetupSizeMb = [math]::Round((Get-Item $SetupExe).Length / 1MB, 2)
                Write-Info "Installer ready: dist\Two-Roblox-Setup-v$Version.exe (${SetupSizeMb} MB)"
                $GeneratedArtifacts += $SetupExe
            }
        } else {
            Write-Warn "Inno Setup compilation encountered errors."
        }
    } else {
        Write-Warn "installer.iss not found. Skipping installer build."
    }
}

# Step 4: Source code zip packaging
if ($SourceZip) {
    Write-Step "Step 4: Packaging clean source zip archive"
    $SourceZipPath = Join-Path $DistDir "Two-Roblox-v$Version-source.zip"
    $TempSourceFolder = Join-Path $env:TEMP "Two-Roblox-v$Version-source"

    if (Test-Path $TempSourceFolder) {
        Remove-Item -Path $TempSourceFolder -Recurse -Force
    }
    New-Item -ItemType Directory -Path $TempSourceFolder -Force | Out-Null

    # Copy source files
    $IncludeFolders = @("src", "tools", "assets")
    foreach ($folder in $IncludeFolders) {
        $srcPath = Join-Path $ScriptDir $folder
        if (Test-Path $srcPath) {
            Copy-Item -Path $srcPath -Destination (Join-Path $TempSourceFolder $folder) -Recurse -Force
        }
    }

    $IncludeFiles = @("Cargo.toml", "Cargo.lock", "build.rs", "installer.iss", "build.ps1", "README.md", "CHANGELOG.txt", ".gitignore")
    foreach ($file in $IncludeFiles) {
        $filePath = Join-Path $ScriptDir $file
        if (Test-Path $filePath) {
            Copy-Item -Path $filePath -Destination (Join-Path $TempSourceFolder $file) -Force
        }
    }

    if (Test-Path $SourceZipPath) {
        Remove-Item -Path $SourceZipPath -Force
    }

    Compress-Archive -Path "$TempSourceFolder\*" -DestinationPath $SourceZipPath -Force
    Remove-Item -Path $TempSourceFolder -Recurse -Force

    if (Test-Path $SourceZipPath) {
        $ZipSizeKb = [math]::Round((Get-Item $SourceZipPath).Length / 1KB, 2)
        Write-Info "Source zip ready: dist\Two-Roblox-v$Version-source.zip (${ZipSizeKb} KB)"
        $GeneratedArtifacts += $SourceZipPath
    }
}

# Step 5: SHA-256 checksum generation
Write-Step "Step 5: Generating SHA-256 integrity checksums"
$ChecksumFile = Join-Path $DistDir "SHA256SUMS.txt"
$ChecksumLines = @()
foreach ($art in $GeneratedArtifacts) {
    if (Test-Path $art) {
        $hash = (Get-FileHash -Path $art -Algorithm SHA256).Hash.ToLower()
        $fileName = Split-Path -Leaf $art
        $ChecksumLines += "$hash  $fileName"
    }
}
$ChecksumLines | Out-File -FilePath $ChecksumFile -Encoding utf8 -Force
Write-Info "Checksums saved: dist\SHA256SUMS.txt"
$GeneratedArtifacts += $ChecksumFile

# Step 6: GPG digital signing
if ($GpgSign -and $GpgPath) {
    Write-Step "Step 6: Performing GPG detached signatures (.asc)"
    Write-Info "GPG Path: $GpgPath"
    if ($GpgKey) {
        Write-Info "GPG Key: $GpgKey"
    }
    
    foreach ($art in $GeneratedArtifacts) {
        if (Test-Path $art) {
            $sigPath = "$art.asc"
            if (Test-Path $sigPath) { Remove-Item $sigPath -Force }

            $gpgArgs = @("--batch", "--yes", "--armor", "--detach-sign")
            if ($GpgKey) {
                $gpgArgs += @("--default-key", $GpgKey)
            }
            $gpgArgs += @("--output", $sigPath, $art)

            Write-Host "  -> Generating signature: $(Split-Path -Leaf $art).asc" -ForegroundColor White
            & $GpgPath $gpgArgs
            if ($LASTEXITCODE -eq 0 -and (Test-Path $sigPath)) {
                Write-Info "     Signature created: $(Split-Path -Leaf $sigPath)"
            } else {
                Write-Warn "     Signature skipped or failed: $(Split-Path -Leaf $art)"
            }
        }
    }
}

# Final summary table
Write-Header "Two-Roblox Build and Packaging Completed"
Write-Host "Output Directory: $DistDir" -ForegroundColor White
Write-Host ""
Write-Host ("{0,-35} {1,12} {2,66}" -f "File Name", "Size", "SHA-256 Checksum") -ForegroundColor Cyan
Write-Host ("-" * 115) -ForegroundColor Gray

Get-ChildItem -Path $DistDir | Sort-Object Name | ForEach-Object {
    $sizeStr = if ($_.Length -ge 1MB) {
        "$([math]::Round($_.Length / 1MB, 2)) MB"
    } else {
        "$([math]::Round($_.Length / 1KB, 2)) KB"
    }
    $fhash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash.ToLower()
    Write-Host ("{0,-35} {1,12} {2,66}" -f $_.Name, $sizeStr, $fhash) -ForegroundColor White
}

Write-Host ("-" * 115) -ForegroundColor Gray
Write-Host ""
Write-Info "All tasks completed successfully."
