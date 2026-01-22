/**
 * Session management utilities for LazorKit
 */

/**
 * Calculate session expiration slot
 * @param currentSlot - Current Solana slot
 * @param durationSlots - Duration in slots
 * @returns Expiration slot
 */
export function calculateSessionExpiration(
    currentSlot: bigint,
    durationSlots: bigint
): bigint {
    return currentSlot + durationSlots;
}

/**
 * Convert duration from seconds to slots (approximate)
 * Assumes ~400ms per slot average
 * @param seconds - Duration in seconds
 * @returns Duration in slots
 */
export function secondsToSlots(seconds: number): bigint {
    const slotsPerSecond = 1000 / 400; // ~2.5 slots/second
    return BigInt(Math.floor(seconds * slotsPerSecond));
}

/**
 * Check if a session is still valid
 * @param validUntilSlot - Session expiration slot
 * @param currentSlot - Current Solana slot
 * @returns true if session is valid
 */
export function isSessionValid(
    validUntilSlot: bigint,
    currentSlot: bigint
): boolean {
    return currentSlot < validUntilSlot;
}

/**
 * Common session durations in slots
 */
export const SESSION_DURATIONS = {
    ONE_HOUR: secondsToSlots(3600),
    ONE_DAY: secondsToSlots(86400),
    ONE_WEEK: secondsToSlots(604800),
    ONE_MONTH: secondsToSlots(2592000),
} as const;
