//! webview2-com-sysで定義が間違っているものをこちらで再定義する。

#![allow(
    non_snake_case,
    non_camel_case_types,
    dead_code,
    unsafe_op_in_unsafe_fn
)]

use webview2_com_sys::Microsoft::Web::WebView2::Win32::ICoreWebView2CustomSchemeRegistration;

#[repr(transparent)]
pub struct ICoreWebView2EnvironmentOptions4(::windows::core::IUnknown);
impl ICoreWebView2EnvironmentOptions4 {
    pub unsafe fn GetCustomSchemeRegistrations(
        &self,
        count: *mut u32,
        schemeregistrations: *mut *mut ::core::option::Option<
            ICoreWebView2CustomSchemeRegistration,
        >,
    ) -> ::windows::core::Result<()> {
        (::windows::core::Interface::vtable(self).GetCustomSchemeRegistrations)(
            ::windows::core::Interface::as_raw(self),
            count,
            schemeregistrations,
        )
        .ok()
    }
    pub unsafe fn SetCustomSchemeRegistrations<P0>(
        &self,
        schemeregistrations: &[::core::option::Option<ICoreWebView2CustomSchemeRegistration>],
    ) -> ::windows::core::Result<()> {
        (::windows::core::Interface::vtable(self).SetCustomSchemeRegistrations)(
            ::windows::core::Interface::as_raw(self),
            schemeregistrations.len() as _,
            ::core::mem::transmute(schemeregistrations.as_ptr()),
        )
        .ok()
    }
}
::windows::imp::interface_hierarchy!(ICoreWebView2EnvironmentOptions4, ::windows::core::IUnknown);
impl ::core::cmp::PartialEq for ICoreWebView2EnvironmentOptions4 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl ::core::cmp::Eq for ICoreWebView2EnvironmentOptions4 {}
impl ::core::fmt::Debug for ICoreWebView2EnvironmentOptions4 {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_tuple("ICoreWebView2EnvironmentOptions4")
            .field(&self.0)
            .finish()
    }
}
unsafe impl ::windows::core::Interface for ICoreWebView2EnvironmentOptions4 {
    type Vtable = ICoreWebView2EnvironmentOptions4_Vtbl;
}
impl ::core::clone::Clone for ICoreWebView2EnvironmentOptions4 {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
unsafe impl ::windows::core::ComInterface for ICoreWebView2EnvironmentOptions4 {
    const IID: ::windows::core::GUID =
        ::windows::core::GUID::from_u128(0xac52d13f_0d38_475a_9dca_876580d6793e);
}
#[repr(C)]
#[doc(hidden)]
pub struct ICoreWebView2EnvironmentOptions4_Vtbl {
    pub base__: ::windows::core::IUnknown_Vtbl,
    pub GetCustomSchemeRegistrations: unsafe extern "system" fn(
        this: *mut ::core::ffi::c_void,
        count: *mut u32,
        schemeregistrations: *mut *mut ::core::option::Option<ICoreWebView2CustomSchemeRegistration>,
    ) -> ::windows::core::HRESULT,
    pub SetCustomSchemeRegistrations: unsafe extern "system" fn(
        this: *mut ::core::ffi::c_void,
        count: u32,
        schemeregistrations: *mut *mut ::core::ffi::c_void,
    ) -> ::windows::core::HRESULT,
}

pub trait ICoreWebView2EnvironmentOptions4_Impl: Sized {
    fn GetCustomSchemeRegistrations(
        &self,
        count: *mut u32,
        schemeregistrations: *mut *mut ::core::option::Option<
            ICoreWebView2CustomSchemeRegistration,
        >,
    ) -> ::windows::core::Result<()>;
    fn SetCustomSchemeRegistrations(
        &self,
        schemeregistrations: &[::core::option::Option<ICoreWebView2CustomSchemeRegistration>],
    ) -> ::windows::core::Result<()>;
}
impl ::windows::core::RuntimeName for ICoreWebView2EnvironmentOptions4 {}
impl ICoreWebView2EnvironmentOptions4_Vtbl {
    pub const fn new<
        Identity: ::windows::core::IUnknownImpl<Impl = Impl>,
        Impl: ICoreWebView2EnvironmentOptions4_Impl,
        const OFFSET: isize,
    >() -> ICoreWebView2EnvironmentOptions4_Vtbl {
        unsafe extern "system" fn GetCustomSchemeRegistrations<
            Identity: ::windows::core::IUnknownImpl<Impl = Impl>,
            Impl: ICoreWebView2EnvironmentOptions4_Impl,
            const OFFSET: isize,
        >(
            this: *mut ::core::ffi::c_void,
            count: *mut u32,
            schemeregistrations: *mut *mut ::core::option::Option<
                ICoreWebView2CustomSchemeRegistration,
            >,
        ) -> ::windows::core::HRESULT {
            let this = (this as *const *const ()).offset(OFFSET) as *const Identity;
            let this = (*this).get_impl();
            this.GetCustomSchemeRegistrations(
                ::core::mem::transmute_copy(&count),
                ::core::mem::transmute_copy(&schemeregistrations),
            )
            .into()
        }
        unsafe extern "system" fn SetCustomSchemeRegistrations<
            Identity: ::windows::core::IUnknownImpl<Impl = Impl>,
            Impl: ICoreWebView2EnvironmentOptions4_Impl,
            const OFFSET: isize,
        >(
            this: *mut ::core::ffi::c_void,
            count: u32,
            schemeregistrations: *mut *mut ::core::ffi::c_void,
        ) -> ::windows::core::HRESULT {
            let this = (this as *const *const ()).offset(OFFSET) as *const Identity;
            let this = (*this).get_impl();
            this.SetCustomSchemeRegistrations(::core::slice::from_raw_parts(
                ::core::mem::transmute_copy(&schemeregistrations),
                count as _,
            ))
            .into()
        }
        Self {
            base__: ::windows::core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            GetCustomSchemeRegistrations: GetCustomSchemeRegistrations::<Identity, Impl, OFFSET>,
            SetCustomSchemeRegistrations: SetCustomSchemeRegistrations::<Identity, Impl, OFFSET>,
        }
    }
    pub fn matches(iid: &windows::core::GUID) -> bool {
        iid == &<ICoreWebView2EnvironmentOptions4 as ::windows::core::ComInterface>::IID
    }
}
