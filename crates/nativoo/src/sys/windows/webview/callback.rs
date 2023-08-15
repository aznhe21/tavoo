//! webview2-comにおけるコールバックの定義が気に入らないので独自に定義する。
//!
//! 全部を定義するのは面倒なので必要に応じて定義することにする。
#![allow(non_snake_case)]

use webview2_com_sys::Microsoft::Web::WebView2::Win32::*;
use windows::core::IUnknown;

macro_rules! event_callback {
    ($name:ident, $intf:ty, $impl:ty, $($arg_name:ident: $arg_type:ty,)*) => {
        #[doc = concat!("クロージャから[`", stringify!($intf), "`]を生成する。")]
        pub fn $name<F>(f: F) -> $intf
        where
            F: ::core::ops::FnMut($($arg_type),*) -> ::windows::core::Result<()> + 'static,
        {
            #[::windows::core::implement($intf)]
            struct Handler(
                ::parking_lot::Mutex<
                    ::std::boxed::Box<
                        dyn ::core::ops::FnMut($($arg_type),*) -> ::windows::core::Result<()>
                    >
                >,
            );
            impl $impl for Handler {
                fn Invoke(&self, $($arg_name: $arg_type),*) -> ::windows::core::Result<()> {
                    (self.0.lock())($($arg_name),*)
                }
            }

            Handler(::parking_lot::Mutex::new(::std::boxed::Box::new(f))).into()
        }
    };
}

macro_rules! completed_callback {
    ($handler:ident, $intf:ty, $impl:ty, fn($value:ident: $invoke:ty) -> Result<$fn:ty> { $($tt:tt)* }) => {
        #[doc = concat!("クロージャから[`", stringify!($intf), "`]を生成する。")]
        pub fn $handler<F>(f: F) -> $intf
        where
            F: ::core::ops::FnOnce(::windows::core::Result<$fn>) -> ::windows::core::Result<()>
                + 'static,
        {
            #[::windows::core::implement($intf)]
            struct Handler(
                ::parking_lot::Mutex<
                    ::core::option::Option<
                        ::std::boxed::Box<
                            dyn ::core::ops::FnOnce(
                                ::windows::core::Result<$fn>,
                            )
                                -> ::windows::core::Result<()>,
                        >,
                    >,
                >,
            );
            impl $impl for Handler {
                fn Invoke(
                    &self,
                    errorcode: ::windows::core::HRESULT,
                    value: $invoke,
                ) -> ::windows::core::Result<()> {
                    match self.0.lock().take() {
                        Some(f) => f(errorcode.ok().and_then(|()| {
                            fn map($value: $invoke) -> ::windows::core::Result<$fn> {
                                $($tt)*
                            }
                            map(value)
                        })),
                        None => {
                            log::trace!(concat!(stringify!($intf), "が二度呼ばれた"));
                            Err(::windows::Win32::Foundation::E_UNEXPECTED.into())
                        }
                    }
                }
            }

            Handler(::parking_lot::Mutex::new(Some(::std::boxed::Box::new(f)))).into()
        }
    };
}

event_callback!(
    navigation_starting_event_handler,
    ICoreWebView2NavigationStartingEventHandler,
    ICoreWebView2NavigationStartingEventHandler_Impl,
    sender: ::core::option::Option<&ICoreWebView2>,
    args: ::core::option::Option<&ICoreWebView2NavigationStartingEventArgs>,
);

event_callback!(
    navigation_completed_event_handler,
    ICoreWebView2NavigationCompletedEventHandler,
    ICoreWebView2NavigationCompletedEventHandler_Impl,
    sender: ::core::option::Option<&ICoreWebView2>,
    args: ::core::option::Option<&ICoreWebView2NavigationCompletedEventArgs>,
);

event_callback!(
    web_resource_requested_event_handler,
    ICoreWebView2WebResourceRequestedEventHandler,
    ICoreWebView2WebResourceRequestedEventHandler_Impl,
    sender: ::core::option::Option<&ICoreWebView2>,
    args: ::core::option::Option<&ICoreWebView2WebResourceRequestedEventArgs>,
);

event_callback!(
    document_title_changed_event_handler,
    ICoreWebView2DocumentTitleChangedEventHandler,
    ICoreWebView2DocumentTitleChangedEventHandler_Impl,
    sender: ::core::option::Option<&ICoreWebView2>,
    args: ::core::option::Option<&IUnknown>,
);

event_callback!(
    web_message_received_event_handler,
    ICoreWebView2WebMessageReceivedEventHandler,
    ICoreWebView2WebMessageReceivedEventHandler_Impl,
    sender: ::core::option::Option<&ICoreWebView2>,
    args: ::core::option::Option<&ICoreWebView2WebMessageReceivedEventArgs>,
);

completed_callback!(
    environment_completed_handler,
    ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler,
    ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_Impl,
    fn(value: ::core::option::Option<&ICoreWebView2Environment>) -> Result<&ICoreWebView2Environment> {
        value.ok_or(::windows::Win32::Foundation::E_POINTER.into())
    }
);

completed_callback!(
    controller_completed_handler,
    ICoreWebView2CreateCoreWebView2ControllerCompletedHandler,
    ICoreWebView2CreateCoreWebView2ControllerCompletedHandler_Impl,
    fn(value: ::core::option::Option<&ICoreWebView2Controller>) -> Result<&ICoreWebView2Controller> {
        value.ok_or(::windows::Win32::Foundation::E_POINTER.into())
    }
);

completed_callback!(
    add_script_to_execute_on_document_created_completed_handler,
    ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler_Impl,
    fn(value: &::windows::core::PCWSTR) -> Result<&::windows::core::PCWSTR> {
        Ok(value)
    }
);
