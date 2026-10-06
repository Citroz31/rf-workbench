//! Small safe session facade over the standard VISA C ABI. Symbols are loaded
//! dynamically so simulator/TCP users do not need a vendor SDK at build time.
//! ABI reference: VISA visa.h / visatype.h, NI-VISA message-based operations.
use super::{Error, MAX_RESPONSE, Result, Session, read_definite_block, validate_command};
use libloading::Library;
use std::ffi::{CString, c_char};
use std::time::{Duration, Instant};

type OpenRm = unsafe extern "system" fn(*mut u32) -> i32;
type Open = unsafe extern "system" fn(u32, *const c_char, u32, u32, *mut u32) -> i32;
type SetAttr = unsafe extern "system" fn(u32, u32, usize) -> i32;
type Write = unsafe extern "system" fn(u32, *const u8, u32, *mut u32) -> i32;
type Read = unsafe extern "system" fn(u32, *mut u8, u32, *mut u32) -> i32;
type Close = unsafe extern "system" fn(u32) -> i32;
const TMO: u32 = 0x3FFF001A;
const TERMCHAR: u32 = 0x3FFF0018;
const TERMCHAR_EN: u32 = 0x3FFF0038;
const MAX_CNT: i32 = 0x3FFF0006;

pub struct VisaSession {
    // This field retains the library for all function pointer calls and Drop.
    _library: Library,
    rm: u32,
    session: u32,
    set_attr: SetAttr,
    write_fn: Write,
    read_fn: Read,
    close: Close,
    timeout: Duration,
    poisoned: bool,
}
fn status(code: i32, operation: &str) -> Result<()> {
    if code < 0 {
        Err(Error::Protocol(format!(
            "VISA {operation} : statut 0x{:08X}",
            code as u32
        )))
    } else {
        Ok(())
    }
}
impl VisaSession {
    pub fn open(resource: &str, timeout: Duration) -> Result<Self> {
        let name = std::env::var_os("RF_WORKBENCH_VISA").unwrap_or_else(|| {
            #[cfg(windows)]
            {
                std::path::PathBuf::from(
                    std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()),
                )
                .join(if cfg!(target_pointer_width = "64") {
                    "System32/visa64.dll"
                } else {
                    "System32/visa32.dll"
                })
                .into_os_string()
            }
            #[cfg(target_os = "macos")]
            {
                "/Library/Frameworks/VISA.framework/VISA".into()
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                "libvisa.so".into()
            }
        });
        // SAFETY: loading is restricted to the system runtime or a user-supplied
        // library. Symbols use standard fixed-width VISA types and system ABI.
        let library=unsafe{Library::new(&name)}.map_err(|e|Error::Protocol(format!("Runtime VISA absent ou incompatible ({}) : {e}. Installer NI-VISA/Keysight VISA ou définir RF_WORKBENCH_VISA.",name.to_string_lossy())))?;
        // SAFETY: symbol signatures match VISA's C headers. The library lives
        // inside VisaSession until after viClose has run.
        let (open_rm, open, set_attr, write_fn, read_fn, close) = unsafe {
            let lookup =
                |e: libloading::Error| Error::Protocol(format!("Symbole VISA absent : {e}"));
            (
                *library
                    .get::<OpenRm>(b"viOpenDefaultRM\0")
                    .map_err(lookup)?,
                *library.get::<Open>(b"viOpen\0").map_err(lookup)?,
                *library
                    .get::<SetAttr>(b"viSetAttribute\0")
                    .map_err(lookup)?,
                *library.get::<Write>(b"viWrite\0").map_err(lookup)?,
                *library.get::<Read>(b"viRead\0").map_err(lookup)?,
                *library.get::<Close>(b"viClose\0").map_err(lookup)?,
            )
        };
        let resource = CString::new(resource)
            .map_err(|_| Error::Protocol("Ressource VISA contenant NUL".into()))?;
        let mut rm = 0;
        let mut session = 0;
        // SAFETY: valid pointers to owned scalar outputs and nul-terminated name.
        unsafe {
            status(open_rm(&mut rm), "viOpenDefaultRM")?;
            if let Err(e) = status(
                open(
                    rm,
                    resource.as_ptr(),
                    0,
                    timeout.as_millis().min(u32::MAX as u128) as u32,
                    &mut session,
                ),
                "viOpen",
            ) {
                close(rm);
                return Err(e);
            }
        }
        let instance = Self {
            _library: library,
            rm,
            session,
            set_attr,
            write_fn,
            read_fn,
            close,
            timeout,
            poisoned: false,
        };
        // Setup failures drop the instance, closing both native sessions.
        instance.attribute(TMO, timeout.as_millis().min(u32::MAX as u128) as usize)?;
        instance.attribute(TERMCHAR, b'\n' as usize)?;
        instance.attribute(TERMCHAR_EN, 1)?;
        Ok(instance)
    }
    fn attribute(&self, attr: u32, value: usize) -> Result<()> {
        // SAFETY: live VISA session, documented attribute/value types.
        status(
            unsafe { (self.set_attr)(self.session, attr, value) },
            "viSetAttribute",
        )
    }
    fn response(&mut self) -> Result<Vec<u8>> {
        let deadline = Instant::now() + self.timeout;
        let mut data = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Error::Protocol("Réponse VISA expirée".into()));
            }
            self.attribute(TMO, remaining.as_millis().max(1) as usize)?;
            let mut chunk = [0u8; 8192];
            let mut count = 0;
            // SAFETY: writable buffer and return-count pointers are live; VISA
            // receives the exact allocated size, and count is checked below.
            let code = unsafe {
                (self.read_fn)(
                    self.session,
                    chunk.as_mut_ptr(),
                    chunk.len() as u32,
                    &mut count,
                )
            };
            status(code, "viRead")?;
            if count as usize > chunk.len() || data.len() + count as usize > MAX_RESPONSE {
                return Err(Error::Protocol("Taille de réponse VISA invalide".into()));
            }
            data.extend_from_slice(&chunk[..count as usize]);
            if code != MAX_CNT {
                return Ok(data);
            }
            if count == 0 {
                return Err(Error::Protocol("Lecture VISA sans progrès".into()));
            }
        }
    }
}
impl Session for VisaSession {
    fn write(&mut self, command: &str) -> Result<()> {
        validate_command(command)?;
        if self.poisoned {
            return Err(Error::Protocol(
                "Session VISA désynchronisée ; reconnecter".into(),
            ));
        }
        self.attribute(TMO, self.timeout.as_millis() as usize)?;
        let mut data = command.as_bytes().to_vec();
        data.push(b'\n');
        let mut count = 0;
        // SAFETY: pointer and size refer to the same live immutable buffer.
        status(
            unsafe { (self.write_fn)(self.session, data.as_ptr(), data.len() as u32, &mut count) },
            "viWrite",
        )?;
        if count as usize != data.len() {
            self.poisoned = true;
            return Err(Error::Protocol("Écriture VISA partielle".into()));
        }
        Ok(())
    }
    fn query(&mut self, command: &str) -> Result<String> {
        self.attribute(TERMCHAR_EN, 1)?;
        self.write(command)?;
        let result = self.response().and_then(|bytes| {
            String::from_utf8(bytes)
                .map(|s| s.trim_end_matches(['\n', '\r']).into())
                .map_err(|_| Error::Protocol("Réponse VISA non UTF-8".into()))
        });
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn read_binary(&mut self, command: &str) -> Result<Vec<u8>> {
        self.attribute(TERMCHAR_EN, 0)?;
        let result = self
            .write(command)
            .and_then(|_| self.response())
            .and_then(|bytes| read_definite_block(&mut bytes.as_slice()));
        let restore = self.attribute(TERMCHAR_EN, 1);
        if result.is_err() || restore.is_err() {
            self.poisoned = true;
        }
        restore?;
        result
    }
}
impl Drop for VisaSession {
    fn drop(&mut self) {
        // SAFETY: handles were successfully opened, are owned by this object,
        // and are closed before the shared library is unloaded.
        unsafe {
            (self.close)(self.session);
            (self.close)(self.rm);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visa_warnings_are_success_errors_are_errors() {
        assert!(status(0, "read").is_ok());
        assert!(status(MAX_CNT, "read").is_ok());
        assert!(
            status(0xBFFF0015u32 as i32, "read")
                .unwrap_err()
                .to_string()
                .contains("BFFF0015")
        );
    }
}
