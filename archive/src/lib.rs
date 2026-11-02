use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

const ARCHIVE_EOF: c_int = 1;
const FILE_TYPE_REGULAR: c_int = 0o100000;
const MAX_ENTRY_SIZE: i64 = 128 * 1024 * 1024;
const MAX_ARCHIVE_SIZE: usize = 256 * 1024 * 1024;
const MAX_ENTRY_COUNT: usize = 16;

type ReadNew = unsafe extern "C" fn() -> *mut c_void;
type ReadSupport = unsafe extern "C" fn(*mut c_void) -> c_int;
type ReadOpenFilename = unsafe extern "C" fn(*mut c_void, *const c_char, usize) -> c_int;
type ReadNextHeader = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int;
type EntryPathname = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type EntryFiletype = unsafe extern "C" fn(*mut c_void) -> c_int;
type EntrySize = unsafe extern "C" fn(*mut c_void) -> i64;
type ReadData = unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> isize;
type ReadFree = unsafe extern "C" fn(*mut c_void) -> c_int;
type WriteNew = unsafe extern "C" fn() -> *mut c_void;
type WriteSetFormat = unsafe extern "C" fn(*mut c_void) -> c_int;
type WriteOpenFilename = unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int;
type EntryNew = unsafe extern "C" fn() -> *mut c_void;
type EntrySetPathname = unsafe extern "C" fn(*mut c_void, *const c_char);
type EntrySetFiletype = unsafe extern "C" fn(*mut c_void, c_int);
type EntrySetPerm = unsafe extern "C" fn(*mut c_void, c_int);
type EntrySetSize = unsafe extern "C" fn(*mut c_void, i64);
type WriteHeader = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;
type WriteData = unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> isize;
type EntryFree = unsafe extern "C" fn(*mut c_void);
type WriteClose = unsafe extern "C" fn(*mut c_void) -> c_int;
type WriteFree = unsafe extern "C" fn(*mut c_void) -> c_int;
type ErrorString = unsafe extern "C" fn(*mut c_void) -> *const c_char;

struct Api {
    _library: Library,
    read_new: ReadNew,
    read_support_filter_all: ReadSupport,
    read_support_format_tar: ReadSupport,
    read_open_filename: ReadOpenFilename,
    read_next_header: ReadNextHeader,
    entry_pathname: EntryPathname,
    entry_filetype: EntryFiletype,
    entry_size: EntrySize,
    read_data: ReadData,
    read_free: ReadFree,
    write_new: WriteNew,
    write_set_format_ustar: WriteSetFormat,
    write_open_filename: WriteOpenFilename,
    entry_new: EntryNew,
    entry_set_pathname: EntrySetPathname,
    entry_set_filetype: EntrySetFiletype,
    entry_set_perm: EntrySetPerm,
    entry_set_size: EntrySetSize,
    write_header: WriteHeader,
    write_data: WriteData,
    entry_free: EntryFree,
    write_close: WriteClose,
    write_free: WriteFree,
    error_string: ErrorString,
}

impl Api {
    fn load() -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        let names = ["libarchive.so.13", "libarchive.so"];
        #[cfg(target_os = "windows")]
        let names = ["archive.dll", "libarchive.dll"];
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        return Err("libarchive loading is supported only on Linux and Windows".to_owned());

        let mut last_error = String::new();
        for name in names {
            let library = match unsafe { Library::new(name) } {
                Ok(library) => library,
                Err(error) => {
                    last_error = error.to_string();
                    continue;
                }
            };
            macro_rules! symbol {
                ($symbol:literal, $kind:ty) => {
                    match unsafe { library.get::<$kind>(concat!($symbol, "\0").as_bytes()) } {
                        Ok(value) => *value,
                        Err(error) => {
                            last_error = error.to_string();
                            break;
                        }
                    }
                };
            }
            let api = Self {
                read_new: symbol!("archive_read_new", ReadNew),
                read_support_filter_all: symbol!("archive_read_support_filter_all", ReadSupport),
                read_support_format_tar: symbol!("archive_read_support_format_tar", ReadSupport),
                read_open_filename: symbol!("archive_read_open_filename", ReadOpenFilename),
                read_next_header: symbol!("archive_read_next_header", ReadNextHeader),
                entry_pathname: symbol!("archive_entry_pathname", EntryPathname),
                entry_filetype: symbol!("archive_entry_filetype", EntryFiletype),
                entry_size: symbol!("archive_entry_size", EntrySize),
                read_data: symbol!("archive_read_data", ReadData),
                read_free: symbol!("archive_read_free", ReadFree),
                write_new: symbol!("archive_write_new", WriteNew),
                write_set_format_ustar: symbol!("archive_write_set_format_ustar", WriteSetFormat),
                write_open_filename: symbol!("archive_write_open_filename", WriteOpenFilename),
                entry_new: symbol!("archive_entry_new", EntryNew),
                entry_set_pathname: symbol!("archive_entry_set_pathname", EntrySetPathname),
                entry_set_filetype: symbol!("archive_entry_set_filetype", EntrySetFiletype),
                entry_set_perm: symbol!("archive_entry_set_perm", EntrySetPerm),
                entry_set_size: symbol!("archive_entry_set_size", EntrySetSize),
                write_header: symbol!("archive_write_header", WriteHeader),
                write_data: symbol!("archive_write_data", WriteData),
                entry_free: symbol!("archive_entry_free", EntryFree),
                write_close: symbol!("archive_write_close", WriteClose),
                write_free: symbol!("archive_write_free", WriteFree),
                error_string: symbol!("archive_error_string", ErrorString),
                _library: library,
            };
            return Ok(api);
        }
        Err(format!("could not load libarchive: {last_error}"))
    }

    fn error(&self, archive: *mut c_void, action: &str) -> String {
        let detail = unsafe {
            let pointer = (self.error_string)(archive);
            if pointer.is_null() {
                "unknown libarchive error".to_owned()
            } else {
                CStr::from_ptr(pointer).to_string_lossy().into_owned()
            }
        };
        format!("{action}: {detail}")
    }
}

pub struct TarEntry {
    pub pathname: String,
    pub is_regular_file: bool,
    pub data: Vec<u8>,
}

pub fn read_tar(path: &Path) -> Result<Vec<TarEntry>, String> {
    let api = Api::load()?;
    let path = path_cstring(path)?;
    let archive = unsafe { (api.read_new)() };
    if archive.is_null() {
        return Err("libarchive could not allocate a tar reader".to_owned());
    }
    let result = (|| {
        for enable in [api.read_support_filter_all, api.read_support_format_tar] {
            let status = unsafe { enable(archive) };
            if status < 0 {
                return Err(api.error(archive, "could not enable tar reader"));
            }
        }
        let status = unsafe { (api.read_open_filename)(archive, path.as_ptr(), 10_240) };
        if status < 0 {
            return Err(api.error(archive, "could not open tar archive"));
        }
        let mut entries = Vec::new();
        let mut total_size = 0usize;
        loop {
            let mut entry = std::ptr::null_mut();
            let status = unsafe { (api.read_next_header)(archive, &mut entry) };
            if status == ARCHIVE_EOF {
                break;
            }
            if status < 0 {
                return Err(api.error(archive, "could not read tar header"));
            }
            if entries.len() == MAX_ENTRY_COUNT {
                return Err("tar archive contains too many entries".to_owned());
            }
            if entry.is_null() {
                return Err("libarchive returned an empty tar entry".to_owned());
            }
            let pathname = unsafe {
                let pointer = (api.entry_pathname)(entry);
                if pointer.is_null() {
                    return Err("tar entry has no pathname".to_owned());
                }
                CStr::from_ptr(pointer)
                    .to_str()
                    .map_err(|_| "tar entry pathname is not UTF-8".to_owned())?
                    .to_owned()
            };
            let size = unsafe { (api.entry_size)(entry) };
            if !(0..=MAX_ENTRY_SIZE).contains(&size) {
                return Err(format!("tar entry `{pathname}` has an invalid size"));
            }
            total_size = total_size
                .checked_add(size as usize)
                .filter(|total| *total <= MAX_ARCHIVE_SIZE)
                .ok_or_else(|| "tar archive exceeds the maximum uncompressed size".to_owned())?;
            let mut data = Vec::with_capacity(size as usize);
            let mut buffer = [0u8; 16 * 1024];
            loop {
                let count =
                    unsafe { (api.read_data)(archive, buffer.as_mut_ptr().cast(), buffer.len()) };
                if count < 0 {
                    return Err(
                        api.error(archive, &format!("could not read tar entry `{pathname}`"))
                    );
                }
                if count == 0 {
                    break;
                }
                data.extend_from_slice(&buffer[..count as usize]);
                if data.len() > size as usize {
                    return Err(format!("tar entry `{pathname}` exceeds its declared size"));
                }
            }
            if data.len() != size as usize {
                return Err(format!("tar entry `{pathname}` is truncated"));
            }
            entries.push(TarEntry {
                pathname,
                is_regular_file: unsafe { (api.entry_filetype)(entry) } == FILE_TYPE_REGULAR,
                data,
            });
        }
        Ok(entries)
    })();
    unsafe {
        (api.read_free)(archive);
    }
    result
}

pub fn write_tar(path: &Path, entries: &[(&str, &[u8])]) -> Result<(), String> {
    let api = Api::load()?;
    let path = path_cstring(path)?;
    let archive = unsafe { (api.write_new)() };
    if archive.is_null() {
        return Err("libarchive could not allocate a tar writer".to_owned());
    }
    let result = (|| {
        if unsafe { (api.write_set_format_ustar)(archive) } < 0 {
            return Err(api.error(archive, "could not select tar format"));
        }
        if unsafe { (api.write_open_filename)(archive, path.as_ptr()) } < 0 {
            return Err(api.error(archive, "could not open tar output"));
        }
        for (pathname, data) in entries {
            let pathname = CString::new(*pathname)
                .map_err(|_| "tar entry pathname contains a null byte".to_owned())?;
            let entry = unsafe { (api.entry_new)() };
            if entry.is_null() {
                return Err("libarchive could not allocate a tar entry".to_owned());
            }
            unsafe {
                (api.entry_set_pathname)(entry, pathname.as_ptr());
                (api.entry_set_filetype)(entry, FILE_TYPE_REGULAR);
                (api.entry_set_perm)(entry, 0o644);
                (api.entry_set_size)(entry, data.len() as i64);
            }
            let status = unsafe { (api.write_header)(archive, entry) };
            unsafe { (api.entry_free)(entry) };
            if status < 0 {
                return Err(api.error(archive, "could not write tar header"));
            }
            let mut written = 0;
            while written < data.len() {
                let count = unsafe {
                    (api.write_data)(
                        archive,
                        data[written..].as_ptr().cast(),
                        data.len() - written,
                    )
                };
                if count <= 0 {
                    return Err(api.error(archive, "could not write tar entry data"));
                }
                written += count as usize;
            }
        }
        if unsafe { (api.write_close)(archive) } < 0 {
            return Err(api.error(archive, "could not close tar archive"));
        }
        Ok(())
    })();
    unsafe {
        (api.write_free)(archive);
    }
    result
}

fn path_cstring(path: &Path) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| format!("path contains a null byte: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{read_tar, write_tar};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_and_reads_root_tar_entries() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "easyscript-archive-test-{}-{nonce}.tar",
            std::process::id()
        ));
        write_tar(&path, &[("libes_std.so", b"so"), ("es_std.dll", b"dll")]).unwrap();
        let entries = read_tar(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].pathname, "libes_std.so");
        assert!(entries[0].is_regular_file);
        assert_eq!(entries[0].data, b"so");
        assert_eq!(entries[1].pathname, "es_std.dll");
        assert_eq!(entries[1].data, b"dll");
        fs::remove_file(path).expect("remove tar fixture");
    }
}
