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

#![no_std]

pub mod mutex;
pub use mutex::Mutex;

pub mod rwlock;
pub use rwlock::RwLock;

pub mod spin;
pub mod semaphore;

pub mod once;
pub use once::OnceLock;

pub mod lazy;
pub use lazy::LazyLock;
