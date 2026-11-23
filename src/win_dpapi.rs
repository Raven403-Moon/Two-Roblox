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
        CryptProtectData(
            &mut input,
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            0,
            &mut output,
        )
    };

    if success == 0 {
        return Err(anyhow::anyhow!("CryptProtectData 실패 (코드: {})", unsafe {
            windows_sys::Win32::Foundation::GetLastError()
        }));
    }

    let result = unsafe {
        std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
    };

    unsafe {
        LocalFree(output.pbData as _);
    }

    Ok(result)
}

pub fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    let success = unsafe {
        CryptUnprotectData(
            &mut input,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            0,
            &mut output,
        )
    };

    if success == 0 {
        return Err(anyhow::anyhow!("CryptUnprotectData 실패 (코드: {})", unsafe {
            windows_sys::Win32::Foundation::GetLastError()
        }));
    }

    let result = unsafe {
        std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
    };

    unsafe {
        LocalFree(output.pbData as _);
    }

    Ok(result)
}
