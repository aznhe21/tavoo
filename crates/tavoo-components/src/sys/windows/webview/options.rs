// WebView2EnvironmentOptions.hのそれっぽい移植

use std::mem::MaybeUninit;

use webview2_com_sys::Microsoft::Web::WebView2::Win32 as WV2;
use windows::core::{self as C, implement, Result, PWSTR};
use windows::Win32::Foundation as F;

use super::com::{self, CoBox};
use super::patch;

macro_rules! prop_string {
    ($prop:ident, $getter:ident, $setter:ident) => {
        fn $getter(&self, value: *mut PWSTR) -> Result<()> {
            unsafe {
                *value.as_mut().ok_or(F::E_POINTER)? = com::to_pwstr(&*self.$prop)?;
                Ok(())
            }
        }

        fn $setter(&self, _: &C::PCWSTR) -> Result<()> {
            log::trace!(stringify!($setter));

            Err(F::E_NOTIMPL.into())
        }
    };
}

macro_rules! prop_bool {
    ($prop:ident, $getter:ident, $setter:ident) => {
        fn $getter(&self, value: *mut F::BOOL) -> Result<()> {
            unsafe {
                *value.as_mut().ok_or(F::E_POINTER)? = self.$prop.into();
                Ok(())
            }
        }

        fn $setter(&self, _: F::BOOL) -> Result<()> {
            log::trace!(stringify!($setter));

            Err(F::E_NOTIMPL.into())
        }
    };
}

#[derive(Debug)]
#[implement(WV2::ICoreWebView2CustomSchemeRegistration)]
pub struct CoreWebView2CustomSchemeRegistration {
    pub scheme_name: String,
    pub allowed_origins: Vec<String>,
    pub treat_as_secure: bool,
    pub has_authority_component: bool,
}

impl CoreWebView2CustomSchemeRegistration {
    /// `CoreWebView2CustomSchemeRegistration`を生成する。
    ///
    /// `Into::into`を使って`ICoreWebView2CustomSchemeRegistration`に変換する。
    #[inline]
    pub fn new<T: Into<String>>(scheme_name: T) -> CoreWebView2CustomSchemeRegistration {
        CoreWebView2CustomSchemeRegistration {
            scheme_name: scheme_name.into(),
            allowed_origins: Vec::new(),
            treat_as_secure: false,
            has_authority_component: false,
        }
    }
}

impl Default for CoreWebView2CustomSchemeRegistration {
    #[inline]
    fn default() -> CoreWebView2CustomSchemeRegistration {
        CoreWebView2CustomSchemeRegistration {
            scheme_name: String::new(),
            allowed_origins: Vec::new(),
            treat_as_secure: false,
            has_authority_component: false,
        }
    }
}

#[allow(non_snake_case)]
impl WV2::ICoreWebView2CustomSchemeRegistration_Impl for CoreWebView2CustomSchemeRegistration {
    fn SchemeName(&self, scheme_name: *mut PWSTR) -> Result<()> {
        unsafe {
            *scheme_name.as_mut().ok_or(F::E_POINTER)? = com::to_pwstr(&*self.scheme_name)?;
            Ok(())
        }
    }

    fn GetAllowedOrigins(
        &self,
        allowed_origins_count: *mut u32,
        allowed_origins: *mut *mut PWSTR,
    ) -> Result<()> {
        /// `PWSTR`の配列を構築するためのオブジェクト。
        ///
        /// 途中でエラーやパニックが発生した場合は自動で領域が解放される。
        struct Builder {
            array: CoBox<[MaybeUninit<PWSTR>]>,
            written: usize,
        }
        impl Builder {
            #[inline]
            fn new(len: usize) -> Result<Builder> {
                #[allow(unused_mut)]
                let mut array = CoBox::try_new_uninit_slice(len)?;
                #[cfg(debug_assertions)]
                array.fill(MaybeUninit::new(PWSTR::null()));

                Ok(Builder { array, written: 0 })
            }

            /// `idx`番目の値に`src`を書き込む。
            #[inline]
            unsafe fn write(&mut self, idx: usize, src: &str) -> Result<()> {
                debug_assert!(idx < self.array.len());

                self.array.get_unchecked_mut(idx).write(com::to_pwstr(src)?);
                self.written += 1;
                Ok(())
            }

            /// 書き込みを完了しポインタを返す。
            #[inline]
            fn finish(self) -> *mut PWSTR {
                unsafe {
                    debug_assert!(
                        self.array.iter().all(|v| !PWSTR::is_null(&*v.as_ptr())),
                        "未書き込み要素がある"
                    );
                }

                // Safety: selfはforgetするのでself.arrayを二重解放することは無い
                let array = unsafe { std::ptr::read(&self.array).assume_init() };
                std::mem::forget(self);
                CoBox::into_raw(array).cast()
            }
        }
        impl Drop for Builder {
            fn drop(&mut self) {
                for origin in &self.array[..self.written] {
                    // Safety: self.writtenまでは書き込み済み
                    unsafe {
                        let _ = CoBox::from_raw(origin.assume_init().0);
                    }
                }
            }
        }

        unsafe {
            let allowed_origins_count = allowed_origins_count.as_mut().ok_or(F::E_POINTER)?;
            let allowed_origins = allowed_origins.as_mut().ok_or(F::E_POINTER)?;

            *allowed_origins_count = 0;
            *allowed_origins = if self.allowed_origins.is_empty() {
                std::ptr::null_mut()
            } else {
                let mut builder = Builder::new(self.allowed_origins.len())?;
                for (i, src) in self.allowed_origins.iter().enumerate() {
                    builder.write(i, &**src)?;
                }
                let ptr = builder.finish();

                *allowed_origins_count = self.allowed_origins.len() as u32;
                ptr
            };

            Ok(())
        }
    }

    fn SetAllowedOrigins(&self, _: u32, _: *mut PWSTR) -> Result<()> {
        log::trace!("CoreWebView2EnvironmentOptions::SetAllowedOrigins");
        Err(F::E_NOTIMPL.into())
    }

    prop_bool!(treat_as_secure, TreatAsSecure, SetTreatAsSecure);
    prop_bool!(
        has_authority_component,
        HasAuthorityComponent,
        SetHasAuthorityComponent
    );
}

#[derive(Debug)]
#[implement(
    WV2::ICoreWebView2EnvironmentOptions,
    WV2::ICoreWebView2EnvironmentOptions2,
    WV2::ICoreWebView2EnvironmentOptions3,
    patch::ICoreWebView2EnvironmentOptions4
)]
pub struct CoreWebView2EnvironmentOptions {
    pub additional_browser_arguments: String,
    pub language: String,
    pub target_compatible_browser_version: String,
    pub allow_single_sign_on_using_os_primary_account: bool,
    pub exclusive_user_data_folder_access: bool,
    pub is_custom_crash_reporting_enabled: bool,
    pub custom_scheme_registrations: Vec<WV2::ICoreWebView2CustomSchemeRegistration>,
}

impl CoreWebView2EnvironmentOptions {
    /// `CoreWebView2EnvironmentOptions`を生成する。
    ///
    /// `Into::into`を使って`ICoreWebView2EnvironmentOptions`に変換する。
    #[inline]
    pub fn new() -> CoreWebView2EnvironmentOptions {
        CoreWebView2EnvironmentOptions {
            additional_browser_arguments: String::new(),
            language: String::new(),
            target_compatible_browser_version: String::from_utf16(unsafe {
                WV2::CORE_WEBVIEW_TARGET_PRODUCT_VERSION.as_wide()
            })
            .unwrap(),
            allow_single_sign_on_using_os_primary_account: false,
            exclusive_user_data_folder_access: false,
            is_custom_crash_reporting_enabled: false,
            custom_scheme_registrations: Vec::new(),
        }
    }
}

impl Default for CoreWebView2EnvironmentOptions {
    #[inline]
    fn default() -> CoreWebView2EnvironmentOptions {
        CoreWebView2EnvironmentOptions::new()
    }
}

#[allow(non_snake_case)]
impl WV2::ICoreWebView2EnvironmentOptions_Impl for CoreWebView2EnvironmentOptions {
    prop_string!(
        additional_browser_arguments,
        AdditionalBrowserArguments,
        SetAdditionalBrowserArguments
    );
    prop_string!(language, Language, SetLanguage);
    prop_string!(
        target_compatible_browser_version,
        TargetCompatibleBrowserVersion,
        SetTargetCompatibleBrowserVersion
    );
    prop_bool!(
        allow_single_sign_on_using_os_primary_account,
        AllowSingleSignOnUsingOSPrimaryAccount,
        SetAllowSingleSignOnUsingOSPrimaryAccount
    );
}

#[allow(non_snake_case)]
impl WV2::ICoreWebView2EnvironmentOptions2_Impl for CoreWebView2EnvironmentOptions {
    prop_bool!(
        exclusive_user_data_folder_access,
        ExclusiveUserDataFolderAccess,
        SetExclusiveUserDataFolderAccess
    );
}

#[allow(non_snake_case)]
impl WV2::ICoreWebView2EnvironmentOptions3_Impl for CoreWebView2EnvironmentOptions {
    prop_bool!(
        is_custom_crash_reporting_enabled,
        IsCustomCrashReportingEnabled,
        SetIsCustomCrashReportingEnabled
    );
}

#[allow(non_snake_case)]
impl patch::ICoreWebView2EnvironmentOptions4_Impl for CoreWebView2EnvironmentOptions {
    fn GetCustomSchemeRegistrations(
        &self,
        count: *mut u32,
        scheme_registrations: *mut *mut Option<WV2::ICoreWebView2CustomSchemeRegistration>,
    ) -> Result<()> {
        unsafe {
            let count = count.as_mut().ok_or(F::E_POINTER)?;
            let scheme_registrations = scheme_registrations.as_mut().ok_or(F::E_POINTER)?;

            *count = 0;
            if self.custom_scheme_registrations.is_empty() {
                *scheme_registrations = std::ptr::null_mut();
                return Ok(());
            }

            let mut array = CoBox::try_new_uninit_slice(self.custom_scheme_registrations.len())?;
            for (src, dst) in self.custom_scheme_registrations.iter().zip(&mut *array) {
                dst.write(Some(src.clone()));
            }

            *scheme_registrations = CoBox::into_raw(array.assume_init()).cast();
            *count = self.custom_scheme_registrations.len() as u32;
            Ok(())
        }
    }

    fn SetCustomSchemeRegistrations(
        &self,
        _: &[Option<WV2::ICoreWebView2CustomSchemeRegistration>],
    ) -> Result<()> {
        log::trace!("CoreWebView2EnvironmentOptions::SetCustomSchemeRegistrations");
        Err(F::E_NOTIMPL.into())
    }
}
