export function analyzeGlobalOutputs(
  outputs: { text: string; expected_count: number }[],
  bank: unknown,
): { results: { display_name: string }[] };
