//! Minimal stable LibreOfficeKit ABI, matched to the bundled 26.2.6.3 header.
use jni::{
    objects::{JByteBuffer, JClass, JString},
    sys::jobject,
    JNIEnv,
};
use std::ffi::{c_char, c_void, CString};
#[repr(C)]
struct Office {
    class: *const OfficeClass,
}
#[repr(C)]
struct OfficeClass {
    size: usize,
    destroy: *const c_void,
    load: *const c_void,
    get_error: *const c_void,
    load_options: unsafe extern "C" fn(*mut Office, *const c_char, *const c_char) -> *mut c_void,
}
#[no_mangle]
pub extern "system" fn Java_cn_materialbox_app_Core_loadOffice(
    mut env: JNIEnv,
    _class: JClass,
    handle: JByteBuffer,
    url: JString,
    options: JString,
) -> jobject {
    let result = (|| {
        let address = env.get_direct_buffer_address(&handle).ok()?;
        if address.is_null()
            || env.get_direct_buffer_capacity(&handle).ok()? < std::mem::size_of::<Office>()
        {
            return None;
        }
        let url: CString =
            CString::new(env.get_string(&url).ok()?.to_string_lossy().as_bytes()).ok()?;
        let options =
            CString::new(env.get_string(&options).ok()?.to_string_lossy().as_bytes()).ok()?;
        // The buffer originates solely from LibreOfficeKit.getLibreOfficeKitHandle().
        unsafe {
            let office = address.cast::<Office>();
            let class = (*office).class;
            if class.is_null() || (*class).size < std::mem::size_of::<OfficeClass>() {
                return None;
            }
            let document = ((*class).load_options)(office, url.as_ptr(), options.as_ptr());
            if document.is_null() {
                return None;
            }
            env.new_direct_byte_buffer(document.cast::<u8>(), std::mem::size_of::<*mut c_void>())
                .ok()
                .map(|b| b.into_raw())
        }
    })();
    result.unwrap_or(std::ptr::null_mut())
}
