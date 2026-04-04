# RFC: ConnectController Service for `patina_boot`

## Change Log

- 2026-04-02: Initial RFC created.

## Motivation

The `SimpleBootManager` orchestrator currently calls `helpers::connect_all()` via `interleave_connect_and_dispatch()` to enumerate devices during boot. `connect_all()` connects every handle in the system recursively, which has several problems:

1. **Inefficiency on large topologies.** Platforms with many controllers (e.g., multiple PCI segments, USB hubs, network controllers) pay the full enumeration cost even when booting from a single NVMe drive.

2. **No selective enumeration.** A headless server that only needs NVMe cannot skip USB or display controller enumeration without replacing the entire `BootOrchestrator`.

3. **`interleave_connect_and_dispatch()` was sealed.** The function was private and hardcoded `connect_all()`. Custom `BootOrchestrator` implementations could not reuse the connect-dispatch interleaving logic with a different connection strategy.

4. **No participation by native Patina bus drivers.** The current `connect_all()` operates solely through the UEFI `ConnectController()` boot service. Native Patina bus drivers have no hook into this flow.

## Requirements

1. Define a `ConnectController` trait for pluggable connection strategies
2. Provide a default implementation that preserves current `connect_all()` behavior
3. Support selective connection by protocol, device type, or handle filter
4. Make `interleave_connect_and_dispatch()` generic over the connection function
5. Replace hardcoded `connect_all()` in `SimpleBootManager` with the strategy
6. Zero breakage — platforms that do not customize get identical behavior

## Proposed Design

### Trait Definition

A single-method trait in `patina_boot::connect_controller`:

```rust
pub trait ConnectController: Send + Sync + 'static {
    fn connect(&self, boot_services: &StandardBootServices) -> Result<()>;
}
```

Deliberately minimal — one method answering "which controllers should be connected on this pass?" Complex multi-phase strategies compose internally.

### Default Implementation

`ConnectAllStrategy` delegates to `helpers::connect_all()`:

```rust
pub struct ConnectAllStrategy;

impl ConnectController for ConnectAllStrategy {
    fn connect(&self, boot_services: &StandardBootServices) -> Result<()> {
        helpers::connect_all(boot_services)
    }
}
```

### SimpleBootManager Integration

`SimpleBootManager` accepts an optional strategy via constructor injection:

```rust
pub struct SimpleBootManager {
    config: BootConfig,
    connect_strategy: Box<dyn ConnectController>,
}

impl SimpleBootManager {
    // Default: ConnectAllStrategy (zero breakage)
    pub fn new(config: BootConfig) -> Self;

    // Custom strategy
    pub fn with_connect_strategy(config: BootConfig, strategy: impl ConnectController) -> Self;
}
```

### Generic interleave_connect_and_dispatch()

The interleaving logic is now a generic `pub(crate)` function in `simple_boot_manager.rs` that accepts any connect function:

```rust
pub(crate) fn interleave_connect_and_dispatch<B: BootServices, D: DxeDispatch + ?Sized>(
    connect_fn: impl Fn(&B) -> Result<()>,
    boot_services: &B,
    dxe_services: &D,
) -> Result<()>;
```

`SimpleBootManager::execute()` calls this with a closure that delegates to the strategy's `connect()` method. Custom `BootOrchestrator` implementations can call it directly with any connect function.

## Platform Usage Examples

**Default (unchanged):**
```rust
add.component(BootDispatcher::new(SimpleBootManager::new(config)));
```

**PCI-only (headless server):**
```rust
add.component(BootDispatcher::new(SimpleBootManager::with_connect_strategy(
    config,
    PciOnlyStrategy,
)));
```

**Skip USB (fast boot):**
```rust
add.component(BootDispatcher::new(SimpleBootManager::with_connect_strategy(
    config,
    SkipUsbStrategy,
)));
```

## Why Constructor Injection, Not a Service

Making `ConnectController` a full `Service<dyn ConnectController>` was considered but deferred:

- `BootOrchestrator::execute()` receives raw types, not `Service<T>` wrappers — changing the signature would break all implementations
- Constructor injection keeps configuration in one place: the `BootDispatcher::new()` call
- The trait can be promoted to a full service later without changing its definition

## Module Structure

```
patina_boot/src/
  connect_controller.rs          # ConnectController trait
  strategies.rs                  # pub mod connect_all;
  strategies/
    connect_all.rs               # ConnectAllStrategy (default)
  helpers.rs                     # connect_all() (unchanged)
  orchestrators/
    simple_boot_manager.rs       # Updated: uses ConnectController, owns interleave logic
  boot_dispatcher.rs             # Unchanged
  boot_orchestrator.rs           # Unchanged
  config.rs                      # Unchanged
  lib.rs                         # Re-exports ConnectController, ConnectAllStrategy
```

## Migration Path

### Phase 1: Add trait and default (this RFC)

- Add `ConnectController` trait and `ConnectAllStrategy`
- Update `SimpleBootManager` with `with_connect_strategy()`
- Make `interleave_connect_and_dispatch()` generic over the connection function
- `SimpleBootManager::new()` behavior unchanged — zero breakage

### Phase 2: Platform adoption

- Platforms opt into custom strategies as needed
- QEMU Q35 can switch to demonstrate usage

### Phase 3: Native bus driver integration (deferred)

- If native Patina bus drivers need to participate, promote `ConnectController` to a full service with component dispatch integration
- The trait definition does not change

## Risks and Mitigations

**Trait too simple for future needs:** Complex strategies compose internally within `connect()`. Default-method extensions can be added later without breaking existing implementations.

**`Box<dyn ConnectController>` allocation:** Single allocation during boot manager construction, same pattern as `Box<dyn BootOrchestrator>` in `BootDispatcher`.

## Alternatives Considered

1. **Connection policy in BootConfig** — Rejected: BootConfig is data, not behavior. Cannot express arbitrary connection logic.
2. **Full Patina service from the start** — Rejected: Requires separate component, splits configuration, changes `BootOrchestrator::execute()` signature.
3. **Filter callbacks on connect_all()** — Rejected: Closures don't compose as services, cannot be named or registered.
