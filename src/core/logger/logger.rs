// Tencent is pleased to support the open source community by making Polaris available.
//
// Copyright (C) 2019 THL A29 Limited, a Tencent company. All rights reserved.
//
// Licensed under the BSD 3-Clause License (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://opensource.org/licenses/BSD-3-Clause
//
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std::sync::Arc;
use tracing::Level;
use tracing::level_filters::LevelFilter;
use tracing_appender::{non_blocking::NonBlocking, rolling};
use tracing_subscriber::{fmt::format::{DefaultFields, Format}, FmtSubscriber};

use crate::core::model::error::{ErrorCode, PolarisError};


struct AtomicUsize {
    v: std::cell::Cell<usize>,
}

impl AtomicUsize {
    const fn new(v: usize) -> AtomicUsize {
        AtomicUsize { v: std::cell::Cell::new(v) }
    }

    fn load(&self, _order: std::sync::atomic::Ordering) -> usize {
        self.v.get()
    }

    fn store(&self, val: usize, _order: std::sync::atomic::Ordering) {
        self.v.set(val)
    }

    #[cfg(target_has_atomic = "ptr")]
    fn compare_exchange(
        &self,
        current: usize,
        new: usize,
        _success: std::sync::atomic::Ordering,
        _failure: std::sync::atomic::Ordering,
    ) -> Result<usize, usize> {
        let prev = self.v.get();
        if current == prev {
            self.v.set(new);
        }
        Ok(prev)
    }
}

// Any platform without atomics is unlikely to have multiple cores, so
// writing via Cell will not be a race condition.
unsafe impl Sync for AtomicUsize {}

static STATE: AtomicUsize = AtomicUsize::new(0);

// There are three different states that we care about: the logger's
// uninitialized, the logger's initializing (set_logger's been called but
// LOGGER hasn't actually been set yet), or the logger's active.
const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;

// The LOGGER static holds a pointer to the global logger. It is protected by
// the STATE static which determines whether LOGGER has been initialized yet.
static mut LOGGER: &dyn Logger = &NopLogger;

pub fn init_logger(dir: &str, file: &str, l: LevelFilter) -> Result<(), PolarisError> {
    let logger = DefaultLogger::new(dir, file, l);
    set_logger(Box::new(logger))
}

pub fn set_logger(logger: Box<dyn Logger>) -> Result<(), PolarisError> {
    set_logger_inner(|| Box::leak(logger))
}

fn set_logger_inner<F>(make_logger: F) -> Result<(), PolarisError>
where
    F: FnOnce() -> &'static dyn Logger,
{
    match STATE.compare_exchange(
        UNINITIALIZED,
        INITIALIZING,
        std::sync::atomic::Ordering::Acquire,
        std::sync::atomic::Ordering::Relaxed,
    ) {
        Ok(UNINITIALIZED) => {
            unsafe {
                LOGGER = make_logger();
            }
            STATE.store(INITIALIZED, std::sync::atomic::Ordering::Release);
            Ok(())
        }
        Err(INITIALIZING) => {
            while STATE.load(std::sync::atomic::Ordering::Relaxed) == INITIALIZING {
                std::hint::spin_loop();
            }
            Err(PolarisError::new(ErrorCode::InternalError, "logger already initialized".to_string()))
        }
        _ => Err(PolarisError::new(ErrorCode::InternalError, "logger already initialized".to_string())),
    }
}

pub trait Logger {
    fn log(&self, level: Level, msg: &str);
}

pub struct NopLogger;

impl Logger for NopLogger {
    fn log(&self, level: Level, msg: &str) {
    }
}

pub struct DefaultLogger {
    sub: Arc<Box<FmtSubscriber<DefaultFields, Format, tracing::level_filters::LevelFilter, NonBlocking>>>,
}

impl DefaultLogger {
    pub fn new(dir: &str, file: &str, l: LevelFilter) -> DefaultLogger {
        let file_appender = rolling::never(dir, file);
        let (non_blocking_appender, _guard) = tracing_appender::non_blocking(file_appender);

        let subscriber = tracing_subscriber::fmt()
                .with_thread_names(true)
                .with_file(true)
                .with_level(true)
                .with_writer(non_blocking_appender)
                .with_line_number(true)
                .with_thread_ids(true)
                .with_max_level(l)
                .finish();

       let sub = Arc::new(Box::new(subscriber));

        Self {
            sub,
        }
    }
}

impl Logger for DefaultLogger {
    fn log(&self, level: Level, msg: &str) {
        match level {
            Level::TRACE => {
                tracing::trace!(msg);
            },
            Level::DEBUG => {
                tracing::debug!(msg);
            },
            Level::INFO => {
                tracing::info!(msg);
            },
            Level::WARN => {
                tracing::warn!(msg);
            },
            Level::ERROR => {
                tracing::error!(msg);
            },
        }
    }
}

pub fn log(level: Level, msg: &str) {
    unsafe { LOGGER }.log(level, msg);
}
