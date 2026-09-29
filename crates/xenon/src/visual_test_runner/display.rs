// Screenshots must not depend on the attached display. Window size is fixed by
// each scene; the scale factor is not: GPUI reads `NSScreen.backingScaleFactor`
// of the window's screen, and even an off-screen window is assigned one. Pin
// it for this process so a 1x monitor renders the same 2x pixels as Retina.

use std::ffi::{CStr, c_void};

/// Baselines are 2x: 1280x800pt scenes are 2560x1600 PNGs.
const SCALE: f64 = 2.0;

unsafe extern "C" {
    fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
    fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
    fn class_getInstanceMethod(class: *mut c_void, selector: *mut c_void) -> *mut c_void;
    fn method_setImplementation(method: *mut c_void, imp: *const c_void) -> *const c_void;
}

extern "C" fn pinned_scale(_screen: *mut c_void, _selector: *mut c_void) -> f64 {
    SCALE
}

/// Call before any window opens.
pub(crate) fn pin_scale_factor() -> anyhow::Result<()> {
    const CLASS: &CStr = c"NSScreen";
    const SELECTOR: &CStr = c"backingScaleFactor";
    // SAFETY: plain Objective-C runtime lookups; the replacement has the
    // `(id, SEL) -> CGFloat` signature of the method it replaces.
    unsafe {
        let class = objc_getClass(CLASS.as_ptr());
        anyhow::ensure!(!class.is_null(), "NSScreen class missing");
        let method = class_getInstanceMethod(class, sel_registerName(SELECTOR.as_ptr()));
        anyhow::ensure!(!method.is_null(), "NSScreen.backingScaleFactor missing");
        method_setImplementation(method, pinned_scale as *const c_void);
    }
    Ok(())
}
