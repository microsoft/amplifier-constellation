//! The fixed detached supervisor must not keep its caller's capture pipes or
//! unrelated inheritable handles alive. Null stdio alone does not ensure that.
use std::io;

// Preserve UTF-16 units, including Windows paths containing unpaired surrogates.
// argv[0] has special parsing; later arguments use CRT backslash/quote rules.
#[cfg(any(windows, test))]
fn command_line(program: &[u16], args: &[Vec<u16>]) -> io::Result<Vec<u16>> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid supervisor command line",
        )
    };
    if program.is_empty() || program.contains(&0) || program.contains(&34) {
        return Err(invalid());
    }
    let mut line = vec![34];
    line.extend_from_slice(program);
    line.push(34);
    for arg in args {
        if arg.contains(&0) {
            return Err(invalid());
        }
        line.extend_from_slice(&[32, 34]);
        let mut slashes = 0;
        for &unit in arg {
            if unit == 92 {
                slashes += 1;
            } else {
                let count = if unit == 34 { slashes * 2 + 1 } else { slashes };
                line.extend(std::iter::repeat_n(92, count));
                line.push(unit);
                slashes = 0;
            }
        }
        line.extend(std::iter::repeat_n(92, slashes * 2));
        line.push(34);
    }
    line.push(0);
    if line.len() > 32_767 {
        return Err(invalid());
    }
    Ok(line)
}

#[cfg(windows)]
pub(super) use windows::spawn;

#[cfg(windows)]
mod windows {
    use super::command_line;
    use std::{
        ffi::{OsStr, c_void},
        io,
        mem::{size_of, size_of_val},
        os::windows::{
            ffi::OsStrExt,
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
        },
        path::Path,
        process::Command,
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{
            ERROR_INSUFFICIENT_BUFFER, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE,
            INVALID_HANDLE_VALUE,
        },
        Security::SECURITY_ATTRIBUTES,
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::Threading::{
            CREATE_NEW_PROCESS_GROUP, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DETACHED_PROCESS,
            DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
            InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
            STARTUPINFOEXW, UpdateProcThreadAttribute,
        },
    };

    struct Attributes(Vec<usize>);
    impl Attributes {
        fn new(handles: &[HANDLE]) -> io::Result<Self> {
            let mut bytes = 0;
            let queried =
                unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut bytes) };
            if queried != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER || bytes == 0
            {
                return Err(io::Error::last_os_error());
            }
            // Pointer-aligned storage, initialized before its destructor is armed.
            let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
            if unsafe {
                InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut bytes)
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            let attributes = Self(storage);
            if unsafe {
                UpdateProcThreadAttribute(
                    attributes.as_ptr(),
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    handles.as_ptr().cast(),
                    size_of_val(handles),
                    ptr::null_mut(),
                    ptr::null(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(attributes)
        }

        fn as_ptr(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
            self.0.as_ptr().cast::<c_void>().cast_mut()
        }
    }
    impl Drop for Attributes {
        fn drop(&mut self) {
            unsafe { DeleteProcThreadAttributeList(self.as_ptr()) };
        }
    }

    fn null_handle(read: bool) -> io::Result<OwnedHandle> {
        let name: Vec<u16> = OsStr::new(r"\\.\NUL")
            .encode_wide()
            .chain(Some(0))
            .collect();
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: 1,
        };
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                if read { GENERIC_READ } else { GENERIC_WRITE },
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &security,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }

    /// Only supervision::launch supplies this fixed, enrolled driver invocation.
    /// It inherits the caller's environment/cwd, as the existing launcher did.
    /// The returned handle must stay alive through generation observation; closing
    /// it does not terminate or wait for the independently owned supervisor.
    pub(crate) fn spawn(command: &Command) -> io::Result<(OwnedHandle, u32)> {
        if !Path::new(command.get_program()).is_absolute()
            || command.get_current_dir().is_some()
            || command.get_envs().next().is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unexpected supervisor invocation",
            ));
        }
        let program: Vec<u16> = command.get_program().encode_wide().collect();
        let args: Vec<Vec<u16>> = command
            .get_args()
            .map(|arg| arg.encode_wide().collect())
            .collect();
        let mut line = command_line(&program, &args)?;
        let mut program = program;
        program.push(0);
        let input = null_handle(true)?;
        let output = null_handle(false)?;
        let error = null_handle(false)?;
        let handles = [
            input.as_raw_handle(),
            output.as_raw_handle(),
            error.as_raw_handle(),
        ];
        // Attribute list references this array; all handles/storage outlive the call.
        let attributes = Attributes::new(&handles)?;
        let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = handles[0];
        startup.StartupInfo.hStdOutput = handles[1];
        startup.StartupInfo.hStdError = handles[2];
        startup.lpAttributeList = attributes.as_ptr();
        let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let created = unsafe {
            CreateProcessW(
                program.as_ptr(),
                line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                CREATE_UNICODE_ENVIRONMENT
                    | CREATE_NEW_PROCESS_GROUP
                    | DETACHED_PROCESS
                    | EXTENDED_STARTUPINFO_PRESENT,
                ptr::null(),
                ptr::null(),
                &startup.StartupInfo,
                &mut process,
            )
        };
        if created == 0 {
            return Err(io::Error::last_os_error());
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(process.hProcess) };
        drop(unsafe { OwnedHandle::from_raw_handle(process.hThread) });
        Ok((handle, process.dwProcessId))
    }
}

#[cfg(test)]
mod tests {
    use super::command_line;
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn preserves_unicode_empty_quoted_and_trailing_backslash_arguments() {
        let line = command_line(
            &wide(r"C:\程序 path\driver.exe"),
            &[
                wide(""),
                wide("quoted\"argument"),
                wide("trailing\\"),
                vec![0xd800],
            ],
        )
        .unwrap();
        let mut expected =
            wide("\"C:\\程序 path\\driver.exe\" \"\" \"quoted\\\"argument\" \"trailing\\\\\" \"");
        expected.extend_from_slice(&[0xd800, 34, 0]);
        assert_eq!(line, expected);
    }

    #[test]
    fn refuses_invalid_or_oversized_command_lines_before_dispatch() {
        for program in [vec![], wide("bad\"image"), vec![0]] {
            assert!(command_line(&program, &[]).is_err());
        }
        assert!(command_line(&wide("driver.exe"), &[vec![0]]).is_err());
        assert!(command_line(&wide("driver.exe"), &[vec![65; 32_767]]).is_err());
    }
}
