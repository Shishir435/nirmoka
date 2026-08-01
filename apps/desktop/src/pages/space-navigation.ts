export interface SpaceLocation {
  scanId: number;
  parentId: number | null;
}

export function parentIdForScan(location: SpaceLocation | null, scanId: number | null) {
  return location?.scanId === scanId ? location.parentId : null;
}
