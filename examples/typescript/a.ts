import { formatNumber } from "./format";

/** A position in a portfolio. */
export interface Position {
  ticker: string;
  quantity: number;
  averagePrice: number;
}

export const MAX_POSITIONS = 100;

export function value(p: Position, price: number): number {
  return p.quantity * price;
}

export class Portfolio {
  private positions: Position[] = [];

  add(p: Position): void {
    this.positions.push(p);
  }

  describe(): string {
    return `Portfolio with ${this.positions.length} positions`;
  }

  // Sum without prices, uses the average price
  sum(): number {
    let total = 0;
    for (const p of this.positions) {
      total += value(p, p.averagePrice);
    }
    return total;
  }
}
