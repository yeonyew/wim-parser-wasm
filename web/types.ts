/** Strings from XML are not trimmed or rewritten. Missing values are null. */
export interface WimInfo {
  imageCount: number;
  totalBytes: string | null;
  container: ContainerInfo;
  images: ImageInfo[];
}
export interface ResourceInfo {
  offset: string;
  size: string;
  originalSize: string;
  flags: number;
}
export interface ContainerInfo {
  imageType: string;
  fileSize: string;
  headerSize: number;
  formatVersion: number;
  flags: number;
  compressionChunkSize: number;
  guid: string;
  partNumber: number;
  totalParts: number;
  bootIndex: number;
  lookupTable: ResourceInfo;
  xml: ResourceInfo;
  bootMetadata: ResourceInfo;
  integrity: ResourceInfo;
  hasIntegrityTable: boolean;
}
export interface ImageTime {
  highPart: string | null;
  lowPart: string | null;
  utc: string | null;
}
export interface Languages {
  values: string[];
  count: number;
  default: string | null;
  defaultIndex: number | null;
  fallbacks: { language: string | null; value: string }[];
}
export interface ImageInfo {
  index: number;
  osName: string | null;
  displayName: string | null;
  description: string | null;
  displayDescription: string | null;
  imageSize: string | null;
  hardLinkBytes: string | null;
  edition: string | null;
  /** Raw WINDOWS/ARCH, e.g. "9". */
  architecture: string | null;
  productName: string | null;
  installationType: string | null;
  hal: string | null;
  productType: string | null;
  productSuite: string | null;
  majorVersion: string | null;
  minorVersion: string | null;
  /** XML MAJOR.MINOR.BUILD. */
  version: string | null;
  /** XML BUILD.SPBUILD, not registry CurrentBuildNumber.UBR. */
  buildVersion: string | null;
  build: string | null;
  spBuild: string | null;
  spLevel: string | null;
  /** Derived from WIM BootIndex; does not test bootability. */
  bootable: boolean;
  wimBoot: string | null;
  flags: string | null;
  systemRoot: string | null;
  languages: Languages | null;
  fileCount: string | null;
  directoryCount: string | null;
  createdTime: ImageTime | null;
  modifiedTime: ImageTime | null;
}
