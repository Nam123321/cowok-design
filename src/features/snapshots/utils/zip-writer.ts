const LOCAL_HEADER_SIG = 0x04034b50;
const CENTRAL_HEADER_SIG = 0x02014b50;
const END_OF_CENTRAL_SIG = 0x06054b50;
const ZIP_VERSION = 20;
const ZIP_STORE_METHOD = 0;
const LOCAL_HEADER_FIXED_SIZE = 30;
const CENTRAL_HEADER_FIXED_SIZE = 46;
const EOCD_FIXED_SIZE = 22;
const CRC_POLYNOMIAL = 0xedb88320;
const BITS_PER_BYTE = 8;
const BYTES_U16 = 2;
const BYTES_U32 = 4;

export function computeCrc32(bytes: Uint8Array): number {
  let crc = -1;
  for (let i = 0; i < bytes.length; i++) {
    let byte = bytes[i];
    for (let j = 0; j < BITS_PER_BYTE; j++) {
      const mask = -(byte & 1);
      byte = (byte >>> 1) ^ (CRC_POLYNOMIAL & mask);
    }
    crc = (crc >>> 1) ^ (CRC_POLYNOMIAL & -(crc & 1));
  }
  return (crc ^ -1) >>> 0;
}

class ByteCursor {
  private view: DataView;
  offset = 0;

  constructor(bytes: Uint8Array) {
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  u16(val: number): void {
    this.view.setUint16(this.offset, val, true);
    this.offset += BYTES_U16;
  }

  u32(val: number): void {
    this.view.setUint32(this.offset, val, true);
    this.offset += BYTES_U32;
  }
}

interface PreparedFile {
  nameBytes: Uint8Array;
  contentBytes: Uint8Array;
  crc: number;
  offset: number;
}

function writeLocalEntry(cursor: ByteCursor, file: PreparedFile): void {
  cursor.u32(LOCAL_HEADER_SIG);
  cursor.u16(ZIP_VERSION);
  cursor.u16(0);
  cursor.u16(ZIP_STORE_METHOD);
  cursor.u32(0);
  cursor.u32(file.crc);
  cursor.u32(file.contentBytes.length);
  cursor.u32(file.contentBytes.length);
  cursor.u16(file.nameBytes.length);
  cursor.u16(0);
}

function writeCentralEntry(cursor: ByteCursor, file: PreparedFile): void {
  cursor.u32(CENTRAL_HEADER_SIG);
  cursor.u16(ZIP_VERSION);
  cursor.u16(ZIP_VERSION);
  cursor.u16(0);
  cursor.u16(ZIP_STORE_METHOD);
  cursor.u32(0);
  cursor.u32(file.crc);
  cursor.u32(file.contentBytes.length);
  cursor.u32(file.contentBytes.length);
  cursor.u16(file.nameBytes.length);
  cursor.u16(0);
  cursor.u16(0);
  cursor.u16(0);
  cursor.u16(0);
  cursor.u32(0);
  cursor.u32(file.offset);
}

export function createZipArchive(files: { path: string; content: string }[]): Uint8Array {
  const encoder = new TextEncoder();
  const prepared: PreparedFile[] = [];
  let localSectionSize = 0;

  for (const f of files) {
    const nameBytes = encoder.encode(f.path);
    const contentBytes = encoder.encode(f.content);
    const crc = computeCrc32(contentBytes);
    prepared.push({ nameBytes, contentBytes, crc, offset: localSectionSize });
    localSectionSize += LOCAL_HEADER_FIXED_SIZE + nameBytes.length + contentBytes.length;
  }

  let centralSectionSize = 0;
  for (const p of prepared) {
    centralSectionSize += CENTRAL_HEADER_FIXED_SIZE + p.nameBytes.length;
  }

  const totalSize = localSectionSize + centralSectionSize + EOCD_FIXED_SIZE;
  const buffer = new Uint8Array(totalSize);
  const cursor = new ByteCursor(buffer);

  for (const p of prepared) {
    cursor.offset = p.offset;
    writeLocalEntry(cursor, p);
    buffer.set(p.nameBytes, cursor.offset);
    buffer.set(p.contentBytes, cursor.offset + p.nameBytes.length);
  }

  cursor.offset = localSectionSize;
  for (const p of prepared) {
    const nextCentral = cursor.offset + CENTRAL_HEADER_FIXED_SIZE + p.nameBytes.length;
    writeCentralEntry(cursor, p);
    buffer.set(p.nameBytes, cursor.offset);
    cursor.offset = nextCentral;
  }

  cursor.u32(END_OF_CENTRAL_SIG);
  cursor.u16(0);
  cursor.u16(0);
  cursor.u16(prepared.length);
  cursor.u16(prepared.length);
  cursor.u32(centralSectionSize);
  cursor.u32(localSectionSize);
  cursor.u16(0);

  return buffer;
}
