use std::collections::BTreeMap;
use std::ffi::{c_char, c_void};
#[cfg(unix)]
use std::ffi::{c_int, CString};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use easyscript_archive::read_tar;
use easyscript_parser::{Program, TopLevelItem};

pub fn validate_library_imports(
    program: &Program,
    source_path: &Path,
) -> Result<Option<PathBuf>, String> {
    let imports: Vec<_> = program
        .items
        .iter()
        .filter_map(|item| match item {
            TopLevelItem::Import(import) => Some(import),
            _ => None,
        })
        .collect();
    if imports.is_empty() {
        return Ok(None);
    }

    let source_directory = source_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let library_directory = source_directory.join("library");
    if !library_directory.is_dir() {
        return Err(format!(
            "missing library directory beside {}",
            source_path.display()
        ));
    }

    let mut std_library_path = None;
    for import in imports {
        let declaration_path = library_directory.join(format!("{}.esh", import.library));
        if !declaration_path.is_file() {
            return Err(format!(
                "library {} header file ({}.esh) not found",
                import.library, import.library
            ));
        }
        let source = fs::read_to_string(&declaration_path).map_err(|error| {
            format!(
                "could not read library {} at {}: {error}",
                import.library,
                declaration_path.display()
            )
        })?;
        let symbols = parse_declaration_file(&source).map_err(|error| {
            format!(
                "invalid library {} declaration at {}: {error}",
                import.library,
                declaration_path.display()
            )
        })?;
        if import.library == "std" {
            validate_std_symbols(&symbols)?;
            let package_path = find_std_package(&library_directory)?;
            std_library_path = Some(extract_native_library(&package_path)?);
        }
    }
    Ok(std_library_path)
}

fn parse_declaration_file(source: &str) -> Result<BTreeMap<String, String>, String> {
    let declarations = easyscript_parser::parse_library_header(source)?;
    let mut symbols = BTreeMap::new();
    for (name, abi) in declarations {
        if symbols.insert(name.clone(), abi).is_some() {
            return Err(format!("duplicate extern symbol `std::{name}`"));
        }
    }
    Ok(symbols)
}

fn validate_std_symbols(symbols: &BTreeMap<String, String>) -> Result<(), String> {
    let expected = [
        ("write", "es_std_write"),
        ("input", "es_std_input"),
        ("output", "es_std_output"),
    ];
    if symbols.len() != expected.len() {
        return Err("std.esh must declare exactly write, input, and output".to_owned());
    }
    for (name, abi) in expected {
        if symbols.get(name).map(String::as_str) != Some(abi) {
            return Err(format!(
                "std.esh must map `std::{name}` to ABI symbol `{abi}`"
            ));
        }
    }
    Ok(())
}

fn find_std_package(directory: &Path) -> Result<PathBuf, String> {
    let package_path = directory.join("std.ess");
    if package_path.is_file() {
        Ok(package_path)
    } else {
        Err(format!(
            "std library package (std.ess) not found at {}",
            package_path.display()
        ))
    }
}

fn extract_native_library(package_path: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let native_filename = "es_std.dll";
    #[cfg(target_os = "linux")]
    let native_filename = "libes_std.so";
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    return Err("EasyScript std library is only packaged for Linux and Windows".to_owned());

    let metadata = fs::metadata(package_path)
        .map_err(|error| format!("could not inspect {}: {error}", package_path.display()))?;
    let modified = metadata
        .modified()
        .unwrap_or(UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    package_path.hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    modified.hash(&mut hasher);
    let cache_directory = std::env::temp_dir()
        .join("easyscript")
        .join(format!("std-{:016x}", hasher.finish()));
    fs::create_dir_all(&cache_directory)
        .map_err(|error| format!("could not create std library cache: {error}"))?;
    let cache_path = cache_directory.join(native_filename);
    if cache_path.is_file() {
        return Ok(cache_path);
    }

    let expected_entries = ["libes_std.so", "es_std.dll"];
    let mut found = BTreeMap::new();
    let entries = read_tar(package_path).map_err(|error| {
        format!(
            "invalid std tar package {}: {error}",
            package_path.display()
        )
    })?;
    for entry in entries {
        let pathname = entry.pathname;
        if !entry.is_regular_file {
            return Err(format!(
                "std package entry `{pathname}` is not a regular file"
            ));
        }
        if !expected_entries.contains(&pathname.as_str()) || found.contains_key(&pathname) {
            return Err(format!(
                "unexpected or duplicate std package entry `{pathname}`"
            ));
        }
        found.insert(pathname, entry.data);
    }
    if found.len() != expected_entries.len() {
        return Err("std package must contain root files libes_std.so and es_std.dll".to_owned());
    }
    let contents = found
        .remove(native_filename)
        .ok_or_else(|| format!("std package is missing {native_filename}"))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary_path = cache_path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    fs::write(&temporary_path, contents)
        .map_err(|error| format!("could not cache std library: {error}"))?;
    if let Err(error) = fs::rename(&temporary_path, &cache_path) {
        if cache_path.is_file() {
            fs::remove_file(&temporary_path).map_err(|cleanup_error| {
                format!("could not clean cache temporary file: {cleanup_error}")
            })?;
        } else {
            fs::remove_file(&temporary_path).map_err(|cleanup_error| {
                format!(
                    "could not install cached std library ({error}) or clean its temporary file: {cleanup_error}"
                )
            })?;
            return Err(format!("could not install cached std library: {error}"));
        }
    }
    Ok(cache_path)
}

type WriteFunction = unsafe extern "C" fn(*const u8, usize) -> i32;
type OutputFunction = unsafe extern "C" fn() -> i32;
type InputFunction = unsafe extern "C" fn(*mut usize) -> *mut u8;

pub struct StdLibrary {
    #[allow(dead_code)]
    handle: *mut c_void,
    write: WriteFunction,
    output: OutputFunction,
    input: InputFunction,
    buffered_value: Mutex<Option<Vec<u8>>>,
}

impl StdLibrary {
    pub fn load(path: &Path) -> Result<Self, String> {
        #[cfg(unix)]
        let handle = {
            let path_string = path.to_string_lossy();
            let path_c = CString::new(path_string.as_bytes()).map_err(|_| {
                format!("std library path contains a null byte: {}", path.display())
            })?;
            unsafe { platform::open(path_c.as_ptr()) }
        };
        #[cfg(target_os = "windows")]
        let handle = {
            use std::os::windows::ffi::OsStrExt;
            let path_wide = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            unsafe { platform::open(path_wide.as_ptr()) }
        };
        #[cfg(not(any(unix, target_os = "windows")))]
        return Err("dynamic std library loading is not supported on this platform".to_owned());

        if handle.is_null() {
            return Err(format!("could not load std library {}", path.display()));
        }

        let loaded = unsafe {
            (|| {
                Ok(Self {
                    handle,
                    write: load_symbol(handle, b"es_std_write\0", path)?,
                    output: load_symbol(handle, b"es_std_output\0", path)?,
                    input: load_symbol(handle, b"es_std_input\0", path)?,
                    buffered_value: Mutex::new(None),
                })
            })()
        };
        if loaded.is_err() {
            unsafe { platform::close(handle) };
        }
        loaded
    }

    pub fn write(&self, value: &[u8]) -> Result<(), String> {
        let result = unsafe { (self.write)(value.as_ptr(), value.len()) };
        if result != 0 {
            return Err("std.write failed".to_owned());
        }
        *self
            .buffered_value
            .lock()
            .map_err(|_| "std output buffer lock is poisoned".to_owned())? = Some(value.to_vec());
        Ok(())
    }

    pub fn output(&self) -> Result<(), String> {
        let result = unsafe { (self.output)() };
        match result {
            0 => Ok(()),
            -2 => Err("std.output() called before std.write() populated the global buffer".into()),
            _ => Err("std.output failed to write to the terminal".into()),
        }
    }

    pub fn input(&self) -> Result<String, String> {
        let mut length = 0;
        let value = unsafe { (self.input)(&mut length) };
        if value.is_null() {
            return Err("std.input failed to read from stdin".to_owned());
        }
        let bytes = unsafe { std::slice::from_raw_parts(value, length) }.to_vec();
        String::from_utf8(bytes)
            .map_err(|error| format!("std.input returned invalid UTF-8: {error}"))
    }

    pub fn buffered_value(&self) -> Result<String, String> {
        let bytes = self
            .buffered_value
            .lock()
            .map_err(|_| "std output buffer lock is poisoned".to_owned())?
            .clone()
            .ok_or_else(|| "std output buffer is not initialized".to_owned())?;
        String::from_utf8(bytes)
            .map_err(|error| format!("std output buffer contains invalid UTF-8: {error}"))
    }
}

impl Drop for StdLibrary {
    fn drop(&mut self) {
        unsafe { platform::close(self.handle) };
    }
}

unsafe fn load_symbol<T: Copy>(
    handle: *mut c_void,
    name: &'static [u8],
    path: &Path,
) -> Result<T, String> {
    let symbol = platform::symbol(handle, name.as_ptr().cast());
    if symbol.is_null() {
        return Err(format!(
            "std library {} is missing symbol {}",
            path.display(),
            String::from_utf8_lossy(&name[..name.len() - 1])
        ));
    }
    Ok(std::mem::transmute_copy(&symbol))
}

#[cfg(unix)]
mod platform {
    use super::{c_char, c_int, c_void};

    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    pub unsafe fn open(path: *const c_char) -> *mut c_void {
        dlopen(path, 2)
    }

    pub unsafe fn symbol(handle: *mut c_void, name: *const c_char) -> *mut c_void {
        dlsym(handle, name)
    }

    pub unsafe fn close(handle: *mut c_void) {
        dlclose(handle);
    }

    unsafe extern "C" {
        fn dlclose(handle: *mut c_void) -> c_int;
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{c_char, c_void};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(filename: *const u16) -> *mut c_void;
        fn GetProcAddress(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(handle: *mut c_void) -> i32;
    }

    pub unsafe fn open(path: *const u16) -> *mut c_void {
        LoadLibraryW(path)
    }

    pub unsafe fn symbol(handle: *mut c_void, name: *const c_char) -> *mut c_void {
        GetProcAddress(handle, name)
    }

    pub unsafe fn close(handle: *mut c_void) {
        FreeLibrary(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_declaration_file, validate_std_symbols};

    #[test]
    fn reads_std_extern_symbols_without_implementation_code() {
        let symbols = parse_declaration_file(
            "// Standard library\nnamespace std {\n    extern std write -> es_std_write\n    extern std input -> es_std_input\n    extern std output -> es_std_output\n}\n",
        )
        .unwrap();
        assert_eq!(
            symbols,
            [
                ("write".to_owned(), "es_std_write".to_owned()),
                ("input".to_owned(), "es_std_input".to_owned()),
                ("output".to_owned(), "es_std_output".to_owned()),
            ]
            .into_iter()
            .collect()
        );
        validate_std_symbols(&symbols).unwrap();
    }

    #[test]
    fn rejects_function_implementations_in_library_declarations() {
        let error =
            parse_declaration_file("namespace std {\n    def println(a) {}\n}\n").unwrap_err();
        assert!(error.contains("expected `extern`"));
    }
}
