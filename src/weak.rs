// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Provide a "WeakFlock" struct that wraps a possibly nonexistent flock() libc function.
//!
//! This borrows from libstd/sys/unix/weak.rs, removing the generic aspects that are available only
//! in a nightly build.

extern crate libc;

use std::ffi::CString;
use std::marker;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type FlockFunc = unsafe extern fn(libc::c_int, libc::c_int) -> libc::c_int;

pub struct WeakFlock {
    name: &'static str,
    addr: AtomicUsize,
    _marker: marker::PhantomData<FlockFunc>,
}

impl WeakFlock {
    pub fn new() -> WeakFlock {
        WeakFlock {
            name: &"flock",
            addr: AtomicUsize::new(1),
            _marker: marker::PhantomData,
        }
    }

    pub fn get(&self) -> Option<&FlockFunc> {
        assert_eq!(mem::size_of::<FlockFunc>(), mem::size_of::<usize>());
        unsafe {
            if self.addr.load(Ordering::SeqCst) == 1 {
                self.addr.store(fetch(self.name), Ordering::SeqCst);
            }
            if self.addr.load(Ordering::SeqCst) == 0 {
                None
            } else {
                mem::transmute::<&AtomicUsize, Option<&FlockFunc>>(&self.addr)
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
