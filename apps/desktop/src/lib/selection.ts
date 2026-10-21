/**
 * Formats a set of 1-based page numbers into a compact range string (e.g. "1-3, 5, 8-10").
 */
export function formatSelectedPagesToRanges(selected: Iterable<number>): string {
  const sorted = Array.from(new Set(selected))
    .filter((n) => Number.isInteger(n) && n >= 1)
    .sort((a, b) => a - b);

  if (sorted.length === 0) return '';

  const parts: string[] = [];
  let start = sorted[0];
  let prev = sorted[0];

  for (let i = 1; i < sorted.length; i++) {
    const current = sorted[i];
    if (current === prev + 1) {
      prev = current;
    } else {
      parts.push(start === prev ? `${start}` : `${start}-${prev}`);
      start = current;
      prev = current;
    }
  }
  parts.push(start === prev ? `${start}` : `${start}-${prev}`);
  return parts.join(', ');
}

/**
 * Parses a range specification (e.g. "1-3, 5, 8-") into a Set of 1-based page numbers.
 */
export function parseRangesToPageSet(input: string, totalPages: number): Set<number> {
  const result = new Set<number>();
  const trimmed = input.trim();
  if (!trimmed || totalPages < 1) return result;

  for (const rawToken of trimmed.split(',')) {
    const token = rawToken.trim();
    if (!token) continue;

    if (token.includes('-')) {
      const [leftRaw, rightRaw, ...rest] = token.split('-');
      if (rest.length > 0) continue;
      const left = leftRaw.trim();
      const right = rightRaw.trim();
      if (!left && !right) continue;

      const start = left ? Number(left) : 1;
      const end = right ? Number(right) : totalPages;
      if (
        !Number.isInteger(start) ||
        !Number.isInteger(end) ||
        start < 1 ||
        end > totalPages ||
        start > end
      ) {
        continue;
      }
      for (let p = start; p <= end; p++) {
        result.add(p);
      }
    } else {
      const p = Number(token);
      if (Number.isInteger(p) && p >= 1 && p <= totalPages) {
        result.add(p);
      }
    }
  }

  return result;
}
