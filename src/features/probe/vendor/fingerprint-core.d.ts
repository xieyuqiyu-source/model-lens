import type { Analysis, Bank, Output } from './types';
export function parseNumbers(text: string): number[];
export function countNumbers(numbers: number[]): number[];
export function hellingerFeature(counts: number[]): number[];
export function orderedBlockFeature(numbers: number[]): number[];
export function analyzeGlobalOutputs(outputs: Output[], bank: Bank): Analysis;
