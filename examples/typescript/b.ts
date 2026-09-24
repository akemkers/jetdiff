import { formatNumber } from "./format";

/** Posisjon i en portefølje. */
export interface Posisjon {
  ticker: string;
  antall: number;
  snittkurs: number;
  valuta?: "NOK" | "USD";
}

export const MAKS_POSISJONER = 250;

export function verdi(p: Posisjon, kurs: number): number {
  return p.antall * kurs;
}

export class Portefolje {
  private readonly posisjoner = new Map<string, Posisjon>();

  leggTil(p: Posisjon): void {
    if (this.posisjoner.size >= MAKS_POSISJONER) {
      throw new Error("For mange posisjoner");
    }
    this.posisjoner.set(p.ticker, p);
  }

  beskriv(): string {
    return `Portefølje med ${this.posisjoner.size} posisjoner\n`;
  }

  sum(kurser: Record<string, number>): number {
    return [...this.posisjoner.values()].reduce(
      (total, p) => total + verdi(p, kurser[p.ticker] ?? p.snittkurs),
      0,
    );
  }
}
