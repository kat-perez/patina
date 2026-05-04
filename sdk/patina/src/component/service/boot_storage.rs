//! Boot Storage Service Definition.
//!
//! This module contains the [`BootStorageService`] trait for components that expose
//! boot-storage operations to boot orchestrators. See [`BootStorageService`] for the
//! primary interface.
//!
//! ## License
//!
//! Copyright (c) Microsoft Corporation.
//!
//! SPDX-License-Identifier: Apache-2.0
//!
#[cfg(any(test, feature = "mockall"))]
use mockall::automock;

use crate::error::Result;

/// Service interface for boot-storage operations.
///
/// Boot orchestrators consume this service via dependency injection (`Service<dyn BootStorageService>`)
/// rather than implementing storage-protocol details directly. The concrete service implementation
/// lives in a platform-storage component (e.g. NVMe, eMMC, UFS) outside the orchestrator crate, and
/// is registered into the component graph alongside the orchestrator's `BootDispatcher`.
///
/// This separation lets the orchestration layer remain platform-agnostic while individual storage
/// stacks own the protocol-specific dispatch.
#[cfg_attr(any(test, feature = "mockall"), automock)]
pub trait BootStorageService {
    /// Write-protect the boot partition until the next power cycle.
    ///
    /// The exact mechanism is implementation-defined: an NVMe BPWPS Set Features command, an EC
    /// call, a secure-variable write, or any other platform-specific lock. Returns `Ok(())` once
    /// the lock is in place.
    fn lock_boot_partition(&self) -> Result<()>;
}
