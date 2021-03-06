//! `plitki-core` is a crate that provides a lot of functionality for implementing a vertical
//! scrolling rhythm game (VSRG).

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

mod macros;

pub mod map;
pub mod object;
pub mod scroll;
pub mod state;
pub mod timing;
