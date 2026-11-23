use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;
use std::ptr;
use anyhow::{anyhow, Result};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT};
use windows_sys::Win32::System::Console::FreeConsole;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenProcess, TerminateProcess, PROCESS_QUERY_INFORMATION,
    PROCESS_TERMINATE, PROCESS_VM_READ,
};
use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, MoveWindow, SetForegroundWindow,
    SetWindowTextW, ShowWindow, SystemParametersInfoW, SPI_GETWORKAREA, SW_RESTORE,
    SW_SHOWNORMAL,
};

#[repr(C)]
#[allow(non_snake_case)]
struct PROCESS_MEMORY_COUNTERS {
    cb: u32,
    PageFaultCount: u32,
    PeakWorkingSetSize: usize,
    WorkingSetSize: usize,
    QuotaPeakPagedPoolUsage: usize,
    QuotaPagedPoolUsage: usize,
    QuotaPeakNonPagedPoolUsage: usize,
    QuotaNonPagedPoolUsage: usize,
    PagefileUsage: usize,
    PeakPagefileUsage: usize,
}

#[link(name = "psapi")]
extern "system" {
    fn K32GetProcessMemoryInfo(
        hProcess: HANDLE,
        ppsmemCounters: *mut PROCESS_MEMORY_COUNTERS,
        cb: u32,
    ) -> i32;
}

fn str_to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

#[derive(Debug, Clone)]
pub struct InstanceInfo {
    pub pid: u32,
    pub user_id: Option<u64>,
    pub username: String,
    pub memory_mb: u64,
    pub hwnd: Option<isize>,
}

pub struct MultiInstanceManager {
    handles: Vec<HANDLE>,
    pub lock_acquired: bool,
    handle64_path: String,
    pub tracked: HashMap<u64, (u32, String)>, // user_id -> (pid, username)
}

impl MultiInstanceManager {
    pub fn new() -> Self {
        Self {
            handles: Vec::new(),
            lock_acquired: false,
            handle64_path: "tools/handle64.exe".to_string(),
            tracked: HashMap::new(),
        }
    }

    pub fn prepare_for_launch(&mut self) -> Result<()> {
        let existing_pids = Self::get_running_roblox_pids();
        if !existing_pids.is_empty() {
            let _ = self.isolate_existing_roblox_processes();
        }

        if !self.lock_acquired {
            let ev_name = str_to_wide("ROBLOX_singletonEvent");
            let mx_name = str_to_wide("ROBLOX_singletonMutex");

            unsafe {
                let ev = CreateEventW(ptr::null(), 0, 0, ev_name.as_ptr());
                let mx = CreateMutexW(ptr::null(), 0, mx_name.as_ptr());

                if !ev.is_null() {
                    self.handles.push(ev);
                }
                if !mx.is_null() {
                    self.handles.push(mx);
                }
            }
            self.lock_acquired = !self.handles.is_empty();
        }

        Ok(())
    }

    pub fn release_locks(&mut self) {
        for h in self.handles.drain(..) {
            if !h.is_null() {
                unsafe {
                    CloseHandle(h);
                }
            }
        }
        self.lock_acquired = false;
    }

    pub fn get_running_roblox_pids() -> Vec<u32> {
        let mut pids = Vec::new();

        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot.is_null() {
                return pids;
            }

            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let len = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile[..len]);

                    if exe_name.eq_ignore_ascii_case("robloxplayerbeta.exe") {
                        pids.push(entry.th32ProcessID);
                    }

                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
        }

        pids
    }

    pub fn isolate_existing_roblox_processes(&self) -> Result<String> {
        let pids = Self::get_running_roblox_pids();
        if pids.is_empty() {
            return Ok("실행 중인 기존 Roblox 프로세스가 없습니다.".to_string());
        }

        if !is_admin() {
            return Err(anyhow!("이미 켜진 창을 유지한 채 추가 실행하려면 '관리자 권한'이 필요합니다."));
        }

        if !Path::new(&self.handle64_path).exists() {
            return Err(anyhow!("tools/handle64.exe 파일이 존재하지 않습니다."));
        }

        let mut closed_count = 0;
        let pattern_re = regex::Regex::new(r"(?i)([0-9A-Fa-f]+):\s*(?:Event|Mutant)\s+.*ROBLOX_singleton").unwrap();

        for pid in &pids {
            let output = Command::new(&self.handle64_path)
                .args(["-accepteula", "-nobanner", "-a", "-p", &pid.to_string(), "ROBLOX_singleton"])
                .output();

            if let Ok(out) = output {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    if let Some(caps) = pattern_re.captures(line) {
                        if let Some(handle_hex) = caps.get(1) {
                            let _ = Command::new(&self.handle64_path)
                                .args(["-accepteula", "-nobanner", "-p", &pid.to_string(), "-c", handle_hex.as_str(), "-y"])
                                .output();
                            closed_count += 1;
                        }
                    }
                }
            }
        }

        Ok(format!("기존 Roblox {}개 격리 완료 (핸들 {}개 해제)", pids.len(), closed_count))
    }

    pub fn is_account_running(&self, user_id: u64) -> bool {
        if let Some(&(pid, _)) = self.tracked.get(&user_id) {
            let running = Self::get_running_roblox_pids();
            running.contains(&pid)
        } else {
            false
        }
    }

    pub fn terminate_account(&mut self, user_id: u64) {
        if let Some((pid, _)) = self.tracked.remove(&user_id) {
            let _ = Self::terminate_pid(pid);
        }
    }

    pub fn get_active_instances_info(&mut self) -> Vec<InstanceInfo> {
        let running_pids = Self::get_running_roblox_pids();
        // 삭제된 프로세스 정리
        self.tracked.retain(|_, (pid, _)| running_pids.contains(pid));

        let mut pid_to_info: HashMap<u32, (u64, String)> = HashMap::new();
        for (&uid, &(pid, ref uname)) in &self.tracked {
            pid_to_info.insert(pid, (uid, uname.clone()));
        }

        let mut results = Vec::new();
        for pid in running_pids {
            let mut mem_mb = 0;
            unsafe {
                let h_proc = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
                if !h_proc.is_null() {
                    let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
                    pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
                    if K32GetProcessMemoryInfo(h_proc, &mut pmc, pmc.cb) != 0 {
                        mem_mb = (pmc.WorkingSetSize / (1024 * 1024)) as u64;
                    }
                    CloseHandle(h_proc);
                }
            }

            let hwnd = Self::find_hwnd_by_pid(pid).map(|h| h as isize);
            let (uid_opt, uname) = if let Some((u, un)) = pid_to_info.get(&pid) {
                (Some(*u), un.clone())
            } else {
                (None, "외부 실행 Roblox".to_string())
            };

            results.push(InstanceInfo {
                pid,
                user_id: uid_opt,
                username: uname,
                memory_mb: mem_mb,
                hwnd,
            });
        }

        results
    }

    pub fn launch_client_directly(
        &mut self,
        user_id: u64,
        username: &str,
        cli_args: &[String],
    ) -> Result<u32> {
        let pid = self.launch_client(username, cli_args)?;
        self.tracked.insert(user_id, (pid, username.to_string()));
        Ok(pid)
    }

    pub fn launch_client(&mut self, username: &str, cli_args: &[String]) -> Result<u32> {
        self.prepare_for_launch()?;

        if cli_args.is_empty() {
            return Err(anyhow!("실행 인자가 비어 있습니다."));
        }

        let exe_path = &cli_args[0];
        let exe_dir = Path::new(exe_path)
            .parent()
            .ok_or_else(|| anyhow!("실행 파일 디렉터리를 찾을 수 없습니다."))?;

        let child = Command::new(exe_path)
            .args(&cli_args[1..])
            .current_dir(exe_dir)
            .spawn()
            .map_err(|e| anyhow!("클라이언트 프로세스 시작 실패: {:?}", e))?;

        let pid = child.id();
        let uname = username.to_string();

        // 백그라운드 스레드에서 창 제목 [username] Roblox로 태깅
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
            while std::time::Instant::now() < deadline {
                if let Some(hwnd) = Self::find_hwnd_by_pid(pid) {
                    let title = str_to_wide(&format!("[{}] Roblox", uname));
                    unsafe {
                        SetWindowTextW(hwnd, title.as_ptr());
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(800));
            }
        });

        Ok(pid)
    }

    pub fn find_hwnd_by_pid(target_pid: u32) -> Option<HWND> {
        struct SearchContext {
            target_pid: u32,
            found_hwnd: Option<HWND>,
        }

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
            let ctx = &mut *(lparam as *mut SearchContext);
            if IsWindowVisible(hwnd) != 0 {
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == ctx.target_pid {
                    ctx.found_hwnd = Some(hwnd);
                    return 0; // stop enumeration
                }
            }
            1 // continue enumeration
        }

        let mut ctx = SearchContext {
            target_pid,
            found_hwnd: None,
        };

        unsafe {
            EnumWindows(Some(enum_proc), &mut ctx as *mut _ as LPARAM);
        }

        ctx.found_hwnd
    }

    #[allow(dead_code)]
    pub fn focus_window(hwnd: HWND) {
        if !hwnd.is_null() {
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
        }
    }

    pub fn tile_all_windows() -> usize {
        let pids = Self::get_running_roblox_pids();
        let mut hwnds = Vec::new();

        for pid in pids {
            if let Some(h) = Self::find_hwnd_by_pid(pid) {
                hwnds.push(h);
            }
        }

        let count = hwnds.len();
        if count == 0 {
            return 0;
        }

        let mut work_area: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                &mut work_area as *mut _ as _,
                0,
            );
        }

        let work_x = work_area.left;
        let work_y = work_area.top;
        let work_w = work_area.right - work_area.left;
        let work_h = work_area.bottom - work_area.top;

        let cols = (count as f64).sqrt().ceil() as i32;
        let rows = ((count as f64) / (cols as f64)).ceil() as i32;
        let cell_w = work_w / cols;
        let cell_h = work_h / rows;

        for (idx, &hwnd) in hwnds.iter().enumerate() {
            let r = (idx as i32) / cols;
            let c = (idx as i32) % cols;

            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                MoveWindow(
                    hwnd,
                    work_x + c * cell_w,
                    work_y + r * cell_h,
                    cell_w,
                    cell_h,
                    1,
                );
            }
        }

        count
    }

    pub fn terminate_pid(pid: u32) -> Result<()> {
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle.is_null() {
                return Err(anyhow!("프로세스 핸들을 열 수 없습니다 (PID: {})", pid));
            }
            TerminateProcess(handle, 1);
            CloseHandle(handle);
        }
        Ok(())
    }

    pub fn terminate_all(&mut self) {
        self.tracked.clear();
        for pid in Self::get_running_roblox_pids() {
            let _ = Self::terminate_pid(pid);
        }
    }
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

pub fn restart_as_admin() {
    let current_exe = std::env::current_exe().unwrap_or_default();
    let wide_exe = str_to_wide(&current_exe.to_string_lossy());
    let wide_runas = str_to_wide("runas");

    unsafe {
        FreeConsole();
        ShellExecuteW(
            ptr::null_mut(),
            wide_runas.as_ptr(),
            wide_exe.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}
