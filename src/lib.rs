/*  Copyright (C) 2025 Saúl Valdelvira
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, version 3.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>. */

//! Spinlock-based synchronization primitives
//!
//! The following synchronization primitives are implemented
//! - [Mutex]
//! - [RwLock]
//! - [OnceLock]
//! - [LazyLock]
#![cfg_attr(feature = "alloc", doc = "- [Arc]")]
//!
//! # Examples
//! ## Mutex
//! ```
//! use syncrs::{Mutex, LazyLock};
//!
//! static N: Mutex<u32> = Mutex::new(0);
//!
//! let threads = (0..10).map(|_|{
//!     std::thread::spawn(move || {
//!         let mut sync_n = N.lock();
//!         for _ in 0..10 {
//!             *sync_n += 1;
//!         }
//!     })
//! }).collect::<Vec<_>>();
//!
//! for t in threads {
//!     t.join().unwrap();
//! }
//!
//! assert_eq!(*N.lock(), 100);
//! ```
//!
//! ## LazyLock
//! ```
//! use syncrs::LazyLock;
//! use std::collections::HashMap;
//!
//! static MAP: LazyLock<HashMap<i32, String>> = LazyLock::new(|| {
//!     let mut map = HashMap::new();
//!     for n in 0..10 {
//!         map.insert(n, format!("{n}"));
//!     }
//!     map
//! });
//!
//! let map = MAP.get(); /* MAP is initialized at this point */
//! assert_eq!(
//!     map.get(&4).map(|s| s.as_str()),
//!     Some("4"),
//! );
//! ```

#![no_std]

pub mod mutex;
pub use mutex::Mutex;

pub mod rwlock;
pub use rwlock::RwLock;

pub mod spin;

#[cfg(feature = "alloc")]
mod arc;
#[cfg(feature = "alloc")]
pub use arc::Arc;

/// WIP
#[allow(unused)]
mod semaphore;

mod once;
pub use once::OnceLock;

mod lazy;
pub use lazy::LazyLock;
