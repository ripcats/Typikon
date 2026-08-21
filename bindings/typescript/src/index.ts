export interface TypikonNative {
  abiVersion(): number;
  negotiateLayer(requested: number, supported: number[]): number;
  encodeJson(layer: number, typeName: string, input: Uint8Array): Uint8Array;
  decodeJson(layer: number, typeName: string, input: Uint8Array): Uint8Array;
  encodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array;
  decodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array;
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
    encodeJson: (layer: number, typeName: string, input: Uint8Array) =>
      native.encodeJson(layer, typeName, input),
    decodeJson: (layer: number, typeName: string, input: Uint8Array) =>
      native.decodeJson(layer, typeName, input),
    encodeBinary: (layer: number, typeName: string, input: Uint8Array) =>
      native.encodeBinary(layer, typeName, input),
    decodeBinary: (layer: number, typeName: string, input: Uint8Array) =>
      native.decodeBinary(layer, typeName, input),
  };
}
