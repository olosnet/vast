fn main() {
    let version_str = unsafe {

        let version = lvast::bindings::svb::SVBGetSDKVersion();

        std::ffi::CStr::from_ptr(version)
            .to_str()
            .unwrap_or("unknown")
    };

    println!("SVB SDK Version: {}", version_str);
}