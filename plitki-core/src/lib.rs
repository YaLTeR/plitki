//! `plitki-core` is a crate that provides a lot of functionality for implementing a vertical
//! scrolling rhythm game (VSRG).

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), warn(missing_docs))]
#![deny(unsafe_code)]

extern crate alloc;

mod macros;

pub mod map;
pub mod object;
pub mod scroll;
pub mod state;
pub mod timing;
pub mod visibility_cache;
