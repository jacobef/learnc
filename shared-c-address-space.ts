const MIN_ADDRESS_BASE = 1000;
const MAX_ADDRESS_BASE = 9000;
const ADDRESS_ALIGNMENT = 16;
const STORAGE_KEY_PREFIX = "cboxes-synthetic-address-base-v2:";

function isAddressBase(value: number): boolean {
  return (
    Number.isSafeInteger(value) &&
    value >= MIN_ADDRESS_BASE &&
    value <= MAX_ADDRESS_BASE &&
    value % ADDRESS_ALIGNMENT === 0
  );
}

/** Creates an aligned base for the interpreter's displayed, non-host addresses. */
export function createSyntheticAddressBase(): number {
  const random = new Uint32Array(1);
  crypto.getRandomValues(random);
  const firstBase =
    Math.ceil(MIN_ADDRESS_BASE / ADDRESS_ALIGNMENT) * ADDRESS_ALIGNMENT;
  const baseCount =
    Math.floor((MAX_ADDRESS_BASE - firstBase) / ADDRESS_ALIGNMENT) + 1;
  return firstBase + (random[0]! % baseCount) * ADDRESS_ALIGNMENT;
}

/** Keeps a page's displayed addresses stable across reloads. */
function loadPageAddressBase(): number {
  const storageKey = `${STORAGE_KEY_PREFIX}${window.location.pathname}`;
  try {
    const stored = Number.parseInt(localStorage.getItem(storageKey) ?? "", 10);
    if (isAddressBase(stored)) return stored;

    const generated = createSyntheticAddressBase();
    localStorage.setItem(storageKey, generated.toString());
    return generated;
  } catch {
    return createSyntheticAddressBase();
  }
}

export const pageSyntheticAddressBase = loadPageAddressBase();
