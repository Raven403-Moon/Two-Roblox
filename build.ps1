<#
.SYNOPSIS
    Two-Roblox Rust 고속 자동 빌드 및 배포 패키징 스크립트

.DESCRIPTION
    Two-Roblox의 릴리즈 바이너리 컴파일, UPX 압축 포터블 바이너리(Portable.exe) 생성,
    Inno Setup 기반 설치 파일(Setup) 패키징, 클린 소스코드 Zip 압축,
    그리고 GPG 서명 및 SHA-256 체크섬 생성을 지원합니다.

.PARAMETER Upx
    UPX 패커를 적용하여 무설치 독립 실행형 포터블 바이너리(Two-Roblox-Portable.exe)를 생성합니다.

.PARAMETER UpxPath
    사용할 UPX.exe 실행 파일의 절대/상대 경로.

.PARAMETER Setup
    Inno Setup 컴파일러(ISCC)를 통해 설치 파일(Two-Roblox-Setup.exe)을 생성합니다 (기본 활성).

.PARAMETER SourceZip
    타깃/임시 파일 및 개인키를 제외한 정제된 소스코드 Zip 압축 파일을 생성합니다 (기본 활성).

.PARAMETER GpgSign
    생성된 배포 아티팩트들에 대해 GPG 분리 서명(.asc)을 수행합니다. 시스템에 등록된 키를 자동 감지합니다.

.PARAMETER GpgKey
    GPG 서명에 사용할 특정 키 ID 또는 사용자 이메일 (생략 시 등록된 서명 키 자동 감지 및 기본 키 사용).

.PARAMETER All
    바이너리 빌드, UPX 포터블, Setup 설치 파일, 소스코드 Zip, GPG 서명 전체를 일괄 수행합니다.

.PARAMETER Clean
    빌드 시작 전 이전 빌드 산출물(dist, target)을 정리합니다.

.PARAMETER NonInteractive
    대화형 질의를 생략하고 지정된 매개변수 값으로만 실행합니다.
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

# 색상 출력 헬퍼 (이모지 배제, 순수 텍스트)
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

# Cargo.toml에서 버전 추출
$CargoTomlPath = Join-Path $ScriptDir "Cargo.toml"
if (-not (Test-Path $CargoTomlPath)) {
    Write-Err "Cargo.toml 파일을 찾을 수 없습니다: $CargoTomlPath"
    exit 1
}

$Version = "0.1.0"
$CargoContent = Get-Content $CargoTomlPath -Raw
if ($CargoContent -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
}
Write-Info "프로젝트 버전: v$Version"

# 툴 탐색 헬퍼 함수
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

# 기본 설치 경로 후보군
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

# 대화형 모드 처리 (옵션 인자가 하나도 전달되지 않은 경우)
$IsInteractive = (-not $NonInteractive) -and ($PSBoundParameters.Count -eq 0)

if ($IsInteractive) {
    Write-Host ""
    Write-Host "[빌드 옵션 선택 (기본값 설정)]" -ForegroundColor White
    
    # 1. UPX.exe 파일 경로 입력 (포터블 버전 생성용)
    $upxPromptHint = if ($detectedUpx) { " [기본 감지: $detectedUpx]" } else { " [생략 시 엔터]" }
    $inputUpx = Read-Host "1. UPX.exe 파일 위치를 입력하세요 (포터블 무설치 버전 생성)$upxPromptHint"
    if (-not [string]::IsNullOrWhiteSpace($inputUpx)) {
        $cleanPath = $inputUpx.Trim().Trim('"').Trim("'")
        if (Test-Path $cleanPath) {
            $UpxPath = (Resolve-Path $cleanPath).Path
            $Upx = $true
            Write-Info "지정된 UPX 실행 파일을 사용합니다: $UpxPath"
        } else {
            Write-Warn "입력한 UPX 파일 경로를 찾을 수 없습니다: $cleanPath. 포터블 압축을 생략합니다."
            $Upx = $false
        }
    } elseif ($detectedUpx) {
        $UpxPath = $detectedUpx
        $Upx = $true
        Write-Info "자동 감지된 UPX 실행 파일을 사용합니다: $UpxPath"
    } else {
        $Upx = $false
    }

    # 2. Inno Setup 설치 프로그램 생성 여부 (기본: Y)
    $ansSetup = Read-Host "2. Inno Setup 설치 파일(Setup.exe)을 생성하시겠습니까? (Y/n) [기본값: Y]"
    if ([string]::IsNullOrWhiteSpace($ansSetup) -or ($ansSetup -notmatch '^(n|no)$')) {
        $Setup = $true
    } else {
        $Setup = $false
    }

    # 3. 소스코드 Zip 생성 여부 (기본: Y)
    $ansZip = Read-Host "3. 소스코드 Zip 파일(Source.zip)을 패키징하시겠습니까? (Y/n) [기본값: Y]"
    if ([string]::IsNullOrWhiteSpace($ansZip) -or ($ansZip -notmatch '^(n|no)$')) {
        $SourceZip = $true
    } else {
        $SourceZip = $false
    }

    # 4. GPG 서명 여부 (기본: N)
    $ansGpg = Read-Host "4. 배포 파일들에 대해 GPG 디지털 서명을 수행하시겠습니까? (y/N) [기본값: N]"
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
    # 비대화형 실행 시 Setup 및 SourceZip은 명시적으로 주어지지 않으면 기본 활성화
    if (-not $PSBoundParameters.ContainsKey('Setup')) { $Setup = $true }
    if (-not $PSBoundParameters.ContainsKey('SourceZip')) { $SourceZip = $true }
    if ($Upx -and -not $UpxPath -and $detectedUpx) { $UpxPath = $detectedUpx }
}

# 도구 가용성 검증
if ($Upx -and -not $UpxPath) {
    Write-Warn "UPX 실행 파일을 찾을 수 없습니다. 포터블 압축을 건너뜁니다."
    $Upx = $false
}

if ($Setup -and -not $IsccPath) {
    Write-Warn "Inno Setup 컴파일러(ISCC.exe)를 찾을 수 없습니다. 설치 프로그램 생성을 건너뜁니다."
    $Setup = $false
}

if ($GpgSign -and -not $GpgPath) {
    Write-Warn "GPG 실행 파일(gpg.exe)을 찾을 수 없습니다. GPG 서명을 건너뜁니다."
    $GpgSign = $false
}

# GPG 자동 키 탐색
if ($GpgSign -and $GpgPath -and [string]::IsNullOrWhiteSpace($GpgKey)) {
    Write-Info "시스템에 등록된 GPG 서명 키를 자동 탐색합니다..."
    try {
        $secKeys = & $GpgPath --list-secret-keys --keyid-format LONG 2>$null
        $keyMatched = $false
        foreach ($line in $secKeys) {
            if ($line -match 'sec\s+[^\/]+\/([0-9A-Fa-f]{8,})') {
                $GpgKey = $Matches[1]
                Write-Info "사용 가능한 GPG 비밀키를 자동으로 감지했습니다: $GpgKey"
                $keyMatched = $true
                break
            }
        }
        if (-not $keyMatched) {
            Write-Info "등록된 키 목록 조회를 완료했습니다. GPG 기본(Default) 비밀키로 서명을 진행합니다."
        }
    } catch {
        Write-Warn "GPG 키 목록 자동 조회 실패: $_. 기본 서명을 시도합니다."
    }
}

# Dist 디렉터리 준비
$DistDir = Join-Path $ScriptDir "dist"
if ($Clean) {
    Write-Step "이전 빌드 산출물 정리 (Clean)"
    if (Test-Path $DistDir) {
        Remove-Item -Path $DistDir -Recurse -Force
        Write-Info "기존 dist 폴더를 삭제했습니다."
    }
    cargo clean
    Write-Info "cargo clean 완료."
}

if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
}

# 1. Rust 릴리즈 바이너리 빌드
Write-Step "1단계: Rust 릴리즈 바이너리 빌드 (cargo build --release)"
$CargoArgs = @("build", "--release")
$BuildStart = Get-Date
& cargo $CargoArgs
if ($LASTEXITCODE -ne 0) {
    Write-Err "Cargo 빌드에 실패했습니다 (종료 코드: $LASTEXITCODE)."
    exit $LASTEXITCODE
}
$BuildDuration = [math]::Round(((Get-Date) - $BuildStart).TotalSeconds, 2)
Write-Info "바이너리 컴파일 완료 (${BuildDuration}초)."

$ReleaseExe = Join-Path $ScriptDir "target\release\two_roblox_rs.exe"
if (-not (Test-Path $ReleaseExe)) {
    Write-Err "빌드 결과물 실행 파일을 찾을 수 없습니다: $ReleaseExe"
    exit 1
}

$DistStandardExe = Join-Path $DistDir "Two-Roblox.exe"
Copy-Item $ReleaseExe -Destination $DistStandardExe -Force
$OrigSize = (Get-Item $DistStandardExe).Length
$OrigSizeMb = [math]::Round($OrigSize / 1MB, 2)
Write-Info "기본 실행 파일 배포 준비 완료: dist\Two-Roblox.exe (${OrigSizeMb} MB)"

$GeneratedArtifacts = @($DistStandardExe)

# 2. UPX 패커를 이용한 포터블 버전 생성 (무설치 독립 실행형)
if ($Upx -and $UpxPath) {
    Write-Step "2단계: UPX 패커를 사용한 초경량 포터블 바이너리 패킹 (Two-Roblox-Portable.exe)"
    $DistPortableExe = Join-Path $DistDir "Two-Roblox-Portable.exe"
    Copy-Item $ReleaseExe -Destination $DistPortableExe -Force

    Write-Info "UPX 실행: $UpxPath --best --lzma `"$DistPortableExe`""
    & $UpxPath --best --lzma $DistPortableExe
    if ($LASTEXITCODE -eq 0) {
        $PackedSize = (Get-Item $DistPortableExe).Length
        $PackedSizeMb = [math]::Round($PackedSize / 1MB, 2)
        $Ratio = [math]::Round(($PackedSize / $OrigSize) * 100, 2)
        Write-Info "UPX 포터블 압축 완료: dist\Two-Roblox-Portable.exe"
        Write-Info "압축 크기: ${PackedSizeMb} MB (기존 대비 ${Ratio}%)"
        Write-Info "포터블 버전은 별도의 설치 과정 없이 바로 실행 가능한 독립형 GUI 바이너리입니다."
        $GeneratedArtifacts += $DistPortableExe
    } else {
        Write-Warn "UPX 압축 과정에서 경고 또는 오류가 발생했습니다."
    }
}

# 3. Inno Setup 설치 프로그램 생성 (선택)
if ($Setup -and $IsccPath) {
    Write-Step "3단계: Inno Setup 설치 파일(Setup.exe) 생성"
    $IssScript = Join-Path $ScriptDir "installer.iss"
    if (Test-Path $IssScript) {
        Write-Info "ISCC 컴파일러 실행 중..."
        & $IsccPath "/DMyAppVersion=$Version" "/O$DistDir" $IssScript
        if ($LASTEXITCODE -eq 0) {
            $SetupExe = Join-Path $DistDir "Two-Roblox-Setup-v$Version.exe"
            if (Test-Path $SetupExe) {
                $SetupSizeMb = [math]::Round((Get-Item $SetupExe).Length / 1MB, 2)
                Write-Info "설치 프로그램 생성 완료: dist\Two-Roblox-Setup-v$Version.exe (${SetupSizeMb} MB)"
                $GeneratedArtifacts += $SetupExe
            }
        } else {
            Write-Warn "Inno Setup 컴파일 중 오류가 발생했습니다."
        }
    } else {
        Write-Warn "installer.iss 스크립트 파일을 찾을 수 없어 설치 파일 생성을 생략합니다."
    }
}

# 4. 소스코드 Zip 파일 생성 (선택)
if ($SourceZip) {
    Write-Step "4단계: 정제된 소스코드 Zip 아카이브 패키징"
    $SourceZipPath = Join-Path $DistDir "Two-Roblox-v$Version-source.zip"
    $TempSourceFolder = Join-Path $env:TEMP "Two-Roblox-v$Version-source"

    if (Test-Path $TempSourceFolder) {
        Remove-Item -Path $TempSourceFolder -Recurse -Force
    }
    New-Item -ItemType Directory -Path $TempSourceFolder -Force | Out-Null

    # 소스 배포 대상 디렉터리 및 파일 복사
    $IncludeFolders = @("src", "tools", "assets")
    foreach ($folder in $IncludeFolders) {
        $srcPath = Join-Path $ScriptDir $folder
        if (Test-Path $srcPath) {
            Copy-Item -Path $srcPath -Destination (Join-Path $TempSourceFolder $folder) -Recurse -Force
        }
    }

    $IncludeFiles = @("Cargo.toml", "Cargo.lock", "build.rs", "installer.iss", "build.ps1", "README.md", "CHANGELOG.txt")
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
        Write-Info "소스코드 Zip 아카이브 생성 완료: dist\Two-Roblox-v$Version-source.zip (${ZipSizeKb} KB)"
        $GeneratedArtifacts += $SourceZipPath
    }
}

# 5. SHA-256 체크섬 파일 생성
Write-Step "5단계: SHA-256 무결성 해시 체크섬 생성"
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
Write-Info "체크섬 파일 기록 완료: dist\SHA256SUMS.txt"
$GeneratedArtifacts += $ChecksumFile

# 6. GPG 디지털 서명 (선택)
if ($GpgSign -and $GpgPath) {
    Write-Step "6단계: GPG 배포 아티팩트 서명 (.asc)"
    Write-Info "GPG 실행 경로: $GpgPath"
    if ($GpgKey) {
        Write-Info "서명에 사용할 GPG 키: $GpgKey"
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

            Write-Host "  -> 서명 생성: $(Split-Path -Leaf $art).asc" -ForegroundColor White
            & $GpgPath $gpgArgs
            if ($LASTEXITCODE -eq 0 -and (Test-Path $sigPath)) {
                Write-Info "     서명 완료: $(Split-Path -Leaf $sigPath)"
            } else {
                Write-Warn "     서명 생성 실패 또는 건너뜀: $(Split-Path -Leaf $art)"
            }
        }
    }
}

# 빌드 결과 요약 표
Write-Header "Two-Roblox 빌드 및 배포 패키징 완료"
Write-Host "배포 디렉터리: $DistDir" -ForegroundColor White
Write-Host ""
Write-Host ("{0,-35} {1,12} {2,66}" -f "파일명", "크기", "SHA-256 해시") -ForegroundColor Cyan
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
Write-Info "모든 작업이 성공적으로 종료되었습니다."
