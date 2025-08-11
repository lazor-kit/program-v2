// Polyfill for structuredClone if not available (for React Native/Expo)
if (typeof globalThis.structuredClone !== 'function') {
  globalThis.structuredClone = (obj: any) => JSON.parse(JSON.stringify(obj));
}

// Main SDK exports
export { LazorkitClient } from './client/lazorkit';
export { DefaultRuleClient } from './client/defaultRule';
export * from './types';
