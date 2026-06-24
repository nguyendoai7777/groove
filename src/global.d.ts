declare global {
  export type CastString<T> = T | (string & {});
}

export {};
