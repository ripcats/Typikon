export interface TypikonNative {
  abiVersion(): number;
  negotiateLayer(requested: number, supported: number[]): number;
  encodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array;
  decodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array;
  validateBinary(layer: number, typeName: string, input: Uint8Array): void;
}

export function loadNative(modulePath: string): TypikonNative {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  return require(modulePath) as TypikonNative;
}

// The native Node-API module is intentionally injected by the package build.
// Wire encoding/decoding remains in the Rust generated backend.
export function createTypikon(native: TypikonNative) {
  return {
    abiVersion: () => native.abiVersion(),
    negotiateLayer: (requested: number, supported: number[]) =>
      native.negotiateLayer(requested, supported),
    encodeBinary: (layer: number, typeName: string, input: Uint8Array) =>
      native.encodeBinary(layer, typeName, input),
    decodeBinary: (layer: number, typeName: string, input: Uint8Array) =>
      native.decodeBinary(layer, typeName, input),
    validateBinary: (layer: number, typeName: string, input: Uint8Array) =>
      native.validateBinary(layer, typeName, input),
  };
}
