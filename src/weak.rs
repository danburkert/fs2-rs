// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Support for "weak linkage" to symbols on Unix
//!
//! Some I/O operations we do in libstd require newer versions of OSes but we
//! need to maintain binary compatibility with older releases for now. In order
//! to use the new functionality when available we use this module for
//! detection.
//!
//! One option to use here is weak linkage, but that is unfortunately only
//! really workable on Linux. Hence, use dlsym to get the symbol value at
//! runtime. This is also done for compatibility with older versions of glibc,
//! and to avoid creating dependencies on GLIBC_PRIVATE symbols. It assumes that
//! we've been dynamically linked to the library the symbol comes from, but that
//! is currently always the case for things like libpthread/libc.
//!
//! A long time ago this used weak linkage for the __pthread_get_minstack
//! symbol, but that caused Debian to detect an unnecessarily strict versioned
//! dependency on libc6 (#23628).

extern crate libc;

use std::ffi::CString;
use std::marker;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};

#[macro_export]
macro_rules! weak {
    (fn $name:ident($($t:ty),*) -> $ret:ty) => (
        /* static */ $name: ::weak::Weak<unsafe extern fn($($t),*) -> $ret> =
            ::weak::Weak::new(stringify!($name));
    )
}

pub struct Weak<F> {
    name: &'static str,
    addr: AtomicUsize,
    _marker: marker::PhantomData<F>,
}

// Remove the const here and the static in the macro because "const fn" isn't available outside of
// nightly yet (see RFC 911 and issue 24111).  But then we run into type ascription not being
// implemented.
impl<F> Weak<F> {
    pub /* const */ fn new(name: &'static str) -> Weak<F> {
        Weak {
            name: name,
            addr: AtomicUsize::new(1),
            _marker: marker::PhantomData,
        }
    }

    pub fn get(&self) -> Option<&F> {
        assert_eq!(mem::size_of::<F>(), mem::size_of::<usize>());
        unsafe {
            if self.addr.load(Ordering::SeqCst) == 1 {
                self.addr.store(fetch(self.name), Ordering::SeqCst);
            }
            if self.addr.load(Ordering::SeqCst) == 0 {
                None
            } else {
                mem::transmute::<&AtomicUsize, Option<&F>>(&self.addr)
            }
        }
    }
}

unsafe fn fetch(name: &str) -> usize {
    let name = match CString::new(name) {
        Ok(cstr) => cstr,
        Err(..) => return 0,
    };
    libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) as usize
}
