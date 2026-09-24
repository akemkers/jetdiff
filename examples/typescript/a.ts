import { formatNumber } from "./format";

/** Posisjon i en portefølje. */
export interface Posisjon {
  ticker: string;
  antall: number;
  snittkurs: number;
}

export const MAKS_POSISJONER = 100;

export function verdi(p: Posisjon, kurs: number): number {
  return p.antall * kurs;
}

export class Portefolje {
  private posisjoner: Posisjon[] = [];

  leggTil(p: Posisjon): void {
    this.posisjoner.push(p);
  }

  beskriv(): string {
    return `Portefølje med ${this.posisjoner.length} posisjoner`;
  }

  // Sum uten kurs, bruker snittkurs
  sum(): number {
    let total = 0;
    for (const p of this.posisjoner) {
      total += verdi(p, p.snittkurs);
    }
    return total;
  }
}
