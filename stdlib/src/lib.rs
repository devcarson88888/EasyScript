use std::io::{self, BufRead, Write};
use std::slice;
use std::sync::Mutex;

static OUTPUT_BUFFER: Mutex<Option<Vec<u8>>> = Mutex::new(None);
thread_local! {
    static INPUT_BUFFER: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[no_mangle]
pub unsafe extern "C" fn es_std_write(value: *const u8, length: usize) -> i32 {
    if value.is_null() && length != 0 {
        return -1;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        slice::from_raw_parts(value, length)
    };
    match OUTPUT_BUFFER.lock() {
        Ok(mut buffer) => {
            *buffer = Some(bytes.to_vec());
            0
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn es_std_output() -> i32 {
    let buffer = match OUTPUT_BUFFER.lock() {
        Ok(buffer) => match buffer.as_ref() {
            Some(buffer) => buffer.clone(),
            None => return -2,
        },
        Err(_) => return -1,
    };
    let mut stdout = io::stdout().lock();
    if stdout.write_all(&buffer).is_err()
        || stdout.write_all(b"\n").is_err()
        || stdout.flush().is_err()
    {
        return -1;
    }
    0
}

#[no_mangle]
pub extern "C" fn es_std_input(length: *mut usize) -> *mut u8 {
    if length.is_null() {
        return std::ptr::null_mut();
    }
    let mut input = String::new();
    if io::stdin().lock().read_line(&mut input).is_err() {
        return std::ptr::null_mut();
    }
    if input.ends_with('\n') {
        input.pop();
        if input.ends_with('\r') {
            input.pop();
        }
    }
    unsafe {
        *length = input.len();
    }
    INPUT_BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        buffer.clear();
        buffer.extend_from_slice(input.as_bytes());
        buffer.as_mut_ptr()
    })
}

#[cfg(test)]
mod tests {
    use super::{es_std_output, es_std_write};

    #[test]
    fn write_and_output_use_the_library_owned_buffer() {
        assert_eq!(es_std_output(), -2);
        let value = b"library buffer";
        assert_eq!(unsafe { es_std_write(value.as_ptr(), value.len()) }, 0);
    }
}
