# Two-Roblox (Rust Native Core)

Two-Roblox는 윈도우 환경에서 여러 개의 Roblox 클라이언트를 계정 충돌 없이 동시에 실행하고, 다수의 계정을 강력하게 암호화하여 관리할 수 있는 고성능 네이티브 Rust 런처입니다.

기존 Python 기반 프로토타입에서 발생할 수 있는 런타임 오버헤드, 종속성 문제, 64비트 정수 오버플로우 문제를 완전히 해결하기 위해 메모리 안전성과 초고속 반응성을 보장하는 순수 Rust 언어로 전면 재설계되었습니다.

---

## 1. 프로젝트 개요

기존 로블록스 클라이언트는 단일 인스턴스 정책(`ROBLOX_singletonEvent`, `ROBLOX_singletonMutex`)으로 인해 하나의 PC에서 하나의 창만 실행할 수 있도록 제한되어 있습니다.

Two-Roblox Rust 코어는 다음 목표를 완벽하게 달성합니다:
1. Win32 커널 레벨 싱글톤 선점 및 프로세스 핸들 격리로 무제한 다중 클라이언트 동시 실행
2. 외부 브라우저 없이 앱 내부에서 직접 동작하는 Microsoft WebView2 내장 공식 웹 로그인 (2FA 및 Arkose Labs 캡차 완벽 대응)
3. 세션 쿠키(.ROBLOSECURITY) 자동 감지, 유효성 검증 및 로컬 암호화 보관함 연동
4. 공식 서버 라이브 빌드 실시간 비교 및 격리된 환경에서의 무중단 자동 업데이트 / 로그인 세션 유지
5. Windows DPAPI 연동 자동 언락 및 PBKDF2 48만 회 + AES-256-GCM 군사급 계정 보관함
6. 현대적 다크 테마 GUI (eframe/egui) 및 한국어 폰트(맑은 고딕) 기본 내장
7. 컴팩트 스플릿 드롭다운 런처 버튼 및 실시간 프로세스 모니터링
8. Inno Setup 인스톨러, UPX 초경량 포터블 바이너리, 소스코드 아카이브, GPG 서명 및 SHA-256 자동 빌드

---

## 2. 주요 핵심 기능

### 1) 다중 클라이언트 무제한 동시 실행
- 커널 레벨에서 싱글톤 이벤트 및 뮤텍스 핸들을 선점/해제하여 외부 부트스트래퍼 없이 다중 실행 환경을 조성합니다.
- 이미 켜져 있는 기존 Roblox 프로세스가 있더라도 창을 닫지 않고 해당 프로세스의 싱글톤 핸들만 선택적으로 해제(격리)하여 추가 실행이 가능합니다.

### 2) 앱 내 공식 웹 로그인 (WebView2 기반 2FA / 캡차 완벽 대응)
- 외부 브라우저 창을 띄우지 않고, 앱 내부의 전용 화면에서 Microsoft WebView2 엔진을 통해 공식 Roblox 로그인 화면(`https://www.roblox.com/login`)을 직접 렌더링합니다.
- 사용자는 앱 화면 안에서 아이디, 비밀번호 입력뿐만 아니라 2단계 인증(2FA 이메일/OTP), Arkose Labs 캡차 퍼즐까지 정상적으로 완료할 수 있습니다.
- 로그인이 성공하면 백그라운드 폴링을 통해 보안 세션 쿠키(`.ROBLOSECURITY`)를 자동 추출하고 암호화 보관함에 영구 등록한 뒤 즉시 계정 목록 화면으로 복귀합니다.
- 다중 계정을 위해 기존 쿠키 등록 방식(줄바꿈을 통한 대량 일괄 등록)도 동일하게 지원합니다.

### 3) 컴팩트 스플릿 드롭다운 런처 버튼
- 하단 런처 바에 자리 잡은 단일 스플릿 버튼(`[선택 계정 실행]` + `[^]`)을 통해 UI 공간을 효율적으로 사용합니다.
- 화살표(^) 버튼을 클릭하면 팝업 메뉴를 통해 '선택 계정 실행', '대기 계정 일괄 실행', '실행 간격(초) 설정'을 언제든지 손쉽게 전환할 수 있습니다.

### 4) 로블록스 무중단 자동 업데이트 및 세션 유지
- 공식 배포 API(`clientsettings.roblox.com`)를 통해 로컬 클라이언트 버전과 라이브 버전을 실시간 비교합니다.
- 새 버전이 감지되면 상단 헤더에 업데이트 알림 배너가 표시되며, 원클릭으로 격리된 1회용 임시 디렉터리에서 공식 인스톨러를 다운로드 및 무중단 설치합니다.
- 설치 완료 즉시 최신 실행 파일 경로를 자동으로 갱신하고, 새로운 단회성 인증 티켓(`rbx-authentication-ticket`)을 발급받아 최신 클라이언트에 주입하므로 로그인 세션 풀림 없이 게임/홈 화면에 바로 접속됩니다.

### 5) 엔터프라이즈급 보안 계정 보관함 (Vault)
- 계정 데이터베이스는 마스터 비밀번호 기반의 PBKDF2-HMAC-SHA256 (480,000회 반복) 키 도출 및 AES-256-GCM으로 강력하게 암호화되어 로컬 파일(`accounts.enc`)에 저장됩니다.
- Windows DPAPI(`CryptProtectData`)와 연동하여 "이 PC에서 마스터 비밀번호 기억" 활성화 시, 프로그램 재실행 시마다 매번 암호를 묻지 않고 운영체제 보안 수준에서 안전하게 자동 언락됩니다.

### 6) 실시간 프로세스 감시 및 윈도우 창 제어
- 실행 중인 모든 Roblox 프로세스(외부 실행 프로세스 포함)의 PID 및 메모리 사용량(MB)을 실시간 모니터링합니다.
- 실행 중인 모든 창을 모니터 작업 영역에 균등 분할 배치하는 "바둑판 자동 정렬" 및 "원클릭 개별/전체 종료" 기능을 지원합니다.

---

## 3. 취약점 진단 및 보안 방어 패치 내역 (Security & Bug Fixes)

Python 코드베이스 감사 및 Rust 재코딩 과정에서 발견된 취약점과 버그들을 분석하고 전면 방어 패치했습니다.

1. 64비트 User ID 정수 오버플로우 버그 해결
- 원인: 100억 대 이상의 최신 로블록스 User ID(예: `10232860322`)가 32비트 signed int 한계(-21억 ~ +21억)를 초과하여 기존 바인딩에서 OverflowError가 발생함.
- 해결: 계정 모델 및 API 전반에서 `u64` 타입을 채택하여 대형 ID에서도 오버플로우가 원천 발생하지 않도록 수정.

2. Place ID / Job ID 인젝션 취약점 차단 (`src/roblox_api.rs`)
- 원인: 사용자 입력이 URL 쿼리에 그대로 결합될 경우 특수문자나 쿼리 조작으로 인한 보안 위험 존재.
- 해결: Place ID는 순수 숫자(`^\d+$`), Job ID는 영숫자 하이픈 UUID(`^[0-9a-zA-Z\-_]+$`)만 허용하는 화이트리스트 정규식 검증 및 URL 인코딩을 적용.
- 인증 티켓 문자열에서도 줄바꿈(`\r`, `\n`) 및 따옴표를 정제하여 쉘 인젝션을 차단.

3. 인스톨러 임시 파일 하이재킹 및 레이스 컨디션 차단 (`src/updater.rs`)
- 원인: 공용 임시 디렉터리(`%TEMP%`)에 고정 파일명으로 다운로드 시 심볼릭 링크 공격 및 파일 치환(TOCTOU) 위험 존재.
- 해결: `tempfile::tempdir()`를 통해 무작위로 생성되는 1회용 격리 디렉터리에서 인스톨러를 다운로드/실행하고 즉시 삭제되도록 개선.

4. 로컬 DPAPI 키 파일 숨김 속성 적용 (`src/crypto_store.rs`)
- 자동 로그인에 사용되는 암호화 파일(`.vault_key`)에 Windows 숨김 속성(`FILE_ATTRIBUTE_HIDDEN`)을 부여하여 불필요한 노출 최소화.

---

## 4. 빌드 및 배포 패키징 방법 (Automated Build & Packaging)

`build.ps1` 스크립트를 통해 Rust 바이너리 컴파일부터 UPX 압축 포터블 바이너리, Inno Setup 기반 설치 파일(Setup), 정제된 소스코드 Zip 압축, GPG 디지털 서명 및 SHA-256 체크섬 생성까지 원클릭으로 수행할 수 있습니다.

### 사전 요구 사항
- 운영체제: Windows 10 / Windows 11 (64-bit)
- Rust 툴체인: Rust 1.75 이상 및 Cargo
- (선택) UPX: `tools/upx.exe` 또는 winget 패키지 (`winget install UPX.UPX`)
- (선택) Inno Setup 6: `winget install JRSoftware.InnoSetup`
- (선택) GPG: Gpg4win / GnuPG (`gpg.exe`)

---

### 원클릭 고속 빌드 (`build.ps1`)

#### 1) 대화형 메뉴 모드 (권장)
인자 없이 스크립트를 실행하면 대화형 프롬프트로 원하는 패키징 옵션을 선택하여 빌드할 수 있습니다:

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

#### 2) 전체 일괄 빌드 및 패키징 (`-All`)
바이너리 빌드, UPX 포터블 압축, Inno Setup 설치 파일, 소스코드 Zip, GPG 서명까지 일괄 처리합니다:

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1 -All
```

#### 3) 개별 옵션 파라미터 빌드
필요한 기능만 스위치로 지정하여 비대화형으로 즉시 빌드할 수 있습니다:

```powershell
# UPX 압축 포터블 버전 + Inno Setup 설치 파일 + 소스코드 Zip 생성
powershell -ExecutionPolicy Bypass -File .\build.ps1 -Upx -Setup -SourceZip

# 특정 GPG 키로 서명 지정 빌드
powershell -ExecutionPolicy Bypass -File .\build.ps1 -Upx -Setup -GpgSign -GpgKey "Raven <prydeqfriezjhkpuz@hotmail.com>"

# 기존 빌드 캐시 및 dist 디렉터리 정리 후 빌드
powershell -ExecutionPolicy Bypass -File .\build.ps1 -Clean -All
```

---

### 빌드 산출물 구조 (`dist/`)

빌드가 완료되면 프로젝트 루트의 `dist/` 디렉터리에 배포용 파일들이 자동으로 정리됩니다:

| 파일명 | 종류 | 설명 |
|---|---|---|
| `Two-Roblox.exe` | 실행 바이너리 | 순수 Rust 릴리즈 최적화 실행 파일 |
| `Two-Roblox-Portable.exe` | 포터블 바이너리 | UPX LZMA 초고압축 독립 실행 파일 (설치 불필요) |
| `Two-Roblox-Setup-v1.1.0.exe` | 설치 프로그램 | 시작 메뉴/바탕화면 바로가기 및 언인스톨러가 포함된 인스톨러 |
| `Two-Roblox-v0.1.0-source.zip` | 소스코드 아카이브 | 빌드 캐시 및 개인 데이터가 배제된 순수 배포용 소스코드 압축 파일 |
| `SHA256SUMS.txt` | 무결성 체크섬 | 생성된 모든 배포 파일의 SHA-256 해시 목록 |
| `*.asc` (선택) | GPG 분리 서명 | GPG 개인키로 전자 서명된 무결성 인증 파일 |

---

### 수동 Cargo 빌드 (개발용)

단순 개발 및 디버깅 시에는 Cargo 명령으로 직접 컴파일할 수 있습니다:

```powershell
cargo build --release
.\target\release\two_roblox_rs.exe
```

---

## 5. 핵심 Rust 구현 코드 예시 (Implementation & Code Snippets)

### 1) Win32 커널 싱글톤 선점 및 격리 (`src/multi_instance.rs`)
로블록스의 단일 인스턴스 검사를 우회하기 위해 Windows 커널 객체 `ROBLOX_singletonEvent`와 `ROBLOX_singletonMutex`를 선점합니다.

```rust
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW};

fn str_to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

pub struct MultiInstanceManager {
    handles: Vec<HANDLE>,
    pub lock_acquired: bool,
}

impl MultiInstanceManager {
    pub fn prepare_for_launch(&mut self) -> Result<(), anyhow::Error> {
        if !self.lock_acquired {
            let ev_name = str_to_wide("ROBLOX_singletonEvent");
            let mx_name = str_to_wide("ROBLOX_singletonMutex");

            unsafe {
                let ev = CreateEventW(std::ptr::null(), 0, 0, ev_name.as_ptr());
                let mx = CreateMutexW(std::ptr::null(), 0, mx_name.as_ptr());

                if !ev.is_null() { self.handles.push(ev); }
                if !mx.is_null() { self.handles.push(mx); }
            }
            self.lock_acquired = !self.handles.is_empty();
        }
        Ok(())
    }

    pub fn release_locks(&mut self) {
        for h in self.handles.drain(..) {
            if !h.is_null() {
                unsafe { CloseHandle(h); }
            }
        }
        self.lock_acquired = false;
    }
}
```

---

### 2) 앱 내 공식 로그인 화면 및 세션 쿠키 자동 추출 (`src/web_login.rs`)
앱 윈도우 내부에 자식 윈도우로 WebView2를 호스팅하고, 로그인 완료 시 세션 쿠키를 즉시 가로채어 보관함에 영구 저장합니다.

```rust
use wry::{Rect, WebView, WebViewBuilder};
use wry::raw_window_handle::HasWindowHandle;

pub struct EmbeddedWebLogin {
    pub webview: WebView,
    pub status: String,
    pub last_cookie: Option<String>,
}

impl EmbeddedWebLogin {
    pub fn new<W: HasWindowHandle>(window: &W, x: f64, y: f64, w: f64, h: f64) -> Result<Self, anyhow::Error> {
        let bounds = Rect {
            position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(x, y)),
            size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(w, h)),
        };

        let webview = WebViewBuilder::new()
            .with_url("https://www.roblox.com/login")
            .with_bounds(bounds)
            .build_as_child(window)?;

        Ok(Self {
            webview,
            status: "공식 로그인 화면에서 로그인해 주세요.".to_string(),
            last_cookie: None,
        })
    }

    pub fn poll_cookie(&mut self) -> Option<String> {
        if let Ok(cookies) = self.webview.cookies() {
            for c in cookies {
                if c.name() == ".ROBLOSECURITY" {
                    let val = c.value().trim().to_string();
                    if !val.is_empty() && self.last_cookie.as_deref() != Some(&val) {
                        self.last_cookie = Some(val.clone());
                        return Some(val);
                    }
                }
            }
        }
        None
    }
}
```

---

### 3) 단회성 인증 티켓 발급 및 직접 실행 CLI 파라미터 빌드 (`src/roblox_api.rs`)
영구 쿠키 대신 수 초 내에 만료되는 일회용 인증 티켓을 발급받아 CLI 인자로 주입하며, 입력값에 대한 화이트리스트 검증을 수행합니다.

```rust
use regex::Regex;
use anyhow::{anyhow, Result};

pub fn build_direct_cli_args(
    exe_path: &str,
    auth_ticket: &str,
    place_id: Option<&str>,
    job_id: Option<&str>,
    locale: &str,
) -> Result<Vec<String>> {
    let tracker_id: u64 = rand::random::<u64>() % 90_000_000_000 + 10_000_000_000;
    let clean_ticket = auth_ticket.replace(['\r', '\n', '"', ' '], "");

    let mut args = vec![
        exe_path.to_string(),
        "--app".to_string(),
        "-a".to_string(),
        "https://www.roblox.com/Login/Negotiate.ashx".to_string(),
        "-t".to_string(),
        clean_ticket,
        "-b".to_string(),
        tracker_id.to_string(),
        "--rloc".to_string(),
        locale.to_string(),
        "--gloc".to_string(),
        locale.to_string(),
    ];

    if let Some(pid_raw) = place_id {
        let pid = pid_raw.trim();
        if !pid.is_empty() {
            // Place ID 순수 숫자 검증 (인젝션 방어)
            let num_re = Regex::new(r"^\d+$").unwrap();
            if !num_re.is_match(pid) {
                return Err(anyhow!("Place ID는 순수 숫자 형식이어야 합니다."));
            }

            let join_url = format!(
                "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId={}&placeId={}&isPlayTogetherGame=false",
                tracker_id, pid
            );
            args.push("-j".to_string());
            args.push(join_url);
        }
    }

    Ok(args)
}
```

---

### 4) Windows DPAPI를 이용한 안전한 키 보호 (`src/win_dpapi.rs`)
Windows 운영체제의 데이터 보호 API를 FFI로 호출하여 현재 로그인된 Windows 계정에서만 복호화 가능한 안전한 키 캐싱을 구현했습니다.

```rust
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};
use windows_sys::Win32::Foundation::LocalFree;
use std::ptr;

pub fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    let success = unsafe {
        CryptProtectData(&mut input, ptr::null(), ptr::null_mut(), ptr::null(), ptr::null(), 0, &mut output)
    };

    if success == 0 {
        return Err(anyhow::anyhow!("CryptProtectData 실패"));
    }

    let result = unsafe {
        std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
    };
    unsafe { LocalFree(output.pbData as _); }

    Ok(result)
}
```

---

### 5) PBKDF2 48만 회 + AES-256-GCM 계정 보관함 (`src/crypto_store.rs`)
계정 정보를 JSON으로 직렬화한 후 난수 Nonce와 함께 AES-256-GCM으로 암호화하여 저장합니다.

```rust
use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use pbkdf2::pbkdf2_hmac_array;
use sha2::Sha256;

const PBKDF2_ITERATIONS: u32 = 480_000;

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    pbkdf2_hmac_array::<Sha256, 32>(password.as_bytes(), salt, PBKDF2_ITERATIONS)
}

pub fn encrypt_accounts(accounts: &[Account], key: &[u8; 32], salt: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let payload = serde_json::to_vec(accounts)?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(key)?;
    let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce_bytes), payload.as_ref())
        .map_err(|e| anyhow::anyhow!("암호화 실패: {:?}", e))?;

    let mut out = Vec::new();
    out.extend_from_slice(salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}
```

---

## 6. 프로젝트 디렉터리 구조

```
Two-Roblox/
│
├── Cargo.toml                  # Rust 프로젝트 명세 및 의존성 설정
├── Cargo.lock                  # 의존성 잠금 파일
├── build.rs                    # Windows 아이콘 및 바이너리 메타데이터 리소스 컴파일러
├── build.ps1                   # 원클릭 자동 빌드, UPX 압축, Setup 패키징 및 GPG 서명 스크립트
├── installer.iss               # Inno Setup 6 설치 프로그램 구성 스크립트
├── README.md                   # 상세 기술 설명서 및 코드 분석 문서
├── CHANGELOG.txt               # 작업 내역 문서
│
├── src/                        # Rust 소스 코드
│   ├── main.rs                 # Win32 애플리케이션 진입점 및 뷰포트 초기화
│   ├── app.rs                  # eframe/egui GUI 메인 인터페이스 (탭, 모달, 런처 바)
│   ├── web_login.rs            # WebView2 기반 인앱 공식 로블록스 웹 로그인 및 쿠키 추출
│   ├── roblox_api.rs           # 공식 API 통신, 인증 티켓 발급, CLI 인자 빌더
│   ├── multi_instance.rs       # Win32 커널 싱글톤 선점, 핸들 격리, 창 제어
│   ├── updater.rs              # 버전 감지 및 격리 임시 폴더 기반 자동 업데이트
│   ├── crypto_store.rs         # PBKDF2 48만회 + AES-256-GCM 계정 보관함
│   └── win_dpapi.rs            # Windows DPAPI 안전 암호화 FFI 래퍼
│
├── tools/                      # 시스템 도구
│   ├── handle64.exe            # 프로세스 핸들 격리용 Sysinternals 바이너리
│   └── upx.exe                 # 포터블 바이너리 패킹용 UPX 실행 파일
│
├── assets/                     # 그래픽 리소스
│   ├── icon.png                # 공식 원본 아이콘 이미지
│   └── icon.ico                # Win32 및 인스톨러용 멀티사이즈 아이콘
│
└── dist/                       # 빌드 및 패키징 산출물 (build.ps1 생성)
    ├── Two-Roblox.exe          # 표준 릴리즈 실행 파일
    ├── Two-Roblox-Portable.exe # UPX 초경량 포터블 바이너리
    ├── Two-Roblox-Setup-v*.exe # Inno Setup 설치 프로그램
    ├── Two-Roblox-*-source.zip # 배포용 소스코드 아카이브
    ├── SHA256SUMS.txt          # 무결성 해시 목록
    └── *.asc                   # GPG 분리 서명 파일
```
