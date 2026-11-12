// SHA-256 FIPS 180-4 Standard Constants Table
const HEX_RADIX = 16;

const K_HEX_STR =
  "428a2f98 71374491 b5c0fbcf e9b5dba5 3956c25b 59f111f1 923f82a4 ab1c5ed5 " +
  "d807aa98 12835b01 243185be 550c7dc3 72be5d74 80deb1fe 9bdc06a7 c19bf174 " +
  "e49b69c1 efbe4786 0fc19dc6 240ca1cc 2de92c6f 4a7484aa 5cb0a9dc 76f988da " +
  "983e5152 a831c66d b00327c8 bf597fc7 c6e00bf3 d5a79147 06ca6351 14292967 " +
  "27b70a85 2e1b2138 4d2c6dfc 53380d13 650a7354 766a0abb 81c2c92e 92722c85 " +
  "a2bfe8a1 a81a664b c24b8b70 c76c51a3 d192e819 d6990624 f40e3585 106aa070 " +
  "19a4c116 1e376c08 2748774c 34b0bcb5 391c0cb3 4ed8aa4a 5b9cca4f 682e6ff3 " +
  "748f82ee 78a5636f 84c87814 8cc70208 90befffa a4506ceb bef9a3f7 c67178f2";

const H_HEX_STR = "6a09e667 bb67ae85 3c6ef372 a54ff53a 510e527f 9b05688c 1f83d9ab 5be0cd19";

export const SHA256_K = Object.freeze(
  K_HEX_STR.split(" ").map((s) => Number.parseInt(s, HEX_RADIX))
);

export const SHA256_H_INIT = Object.freeze(
  H_HEX_STR.split(" ").map((s) => Number.parseInt(s, HEX_RADIX))
);

export const BITS_PER_BYTE = 8;
export const BLOCK_SIZE = 64;
export const WORDS_IN_BLOCK = 16;
export const SCHEDULE_WORDS = 64;
export const WORD_BYTES = 4;
export const HEX_PAD_LEN = 8;
export const HEX_BASE = 16;
export const PADDING_MAGIC = 0x80;
export const PADDING_OFFSET = 8;
export const BITS_IN_WORD = 32;

export const SHIFT_2 = 2;
export const SHIFT_3 = 3;
export const SHIFT_6 = 6;
export const SHIFT_7 = 7;
export const SHIFT_10 = 10;
export const SHIFT_11 = 11;
export const SHIFT_13 = 13;
export const SHIFT_15 = 15;
export const SHIFT_16 = 16;
export const SHIFT_17 = 17;
export const SHIFT_18 = 18;
export const SHIFT_19 = 19;
export const SHIFT_22 = 22;
export const SHIFT_25 = 25;
