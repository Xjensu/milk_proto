/**
 * Deliberately small declaration fixture for the repository-only typecheck.
 *
 * The published package is compiled by the consuming React Native application,
 * which supplies the real declarations and TurboModule codegen types. This
 * fixture models only the symbols imported by this wrapper; it does not claim
 * to model the native bridge or platform runtime.
 */
declare module 'react-native' {
  export class NativeEventEmitter {
    constructor(nativeModule?: unknown);
    addListener(
      eventName: string,
      handler: (event: unknown) => void,
      context?: unknown,
    ): { remove(): void };
  }

  export const NativeModules: {
    readonly [name: string]: unknown;
    readonly MmpMesh?: unknown;
  };

  export interface TurboModule {}

  export const TurboModuleRegistry: {
    getEnforcing<T extends TurboModule>(name: string): T;
  };
}
