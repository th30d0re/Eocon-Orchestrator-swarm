import Foundation

#if canImport(MLXLMCommon)

import MLXLMCommon

/// GPU memory admission for concurrent agents, wrapping MLX's wired-memory policy.
///
/// `WiredSumPolicy(cap: nil)` derives the budget from the device's *recommended working set*
/// at runtime, so the same code scales from a 64 GB Mac to a 512 GB M3 Ultra without a fixed
/// constant (the swarm's "derive budgets at runtime" decision). Each inference acquires a
/// ticket sized to the model's active footprint and runs inside `withWiredLimit`, so the sum
/// of concurrently-wired models stays under budget instead of racing into an OOM.
struct WiredMemoryAdmission {
    private let policy = WiredSumPolicy(cap: nil)

    /// Run `body` inside a wired-memory ticket sized to `activeBytes`.
    func withAdmission<R: Sendable>(activeBytes: Int, _ body: () async throws -> R) async throws -> R {
        let ticket = policy.ticket(size: max(activeBytes, 1), kind: .active)
        return try await ticket.withWiredLimit(body)
    }
}

#endif
