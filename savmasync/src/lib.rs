#[cfg_attr(windows, link(name = "savmasync", kind = "raw-dylib"))]
#[cfg_attr(not(windows), link(name = "savmasync", kind = "dylib"))]
unsafe extern "C" {
  pub unsafe fn register(id: u8, f: Fn);
  pub unsafe fn unregister(id: u8, f: Fn);
  pub safe fn init();
}

pub type Fn = extern "C" fn() -> bool;

pub struct Reactor(u8, Fn);

impl Reactor {
  pub const fn new(id: u8, f: Fn) -> Self {
    Self(id, f)
  }

  // Runtime registration call
  pub unsafe fn register(&self) {
    unsafe {
      register(self.0, self.1);
    }
  }

  pub const unsafe fn id(&self) -> u8 {
    self.0
  }

  pub const unsafe fn fnarg(&self) -> Fn {
    self.1
  }
}

#[macro_export]
/// Generates the GLUE for interacting with SaVM's Async Thread library!
macro_rules! generate {
  (
    $( $instance:ident => ($id:expr, $callback:expr) ),* $(,)?
  ) => {
    $(
      pub static $instance: $crate::Reactor = unsafe {
        $crate::Reactor::new($id, $callback)
      };
    )*

    // Linux
    #[cfg(target_os = "linux")]
    #[unsafe(link_section = ".init_array")]
    pub static INIT_FN: extern "C" fn() = setup_fn;

    #[cfg(target_os = "linux")]
    #[unsafe(link_section = ".fini_array")]
    pub static DESTROY_FN: extern "C" fn() = cleanup_fn;

    // macOS
    #[cfg(target_os = "macos")]
    #[unsafe(link_section = "__DATA,__mod_init_func")]
    pub static INIT_FN: extern "C" fn() = setup_fn;

    #[cfg(target_os = "macos")]
    #[unsafe(link_section = "__DATA,__mod_term_func")]
    pub static DESTROY_FN: extern "C" fn() = cleanup_fn;

    // Windows
    #[cfg(target_os = "windows")]
    #[unsafe(no_mangle)]
    pub extern "system" fn DllMain(
      _hinst_dll: *mut std::ffi::c_void,
      fdw_reason: u32,
      _lp_reserved: *mut std::ffi::c_void,
    ) -> i32 {
      const DLL_PROCESS_ATTACH: u32 = 1;
      const DLL_PROCESS_DETACH: u32 = 0;

      if fdw_reason == DLL_PROCESS_ATTACH {
        setup_fn();
      } else if fdw_reason == DLL_PROCESS_DETACH {
        cleanup_fn();
      }
      1 // Success
    }

    // Called automatically when library maps into memory
    extern "C" fn setup_fn() {
      unsafe {
        $(
          $instance.register();
        )*
      }
    }

    // Called automatically during dlclose/FreeLibrary
    extern "C" fn cleanup_fn() {
      unsafe {
        $(
          let id = $instance.id();
          let arg = $instance.fnarg();
          $crate::unregister(id, arg);
        )*
      }
    }
  };
}
