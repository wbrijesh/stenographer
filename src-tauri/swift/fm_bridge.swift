// C-ABI bridge to Apple's on-device FoundationModels framework.
//
// Compiled by build.rs with `swiftc` and linked into the Rust binary. Exposes a
// small synchronous C surface (`@_cdecl`) that the Rust `llm` module calls via
// FFI. Each cleanup uses a FRESH `LanguageModelSession` so no context accumulates
// across calls. The async `respond(to:)` API is driven to completion synchronously
// using a `DispatchSemaphore` (the pattern proven in the standalone swiftc tests).

import Foundation
import FoundationModels

private let CLEANUP_INSTRUCTIONS = """
You are a transcription cleanup assistant. Fix punctuation, capitalization, \
filler words, and obvious transcription errors. Preserve the speaker's meaning \
and wording. Output ONLY the corrected text, with no preamble or quotes.
"""

/// Returns 1 if the on-device system language model is available, else 0.
@_cdecl("fm_is_available")
public func fm_is_available() -> Int32 {
    guard #available(macOS 26.0, *) else { return 0 }
    switch SystemLanguageModel.default.availability {
    case .available:
        return 1
    default:
        return 0
    }
}

/// Fire one throwaway `respond` to warm the model so the first real cleanup is
/// on the warm path. Best-effort: ignores result and errors. Blocks briefly.
@_cdecl("fm_warm")
public func fm_warm() {
    guard #available(macOS 26.0, *) else { return }
    guard case .available = SystemLanguageModel.default.availability else { return }

    let sem = DispatchSemaphore(value: 0)
    Task {
        do {
            let session = LanguageModelSession(instructions: CLEANUP_INSTRUCTIONS)
            _ = try await session.respond(to: "warmup")
        } catch {
            // Ignore warmup errors.
        }
        sem.signal()
    }
    sem.wait()
}

/// Clean up the given transcription text using a fresh session. Returns a newly
/// allocated C string (caller must free via `fm_free`), or null on error.
@_cdecl("fm_cleanup")
public func fm_cleanup(_ textPtr: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard #available(macOS 26.0, *) else { return nil }
    guard let textPtr = textPtr else { return nil }
    guard case .available = SystemLanguageModel.default.availability else { return nil }

    let input = String(cString: textPtr)

    let sem = DispatchSemaphore(value: 0)
    var result: String? = nil
    Task {
        do {
            // Fresh session per call — no accumulated context.
            let session = LanguageModelSession(instructions: CLEANUP_INSTRUCTIONS)
            let response = try await session.respond(to: input)
            result = response.content
        } catch {
            result = nil
        }
        sem.signal()
    }
    sem.wait()

    guard let cleaned = result else { return nil }
    return strdup(cleaned)
}

/// Free a C string previously returned by `fm_cleanup`.
@_cdecl("fm_free")
public func fm_free(_ ptr: UnsafeMutablePointer<CChar>?) {
    if let ptr = ptr {
        free(ptr)
    }
}
