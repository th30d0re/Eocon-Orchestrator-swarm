import Foundation

#if canImport(MLXLMCommon)

import MLX
import MLXLMCommon

/// GPU memory admission for concurrent agents, wrapping MLX's wired-memory policy.
///
/// `WiredSumPolicy(cap: nil)` derives the budget from the device's *recommended working set*
/// at runtime, so the same code scales from a 64 GB Mac to a 512 GB M3 Ultra without a fixed
/// constant (the swarm's "derive budgets at runtime" decision). The sum of concurrently-wired
/// model footprints stays under budget instead of racing into an OOM.
///
/// Vends a (Sendable) `WiredMemoryTicket`; the caller runs inference inside
/// `ticket.withWiredLimit { … }` in its own isolation domain (the closure is non-Sendable,
/// so it must not cross an actor boundary — hence we hand back the ticket, not a closure).
struct WiredMemoryAdmission {
    private let policy = WiredSumPolicy(cap: nil)

    /// A ticket sized to the model's active footprint (bytes).
    func ticket(activeBytes: Int) -> WiredMemoryTicket {
        policy.ticket(size: max(activeBytes, 1), kind: .active)
    }
}

#endif
